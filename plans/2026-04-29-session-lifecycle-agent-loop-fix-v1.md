# Session Lifecycle & Agent Loop — 生产级修复计划

## 目标

将 Session 生命周期管理和 Agent Loop 升级到生产级可用：
- LLM 自主决定循环终止（无人工硬限制）
- Cancel 信号可传播到运行中的 agent loop
- 消息历史在 suspend/resume 间完整持久化
- 终端状态（Completed/Failed/Cancelled）完整建模

---

## Phase 1: Kernel 状态机完善

### - [ ] Task 1.1 — 添加终端状态到 `AgentInstanceState` 枚举

**文件:** `crates/torque-kernel/src/agent_instance.rs:9-19`

在枚举末尾添加三个终端状态：`Completed`、`Failed`、`Cancelled`。更新 `Display` impl 和现有 `mark_ready` 从 `Running` 出发的转换（Completed/Failed/Cancelled 应是终态，不可再变为 Ready）。

同时添加 `created_at: DateTime<Utc>` 和 `updated_at: DateTime<Utc>` 字段到 `AgentInstance` 结构体。

**理由:** 内核缺少终态概念，实例永远无法进入完成/失败/取消状态，导致 recoverability 判断依赖 harness 层的字符串映射。

### - [ ] Task 1.2 — 添加 `close()` 和 `cancel()` 转换方法

**文件:** `crates/torque-kernel/src/agent_instance.rs`

```rust
pub fn complete(&mut self) -> Result<(), StateTransitionError> {
    self.transition(AgentInstanceState::Completed, &[AgentInstanceState::Running, AgentInstanceState::AwaitingTool, AgentInstanceState::AwaitingDelegation, AgentInstanceState::AwaitingApproval])
}
pub fn fail(&mut self) -> Result<(), StateTransitionError> {
    self.transition(AgentInstanceState::Failed, &[AgentInstanceState::Running, AgentInstanceState::AwaitingTool, AgentInstanceState::AwaitingDelegation, AgentInstanceState::AwaitingApproval])
}
pub fn cancel(&mut self) -> Result<(), StateTransitionError> {
    self.transition(AgentInstanceState::Cancelled, &[AgentInstanceState::Running, AgentInstanceState::AwaitingTool, AgentInstanceState::AwaitingDelegation, AgentInstanceState::AwaitingApproval, AgentInstanceState::Suspended])
}
pub fn is_terminal(&self) -> bool {
    matches!(self.state, AgentInstanceState::Completed | AgentInstanceState::Failed | AgentInstanceState::Cancelled)
}
```

### - [ ] Task 1.3 — 添加 `AgentInstanceStatus` → `AgentInstanceState` 的类型安全映射

**文件:** `crates/torque-harness/src/models/v1/agent_instance.rs`

在 `AgentInstanceStatus` 上添加 `to_kernel_state()` 方法，返回 `Option<AgentInstanceState>`。移除 harness 中的字符串映射（`recovery.rs:20-33` 的 `normalize_status`）。

### - [ ] Task 1.4 — 在 kernel `Task` 添加时间戳

**文件:** `crates/torque-kernel/src/task.rs:73-85`

`Task` 结构体添加 `created_at` 和 `updated_at` 字段（需要 `chrono` 依赖已在 `Cargo.toml` 中）。

---

## Phase 2: Agent Loop 修复

### - [ ] Task 2.1 — 移除 `MAX_TOOL_CALLS` 硬限制，替换为 LLM 驱动的自然终止

**文件:** `crates/torque-runtime/src/host.rs:18, 197-201`

删除 `pub const MAX_TOOL_CALLS: usize = 20;`。

删除 loop 内部的 `if tool_call_count >= MAX_TOOL_CALLS { return Err(...) }` 检查（第 197-201 行）。

循环已经通过 `match turn.finish_reason { ToolCalls => continue, _ => return Ok(...) }` 自然终止。LLM 不调用工具时自动退出。

保留 `MAX_CONSECUTIVE_TOOL_FAILURES = 3` 作为故障安全保护（第 253 行）。

**理由:** per pi.dev 模型，Agent turn 应该反复循环直到 LLM 返回最终响应（不再调用工具）。`MAX_TOOL_CALLS` 是人工限制，阻挡合法的长链工具调用。

### - [ ] Task 2.2 — 添加 `CancellationToken` 检查点

**文件:** `crates/torque-runtime/src/host.rs:196`（loop 入口）

在 `RuntimeHost` 结构体添加字段 `cancel_token: Option<tokio_util::sync::CancellationToken>`。

在 loop 每次迭代开始处检查取消信号（替换原 `MAX_TOOL_CALLS` 检查的位置）：
```rust
loop {
    if let Some(token) = &self.cancel_token {
        if token.is_cancelled() {
            return Err(RuntimeHostError::Runtime(anyhow::anyhow!("Execution cancelled")));
        }
    }
    // ...
}
```

在 `with_cancel_token()` builder 方法中暴露。在 `RuntimeFactory::create_handle` 中传入，由 `RunService` 持有 token 引用。

**理由:** 当前 cancel endpoint 只执行 `UPDATE ... SET status = 'Cancelled'`，运行中的 agent loop 完全不知道被取消。

### - [ ] Task 2.3 — 添加 token usage 追踪

**文件:** `crates/torque-runtime/src/host.rs:213`

在 `run_llm_conversation` 中追踪累计 token usage：
```rust
let mut total_prompt_tokens: u32 = 0;
let mut total_completion_tokens: u32 = 0;
// 每次 turn 后：
total_prompt_tokens += turn.prompt_tokens.unwrap_or(0);
total_completion_tokens += turn.completion_tokens.unwrap_or(0);
```

在 checkpoint 和最终结果中报告 token usage。

### - [ ] Task 2.4 — 在 `create_checkpoint` 中持久化消息历史

**文件:** `crates/torque-runtime/src/host.rs:362-374` 和 `crates/torque-harness/src/service/run.rs:286`

两处 `create_checkpoint` 都将 `"messages": []` 替换为实际的序列化消息：
```rust
let messages_json: Vec<serde_json::Value> = messages.iter().map(|m| serde_json::json!({
    "role": m.role,
    "content": m.content,
    "tool_call_id": m.tool_call_id,
    "name": m.name,
})).collect();
// 然后:
"messages": messages_json,
```

**理由:** 这是 Critical 缺口 — suspend 时所有对话历史丢失，resume 后 LLM 看不到任何上下文。

### - [ ] Task 2.5 — 添加 turn 生命周期 hook 注入点

**文件:** `crates/torque-runtime/src/host.rs:184-280`

在 loop 中显式标记 turn 边界，每步添加 `turn_start` / `turn_end` 日志和可选回调：
```rust
loop {
    // cancellation check
    tracing::debug!(turn = turn_count, messages = messages.len(), "turn_start");
    // compaction
    // LLM call
    // tool execution
    tracing::debug!(turn = turn_count, "turn_end");
    turn_count += 1;
}
```

如果存在 output_sink，发送 `on_turn_start` / `on_turn_end` 事件。

---

## Phase 3: ReActHarness 文本解析循环修复

### - [ ] Task 3.1 — 使 ReActHarness 使用 native tool calling 而非文本解析

**文件:** `crates/torque-harness/src/harness/react.rs:103-174`

当前 `ReActHarness::run` 使用文本解析（`parse_tool_call_from_message`）和 `MAX_STEPS=50` 硬限制。

重构为：
1. 调用 LLM 时设置 `tool_choice: "auto"`，使用 `LlmClient::chat()` 的 native tool calling
2. 移除 `MAX_STEPS` — 改为 `finish_reason` 驱动的自然终止（与 `run_llm_conversation` 一致）
3. 保留 `MAX_CONSECUTIVE_TOOL_FAILURES=3` 作为故障保护
4. 移除 `parse_tool_call_from_message` — 完全依赖 API 返回的 `ToolCall` 结构体

**理由:** 文本解析方式不稳定（依赖 LLM 输出特定格式），与生产路径不一致。`SupervisorAgent` 也应使用与 `RunService` 相同的 native tool calling 路径。

---

## Phase 4: Cancel 信号传播端到端

### - [ ] Task 4.1 — 在 `RunService` 中管理 `CancellationToken`

**文件:** `crates/torque-harness/src/service/run.rs:343-369`

`run_execution()` 创建 `CancellationToken`，通过 `RuntimeFactory` 传入 `RuntimeHost`。

在 `ServiceContainer` 中维护 `HashMap<Uuid, CancellationToken>`，key 为 instance_id。

### - [ ] Task 4.2 — 在 Cancel API 端点中触发 `CancellationToken`

**文件:** `crates/torque-harness/src/api/v1/agent_instances.rs:98-111`

`cancel()` 端点写入 DB 后，通过 `ServiceContainer` 查找并触发对应 instance 的 `CancellationToken`。

添加状态守卫：只有 `Running`/`AwaitingTool`/`AwaitingDelegation`/`AwaitingApproval` 状态的实例才能被取消。

---

## Phase 5: Compaction 深度集成

### - [ ] Task 5.1 — 将 compaction 策略设为可配置

**文件:** `crates/torque-runtime/src/context.rs:4-9`

`ContextCompactionPolicy` 添加 builder 方法 `with_message_threshold()` / `with_token_threshold()` / `with_preserve_count()`。

### - [ ] Task 5.2 — 添加 compaction 结果到 checkpoint

**文件:** `crates/torque-runtime/src/host.rs:362-374` 和 `crates/torque-harness/src/service/run.rs:286`

在 checkpoint state 中添加 `compaction_count` 和 `last_compaction_summary`，使 resume 时能看到 compaction 历史。

---

## Phase 6: 验证

### - [ ] Task 6.1 — `cargo check` 所有 crate

确保 `torque-kernel`、`torque-runtime`、`torque-harness` 全部编译通过。

### - [ ] Task 6.2 — 端到端验证路径

验证以下路径可走通：
1. `create instance → run → LLM tool calls → tool execution → complete`
2. `run → tool call → suspend → resume → continue → complete`
3. `run → cancel (mid-tool-call) → Cancelled`
4. `run → tool failure × 3 → Failed`

---

## 变更文件清单

| Phase | 文件 | 变更 |
|---|---|---|
| 1 | `torque-kernel/src/agent_instance.rs` | +Completed/Failed/Cancelled 状态, +created_at/updated_at, +complete/fail/cancel/is_terminal |
| 1 | `torque-kernel/src/task.rs` | +created_at/updated_at |
| 1 | `torque-harness/src/models/v1/agent_instance.rs` | +to_kernel_state() 方法 |
| 1 | `torque-harness/src/service/recovery.rs` | 用 to_kernel_state() 替换 normalize_status() |
| 2 | `torque-runtime/src/host.rs` | -MAX_TOOL_CALLS, +CancellationToken, +token usage 追踪, +turn hooks, 消息序列化到 checkpoint |
| 2 | `torque-harness/src/service/run.rs` | 消息序列化到 checkpoint |
| 3 | `torque-harness/src/harness/react.rs` | native tool calling, -MAX_STEPS, -parse_tool_call_from_message |
| 4 | `torque-harness/src/service/run.rs` | CancellationToken 管理 |
| 4 | `torque-harness/src/api/v1/agent_instances.rs` | cancel 端点触发 token |
| 5 | `torque-runtime/src/context.rs` | CompactionPolicy builder 方法 |
| 5 | `torque-runtime/src/host.rs` | +compaction state 到 checkpoint |

## 风险与缓解

1. **移除 MAX_TOOL_CALLS 可能导致无限循环。** 缓解：保留 `MAX_CONSECUTIVE_TOOL_FAILURES=3`，如果 LLM 确实卡住，连续失败 3 次会终止。可在后续添加可选的 `max_tool_calls` 配置。

2. **消息持久化增加 checkpoint 体积。** 缓解：使用 compaction 在保存前压缩较早的消息。仅保存最近 N 条完整消息 + 压缩摘要。

3. **CancellationToken 需要 tokio 依赖。** 缓解：`torque-runtime` 已有 tokio 依赖。使用 `tokio_util::sync::CancellationToken`（需添加 `tokio-util` 到 Cargo.toml）。
