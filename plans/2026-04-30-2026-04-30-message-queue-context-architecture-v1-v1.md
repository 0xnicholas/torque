# Torque 消息队列与上下文架构演进计划

## Objective

将 Torque 的消息投递模型从当前的裸 `Vec<RuntimeMessage>` 线性累积模式重构为分层上下文状态系统，对齐 [Context State Model 规范](../docs/superpowers/specs/2026-04-08-torque-context-state-model-design.md) 和 [Context Planes 规范](../docs/superpowers/specs/2026-04-08-torque-context-planes-design.md)。核心目标：

1. 引入三种精确的**消息投递模式**：`steer`（立即注入当前执行循环）、`followUp`（实例空闲后链式执行）、`nextTurn`（被动加入队列）
2. 引入结构化的消息队列抽象，替代 `Vec<RuntimeMessage>` 裸向量
3. 丰富消息类型，修复 `RuntimeMessage` ↔ `llm::Message` 转换中的信息丢失
4. 统一 LLM 调用路径，消除旁路（Reflexion / CandidateGenerator / Merge）绕过 RuntimeHost 的问题
5. 将上下文压缩从启发式截断升级为 LLM 驱动的语义摘要
6. 按 Context Plane 分层存储消息，实现规范的平面分离

---

## Delivery Modes Design

### 三模式语义与注入点

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                           DeliveryMode 三模式                                  │
├──────────────────────────────────────────────────────────────────────────────┤
│                                                                               │
│  Steer ── 注入当前 turn 工具执行完成后，下一轮 LLM 调用前                        │
│  ┌─────────────────────────────────────────────────────────────────────┐     │
│  │  RuntimeHost::run_llm_conversation 循环内部：                        │     │
│  │                                                                      │     │
│  │  for tool_call in turn.tool_calls {                                  │     │
│  │      tool_executor.execute(...)                                      │     │
│  │      messages.push(tool_result)                                      │     │
│  │  }                                                                   │     │
│  │  ┌──────────────────────────────────────────────┐                    │     │
│  │  │  while let Some(msg) = queue.poll_steer() {  │ ← 注入点          │     │
│  │  │      messages.push(msg.into());              │                    │     │
│  │  │  }                                           │                    │     │
│  │  └──────────────────────────────────────────────┘                    │     │
│  │  // 下一轮: model_driver.run_turn(messages, ...)                     │     │
│  └─────────────────────────────────────────────────────────────────────┘     │
│                                                                               │
│  FollowUp ── 实例状态变为 Ready 后，链式触发新执行                               │
│  ┌─────────────────────────────────────────────────────────────────────┐     │
│  │  RunService::execute_inner 末尾：                                     │     │
│  │                                                                      │     │
│  │  update_status(instance_id, Ready);                                  │     │
│  │  ┌──────────────────────────────────────────────┐                    │     │
│  │  │  let followups = queue.drain_followups();    │ ← 检查点          │     │
│  │  │  if !followups.is_empty() {                 │                    │     │
│  │  │      // 链式调用新的 execute_inner           │                    │     │
│  │  │      return self.execute_inner(...).await;   │                    │     │
│  │  │  }                                           │                    │     │
│  │  └──────────────────────────────────────────────┘                    │     │
│  │  event_sink.send(StreamEvent::Done { ... });                         │     │
│  └─────────────────────────────────────────────────────────────────────┘     │
│                                                                               │
│  NextTurn ── 仅入队，不触发任何操作                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐     │
│  │  execute_v1 启动时：                                                  │     │
│  │                                                                      │     │
│  │  let next_turn = queue.next_turn_messages();                         │     │
│  │  initial_messages.extend(next_turn);                                 │     │
│  │  // 正常执行，消息被动携带                                             │     │
│  └─────────────────────────────────────────────────────────────────────┘     │
│                                                                               │
└──────────────────────────────────────────────────────────────────────────────┘
```

### 模式对比

| 维度 | `steer` | `followUp` | `nextTurn` |
|------|---------|------------|------------|
| 投递时机 | 当前 turn 工具完成后 | 实例状态 → Ready 时 | 下次 execute 启动时 |
| 是否触发执行 | 否（仅注入当前循环） | **是**（链式新执行） | 否 |
| 对当前任务的影响 | 立即可见 | 不影响（新任务上下文） | 不影响 |
| 检查点 | `run_llm_conversation` 循环内部 poll | `RunService::execute_inner` 末尾 | 无运行时代码触发 |
| 队列语义 | 优先级出队（LIFO） | FIFO（执行完后按序） | 完全被动聚集 |
| 典型场景 | supervisor 纠偏、策略注入、上下文更新 | 审批通过后续、链式任务分解 | 背景知识、跨 session 记忆注入 |

### DeliveryMode 枚举定义

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryMode {
    /// 注入当前执行循环中，下一轮 LLM 调用前生效。
    /// 不触发新执行，由 RuntimeHost::poll_steer() 轮询消费。
    Steer,
    /// 当前实例空闲后触发新的 execute_inner 调用。
    /// 由 RunService 在 update_status(Ready) 后消费。
    FollowUp,
    /// 仅入队，在下次 execute_v1 启动时合并到 initial_messages。
    /// 不做任何运行时触发。
    NextTurn,
}
```

### 与规范的映射

| 规范条款 | 对应模式 | 说明 |
|----------|----------|------|
| §3.3 Authoritative State Stays Split | `steer` | supervisor 通过 steer 注入共享状态切片，而非共享全局可变状态 |
| §7.2 Two-Stage Slicing Model | `steer` | supervisor 显式添加/移除子 agent 可见的上下文项 |
| §7.5 Escalation for More Context | `steer` | 子 agent 请求更多上下文 → supervisor 批准 → steer 注入 |
| §8.3 Parent As Visibility Authority | `steer` | 父层控制子 agent 可见内容，保持治理本地化 |
| §10.1 Lazy Loading by Default | `nextTurn` | 被动加载，不强制立即消费 |
| §10.2 Retrieval Pattern | `nextTurn` | 从外部引用按需加载，不预载所有上下文 |
| §11.5 Periodic State Convergence | `steer` | 收敛后的压缩状态通过 steer 注入活跃执行 |

---

## Implementation Plan

### Phase 1: 消息类型增强 (Runtime Layer)

- [~] Task 1. **向 `RuntimeMessage` 添加 `tool_calls` 和 `tool_call_id` 字段**
  - 当前 `RuntimeMessage`（`crates/torque-runtime/src/message.rs:14-17`）仅为 `{role, content}` 扁平结构
  - 添加 `tool_calls: Option<Vec<RuntimeToolCall>>` 和 `tool_call_id: Option<String>` 字段
  - 更新 `From<LlmMessage> for RuntimeMessage`（`message.rs:44-57`）保留完整信息
  - 更新 `From<RuntimeMessage> for LlmMessage`（`message.rs:59-75`）恢复完整信息
  - 更新所有 checkpoint 序列化/反序列化路径以兼容新字段
  - 理由：当前转换链路丢包 `tool_calls`、`tool_call_id`、`name` 三个字段，导致恢复时丢失工具调用结构

- [ ] Task 2. **引入 `StructuredMessage` 枚举替代扁平 `RuntimeMessage`**
  - 定义 `enum StructuredMessage`：
    - `System { content, policy_ref }`
    - `UserInput { content }`
    - `AssistantResponse { content, tool_calls }`
    - `ToolResult { call_id, tool_name, result }`
    - `CompactionMarker { summary }`
    - `TaskPacket { goal, instructions, shared_state_slice, constraints }`
    - `SteerInjection { source, payload }` — 携带 supervisor 来源标识
  - 实现 `StructuredMessage::to_llm()` 和 `StructuredMessage::from_llm()` 双向转换
  - 在 `torque-runtime` crate 中实现，保持 `RuntimeMessage` 为兼容过渡类型
  - 理由：规范要求 "structure first, transcript last"，工具结果应当携带 `call_id` 以便精确对应；`SteerInjection` 变体用于审计 steer 消息来源

### Phase 2: 消息队列抽象 (Queue Layer)

- [ ] Task 3. **定义 `MessageQueue` trait，内置三模式投递**
  - 位置：`crates/torque-runtime/src/message_queue.rs`（新建）
  - 定义 `DeliveryMode` 枚举（见上方 Design 段）
  - Trait 方法：
    ```rust
    #[async_trait]
    pub trait MessageQueue: Send + Sync {
        /// 入队消息并指定投递模式
        async fn enqueue(&mut self, msg: StructuredMessage, mode: DeliveryMode);

        /// 轮询待注入的 steer 消息（RuntimeHost 循环内调用）
        fn poll_steer(&mut self) -> Option<StructuredMessage>;

        /// 获取所有待处理的 followUp 消息并清空（RunService 调用）
        fn drain_followups(&mut self) -> Vec<StructuredMessage>;

        /// 获取被动队列中的 nextTurn 消息（execute_v1 启动时调用）
        fn next_turn_messages(&self) -> Vec<&StructuredMessage>;

        /// 转换为 LLM 可消费消息列表（steer 已注入的消息会被标记）
        fn to_llm_messages(&self) -> Vec<llm::Message>;

        /// 精确 token 计数
        fn token_count(&self) -> usize;

        /// 上下文压缩
        fn compact(&mut self, policy: &CompactionPolicy) -> Option<CompactSummary>;
    }
    ```
  - 理由：三模式是架构核心，必须在 trait 层面显式建模，避免后续通过隐式约定实现

- [ ] Task 4. **实现 `InMemoryMessageQueue` 默认实现**
  - 内部使用三个独立队列：
    - `steer_queue: VecDeque<StructuredMessage>` — LIFO 语义（后进先出）
    - `followup_queue: VecDeque<StructuredMessage>` — FIFO 语义
    - `nextturn_queue: Vec<StructuredMessage>` — 被动聚集
  - 实现精确 token 计数（集成 `llm::LlmClient::count_tokens`）
  - 支持容量上限配置（`max_total_tokens`, `max_steer_messages`, `max_followup_depth`）
  - `drain_followups()` 清空 followup_queue 后返回
  - `poll_steer()` 从 steer_queue 尾部弹出（LIFO）
  - `to_llm_messages()` 合并 steer_queue、nextturn_queue 和主消息列表
  - 理由：三类消息的不同语义要求不同的出队策略，不能混用单一 VecDeque

### Phase 3: RuntimeHost 集成替换

- [ ] Task 5. **重构 `RuntimeHost::run_llm_conversation` 使用 `MessageQueue` + `steer` 轮询**
  - 将 `messages: Vec<RuntimeMessage>`（`host.rs:200`）替换为 `Box<dyn MessageQueue>`
  - 移除手动 `messages.clone()` 全量克隆，改用 `MessageQueue::to_llm_messages()`
  - 工具结果反馈使用 `queue.enqueue(StructuredMessage::ToolResult{...}, DeliveryMode::NextTurn)`
  - 助手响应使用 `queue.enqueue(StructuredMessage::AssistantResponse{...}, DeliveryMode::NextTurn)`
  - **在工具执行后、下一轮 LLM 调用前插入 `poll_steer()` 轮询**：
    ```rust
    // host.rs ToolCalls 分支末尾
    while let Some(steer_msg) = queue.poll_steer() {
        tracing::info!(msg_type = ?steer_msg.variant_name(), "steer injected");
        // steer 消息已自动进入 to_llm_messages() 的可见列表
    }
    ```
  - 理由：steer 轮询是三模式中唯一需要循环内检查的模式，必须显式插入

- [ ] Task 6. **将 LLM 驱动摘要集成到 `ContextCompactionService`**
  - 当前 `context.rs:64-99` 使用 `preview_chars: 160` 纯文本截断
  - 添加 `CompactionStrategy::LlmDriven` 变体，通过 `RuntimeModelDriver` 调用 LLM 生成摘要
  - 保留 `CompactionStrategy::Heuristic` 作为降级路径（LLM 不可用时）
  - `CompactSummary` 扩展为包含结构化 `key_facts`, `decisions`, `open_questions`
  - 压缩后以 `StructuredMessage::CompactionMarker` 注入队列
  - 对齐规范 §11.5："Periodic State Convergence" — 压缩后状态应包含 confirmed facts、confirmed decisions、open questions
  - 理由：纯截断丢失语义，LLM 摘要可保留关键决策和事实

### Phase 4: RunService followUp 链式执行

- [ ] Task 7. **在 `RunService::execute_inner` 末尾集成 `followUp` 检查**
  - 位置：`crates/torque-harness/src/service/run.rs:228-278`
  - 在 `update_status(instance_id, Ready)` 之后、发送 `Done` 事件之前插入检查：
    ```rust
    // run.rs execute_inner 末尾
    self.agent_instance_repo
        .update_status(instance_id, AgentInstanceStatus::Ready)
        .await?;

    // ★ followUp 检查点
    let followups = queue.drain_followups();
    if !followups.is_empty() {
        tracing::info!(count = followups.len(), "chain-executing followUp messages");
        let next_request = build_followup_request(&request, followups);
        return self.execute_inner(instance_id, next_request, event_sink, followups).await;
    }

    // 正常终结
    match result {
        Ok(_) => event_sink.send(StreamEvent::Done { ... }),
        Err(e) => event_sink.send(StreamEvent::Error { ... }),
    }
    ```
  - 添加 `followup_depth` 参数到 `execute_inner`，默认上限 3 层，防止无限递归
  - 理由：followUp 是唯一会触发新执行的模式，必须在 RunService 层显式处理

- [ ] Task 8. **在 `RuntimeHost::execute_v1` 启动时合并 `nextTurn` 消息**
  - 位置：`host.rs:148-149`
  - 将 `initial_messages: Vec<RuntimeMessage>` 扩展为合并 `queue.next_turn_messages()`：
    ```rust
    pub async fn execute_v1(
        &mut self,
        request: ExecutionRequest,
        model_driver: &dyn RuntimeModelDriver,
        tool_executor: &dyn RuntimeToolExecutor,
        output_sink: Option<&dyn RuntimeOutputSink>,
        initial_messages: Vec<RuntimeMessage>,
    ) -> Result<ExecutionResult, RuntimeHostError> {
        // 合并 nextTurn 消息
        let mut all_messages = initial_messages;
        for nt in self.queue.next_turn_messages() {
            all_messages.push(nt.clone().into());
        }
        // ...
    }
    ```
  - 理由：nextTurn 不触发任何操作，仅在下一次执行启动时被动携带

### Phase 5: 统一 LLM 调用路径

- [ ] Task 9. **将 `ReflexionService` LLM 调用通过 RuntimeHost 桥接**
  - `reflexion.rs:150-166` 当前直接调用 `llm::LlmClient::chat()`
  - 改为注入 `Arc<dyn RuntimeModelDriver>`，通过 `model_driver.run_turn()` 调用
  - 保持 Reflexion 的消息简洁性（2 条消息），不引入完整的对话循环
  - 理由：统一调用路径可获得统一的 token 计数、审计日志和策略评估

- [ ] Task 10. **将 `CandidateGenerator` LLM 调用通过 RuntimeHost 桥接**
  - `candidate_generator.rs:73-81` 当前直接调用 `llm::LlmClient::chat()`
  - 改为注入 `Arc<dyn RuntimeModelDriver>`
  - 保持候选生成的消息模式不变（system + user）
  - 理由：与 Task 9 相同，统一路径

- [ ] Task 11. **将 `SummarizeStrategy` LLM 调用通过 RuntimeHost 桥接**
  - `merge_strategy.rs:165-173` 当前直接调用 `llm::LlmClient::chat()`
  - 改为注入 `Arc<dyn RuntimeModelDriver>`
  - 理由：确保 Memory 合并也受策略治理

### Phase 6: Context Plane 分层存储

- [ ] Task 12. **实现 `ContextPlane` 区分存储**
  - 在 `MessageQueue` 或 checkpoint 层实现三个平面的消息分类：
    - `ExternalContextRef`：外部引用消息，标记为只读，允许 steer 注入
    - `Artifact`：工具执行输出和 LLM 生成物
    - `Memory`：语义保留（通过 `MemoryWriteCandidate` 管道，仅 nextTurn 加入）
  - Checkpoint 序列化时按平面组织消息：`{"external_refs": [...], "artifacts": [...], "memory_nominations": [...]}`
  - 恢复时按平面分别加载，默认只加载 `Artifact` 平面到活跃上下文
  - 理由：对齐 Context Planes 规范 §3，防止平面坍塌

- [ ] Task 13. **实现 `TaskPacket` 材质化**
  - 定义 `TaskPacket` 结构体（对齐规范 §6.3）：
    - `goal`, `instructions`, `expected_outputs`
    - `input_artifact_refs`, `visible_context_refs`
    - `shared_state_slice`
    - `constraints: ExecutionConstraints`
  - 实现 `TaskPacket::from_context_planes()` — 从三个平面组装最小执行包
  - 实现 `TaskPacket::to_structured_message()` — 转换为 `StructuredMessage::TaskPacket`
  - 修改 `RuntimeHost::execute_v1()` 接受 `TaskPacket` 而非 `Vec<RuntimeMessage>`
  - 理由：规范核心要求 — "execution context is materialized as a narrow derived packet"

### Phase 7: 清理与统一

- [ ] Task 14. **移除 AgentLoop Legacy 路径**
  - 评估 `crates/torque-harness/src/harness/react.rs` 中 `AgentLoop` 的剩余调用方
  - 将 `AgentLoop::run()` 和 `AgentLoop::triage()` 的调用方迁移到 RuntimeHost 路径
  - 移除或标记 `AgentLoop` 为 deprecated
  - 理由：双执行路径（RuntimeHost vs AgentLoop）行为不一致，消除维护负担

- [ ] Task 15. **更新所有测试以匹配新消息模型和三模式**
  - 更新 `torque-runtime/tests/host_port_integration.rs` 中的消息构造
  - 更新 `torque-harness/tests/checkpoint_recovery_tests.rs` 的 checkpoint 格式断言
  - 更新 `torque-harness/tests/context_compaction_tests.rs` 的压缩逻辑测试
  - 添加 `StructuredMessage` 和 `MessageQueue` 的单元测试
  - 添加 `DeliveryMode` 三模式语义测试：
    - steer 消息在工具执行后被 poll_steer() 消费
    - followUp 消息在实例 Ready 后被 drain 并触发链式执行
    - nextTurn 消息不触发任何运行时操作，仅在下一次 execute_v1 时携带
    - followUp 链式深度达到上限后正确终止
  - 添加 `TaskPacket` 组装和序列化测试

---

## Verification Criteria

- `RuntimeMessage` 到 `llm::Message` 往返转换不再丢失 `tool_calls`、`tool_call_id`、`name` 字段
- `MessageQueue` trait 的 `InMemoryMessageQueue` 实现通过三种 DeliveryMode 的出队语义测试：
  - `steer` → `poll_steer()` 返回 LIFO 顺序，在工具执行后消费
  - `followUp` → `drain_followups()` 返回 FIFO 顺序并清空队列
  - `nextTurn` → `next_turn_messages()` 返回全量且不修改队列
- `RuntimeHost::run_llm_conversation` 不再执行 `messages.clone()` 全量克隆
- RuntimeHost 循环中插入 `poll_steer()` 后，steer 消息在下一轮 LLM 调用前可见
- `RunService::execute_inner` 在实例 Ready 后正确检查 `drain_followups()` 并链式执行
- followUp 链式执行深度不超过 3 层
- `ReflexionService`、`CandidateGenerator`、`SummarizeStrategy` 的 LLM 调用经过 `RuntimeModelDriver`
- Checkpoint 序列化按 `{external_refs, artifacts, memory_nominations}` 三个平面组织
- `TaskPacket` 可从上下文平面正确材质化并投递给执行引擎
- 所有现有集成测试通过，无回归
- AgentLoop 调用方已迁移或确认废弃

---

## Potential Risks and Mitigations

1. **RuntimeMessage 字段扩展导致 checkpoint 格式不兼容**
   Mitigation: 添加 `SchemaVersion` 字段到 checkpoint payload，支持向前兼容反序列化；旧格式自动映射为 `tool_calls: None`。

2. **LLM 驱动摘要引入额外延迟和成本**
   Mitigation: 保留 `CompactionStrategy::Heuristic` 作为降级路径；LLM 摘要仅在消息数或 token 数超过阈值时触发；使用轻量模型（如 `gpt-4o-mini`）进行摘要。

3. **统一 LLM 调用路径可能改变旁路服务的行为**
   Mitigation: 旁路服务（Reflexion、CandidateGenerator、Summarize）保持原有的 2 条消息模式，仅替换底层调用方式，不引入对话循环。

4. **MessageQueue 抽象可能导致过度设计**
   Mitigation: 仅实现 `InMemoryMessageQueue`，trait 方法聚焦于三模式必需接口；优先落实到 RuntimeHost + RunService 后再评估需要扩展的能力。

5. **TaskPacket 材质化逻辑可能过度依赖未就绪的 Context Plane 层**
   Mitigation: `TaskPacket` 初始实现接受显式参数而非自动从平面组装；后续迭代中逐步引入自动材质化。

6. **followUp 链式执行可能触发无限递归**
   Mitigation: `execute_inner` 添加 `followup_depth` 计数器，上限 3；超限后剩余 followUp 消息降级为 `nextTurn` 模式入队；在 tracing 中记录降级事件。

7. **steer 消息可能在工具执行循环内堆积**
   Mitigation: `poll_steer()` 返回 `Option` 单个出队，不批量消费；`steer_queue` 容量上限 `max_steer_messages` 默认 8，超限时最旧消息降级为 `nextTurn`。

---

## Alternative Approaches

1. **方案 A：渐进式演进（推荐）**
   - Phase 1-2 先完成消息类型增强和 Queue trait（含三模式），保持 RuntimeHost 兼容
   - Phase 3-4 逐步替换 RuntimeHost 内部（steer 轮询 + followUp 链式执行）
   - Phase 5-6 统一旁路路径 + Plane 分离 + TaskPacket
   - Phase 7 清理遗留路径 + 全量测试
   - 优点：每步可验证，风险可控；缺点：周期较长（7 个 Phase）

2. **方案 B：大爆炸重构**
   - 一次性实现全部 StructuredMessage、MessageQueue（三模式）、Plane 分离、TaskPacket
   - 优点：架构干净，无过渡状态；缺点：风险高，难以逐步验证

3. **方案 C：仅修复信息丢失 + 三模式，不引入 Plane 分离**
   - 扩展 `RuntimeMessage` 字段保真度 + 实现 DeliveryMode 三模式
   - 暂不实现 ContextPlane 分层存储和 TaskPacket
   - 优点：变更范围更小；缺点：仍需后续 Plane 重构，可能两次迁移 checkpoint 格式
