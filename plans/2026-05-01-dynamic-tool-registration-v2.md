# 动态工具注册开发计划

## Objective

为 `torque-harness` 添加动态工具注册能力，使 Extension 和外部代码能在**扩展加载时**或**系统启动后**随时注册新工具，工具**即时生效**，无需 `/reload`，LLM 即可调用。

最小工具定义字段：

```
name         — 工具名称，LLM 调用时的标识
description  — 工具功能描述，LLM 决定是否使用
parameters   — JSON Schema 格式的参数定义
execute()    — 实际执行逻辑
```

## Background: Current Architecture

### 工具注册现状（一次性）

```
ServiceContainer::new()                               ← 启动时
  → ToolService::new_with_builtins(artifact)
    → create_builtin_tools()                           ← 硬编码 10 个工具
      → block_on(registry.register(tool))               ← 注册到 ToolRegistry
```

### LLM 调用路径（每轮重新读取）

```
RuntimeHost::run_llm_conversation()                    ← 每轮 LLM 调用
  → tool_executor.tool_defs()                           ← 实时从 HashMap 读
    → GovernedToolRegistry.to_llm_tools()
      → ToolRegistry.list()
        → tools.read().await.values()                  ← RwLock，无缓存
```

### 关键 Gap

Extension 系统与工具系统完全隔离：

```
ToolRegistry (HashMap<String, ToolArc>)                 ← 工具在这里
ExtensionActor                                          ← Extension 在这里
  └── 没有 tools() 方法声明提供的工具
  └── ExtensionConfig.tools                              ← 只有 timeout/retries，没有定义
```

### 运行时动态性已就绪

`RuntimeHost::run_llm_conversation()`（`crates/torque-runtime/src/host.rs:242`）和 `AgentLoop::run()`（`crates/torque-harness/src/harness/react.rs:196`）**每轮**都重新调用 `tool_defs()` 获取工具列表，没有任何缓存层。因此任何新注册到 `ToolRegistry` 的工具在**下一轮 LLM 调用**即可使用。

---

## Implementation Plan

### Phase 1: 将 Tool 契约提升到 torque-kernel

**目标**：将 `Tool`、`ToolResult`、`ToolArc` 从 `torque-harness` 提升到 `torque-kernel` 作为共享契约类型，使 `torque-extension` 和 `torque-harness` 都能引用同一组类型。

- [ ] 1.1 在 `torque-kernel` 中新建 `tool` 模块，定义 `Tool` trait、`ToolResult` 结构体、`ToolArc` 类型别名

**理由**：`AGENTS.md` 指导"prefer placing core runtime concepts in kernel-oriented crates"。`Tool` 是执行契约的核心部分，与 `AgentInstance`、`Task`、`ExecutionRequest` 同级。提升到 kernel 层可以避免 `torque-extension` 依赖 `torque-harness` 导致的循环依赖。

- [ ] 1.2 更新 `torque-harness` 中所有引用 `Tool`、`ToolResult`、`ToolArc` 的位置，改为引用 `torque-kernel` 中的定义

**理由**：保持类型统一，避免编译时出现两个不同的 `Tool` 类型。

- [ ] 1.3 在 `torque-extension` 中为 `ExtensionActor` 添加可选方法 `fn tools(&self) -> Vec<ToolArc>`，默认返回空 vec

**理由**：Extension 需要标准方式来声明其提供的工具。默认实现确保现有 extension 不受影响。

### Phase 2: 扩展 ToolRegistry CRUD

**目标**：为 `ToolRegistry` 添加完整的工具生命周期管理能力。

- [ ] 2.1 在 `ToolRegistry` 中添加 `remove(name: &str) -> bool` 方法，从 HashMap 中移除对应工具，返回是否找到并移除

**理由**：当前只有单向 `register()`。卸载是工具生命周期管理的基本操作。

- [ ] 2.2 在 `ToolRegistry` 中添加 `update(name: &str, tool: ToolArc) -> bool` 方法，原地替换同名工具（若不存在则返回 false）

**理由**：extension 升级或配置变更时需要更新已有工具，而不是先卸载再注册。`update` 原子性强于 `remove + register`。

### Phase 3: 扩展 ToolService 公开 API

**目标**：在 `ToolService` 上提供供 Extension 和外部代码调用的公开方法。

- [ ] 3.1 添加 `pub async fn register_tool(&self, tool: ToolArc)`，委托给 `self.registry.register(tool)`

**理由**：当前 `ToolRegistry::register()` 是 `pub`，但 `ToolService` 没有公开的 async 入口。外部调用者需要标准的注册接口。

- [ ] 3.2 添加 `pub async fn unregister_tool(&self, name: &str) -> bool`，委托给 `ToolRegistry::remove()`

**理由**：工具卸载的公开入口。

- [ ] 3.3 添加 `pub async fn list_tool_names(&self) -> Vec<String>` 和 `pub async fn get_tool(&self, name: &str) -> Option<ToolArc>`，委托给 `ToolRegistry`

**理由**：提供只读查询接口，用于 API 发现和诊断。

### Phase 4: Extension 工具注册集成

**目标**：在 Extension 注册/卸载时自动触发工具注册/清理。

- [ ] 4.1 在 `ExtensionService` 中添加 `tool_service: Option<Arc<ToolService>>` 字段，通过 `with_tool_service()` builder 方法注入

**理由**：`ExtensionService` 需要持有 `ToolService` 引用来调用 `register_tool()`。使用 `Option` 保持灵活性——没有 `ToolService` 的 Extension 环境不强制要求。

- [ ] 4.2 在 `ExtensionService::register()` 中，在成功注册 extension 后调用 `extension.tools()`，遍历返回的工具列表，对每个工具调用 `ToolService::register_tool()`

**理由**：这是核心 hook 点：每次 extension 注册时自动注册其声明工具，无论是启动时还是运行时。

- [ ] 4.3 在 `ExtensionService` 中维护 `HashMap<ExtensionId, Vec<String>>` 映射，记录每个 extension 注册了哪些工具名

**理由**：extension 卸载时需要知道要清理哪些工具。

- [ ] 4.4 在 `ExtensionService::unregister()` 中，遍历该 extension 关联的工具列表，调用 `ToolService::unregister_tool()` 清理，然后清除映射

**理由**：防止卸载后残留过期工具导致僵尸工具可被 LLM 调用。

- [ ] 4.5 在 `ServiceContainer::new()` 中将 `ToolService` 注入到 `ExtensionService`

**理由**：依赖注入的实现。

### Phase 5: 运行时 HTTP API

**目标**：提供 HTTP API 以在运行时动态注册/更新/卸载工具。

- [ ] 5.1 新建 `src/api/v1/tools.rs`，实现以下 handler：
  - `POST /v1/tools/register` — 接收 JSON 格式的工具定义（name, description, parameters schema, source），注册到 `ToolRegistry`
  - `DELETE /v1/tools/:name` — 从 `ToolRegistry` 中移除工具
  - `GET /v1/tools` — 返回所有已注册工具的名称和元数据列表
  - `PUT /v1/tools/:name` — 更新已有工具的定义或执行逻辑

**理由**：除了 extension 自动注册外，管理员也需要手动注册工具的能力。`source` 字段用于追踪工具来源（`"manual"` 或 `"extension:<ext_id>"`）。

- [ ] 5.2 在 `src/api/v1/mod.rs` 中挂载新路由

**理由**：将工具 API 暴露到 Axum router 中。

- [ ] 5.3 handler 注册时检测工具名冲突，若已存在则返回 `409 Conflict`（要求调用方明确使用 `PUT` 进行更新）

**理由**：防止意外覆盖已有工具。

### Phase 6: 工具治理集成

**目标**：确保动态注册的工具自动纳入治理检查。

- [ ] 6.1 确认 `GovernedToolRegistry::execute_with_context()` 对动态注册的工具执行完整的治理检查链

**理由**：`GovernedToolRegistry`（`governed_tool.rs:35-82`）在每次执行时检查：blocklist → policy evaluation（allowed, requires_approval）→ 最终执行。扩展工具注册到同一个 `ToolRegistry` 底层，因此自动享受全部治理检查。只需在代码审查中确认没有绕过路径。

- [ ] 6.2 可选：在 `POST /v1/tools/register` 的 payload 中添加可选治理字段 `risk_level`（默认继承系统配置）

**理由**：允许管理员在手动注册工具时声明风险等级和审批要求。

### Phase 7: 正确性保障

**目标**：验证所有执行路径上动态注册的工具即时生效。

- [ ] 7.1 确认 `RuntimeHost::run_llm_conversation()`（`crates/torque-runtime/src/host.rs:242`）每轮调用 `tool_executor.tool_defs()`，无缓存层

**理由**：这是"即时生效"的核心保证。需要显式审查确认没有引入中间缓存。

- [ ] 7.2 确认 `AgentLoop::run()`（`crates/torque-harness/src/harness/react.rs:196`）也每轮重新拉取工具列表

**理由**：团队 Supervisor 使用 `AgentLoop` 执行，需要与 RuntimeHost 同等保证。

- [ ] 7.3 确认 `HarnessToolExecutor::tool_defs()`（`crates/torque-harness/src/runtime/adapters/tool_executor.rs:52-60`）直接从 governed registry 读取，无缓存

**理由**：`HarnessToolExecutor` 是 Kernel 层和执行器之间的桥梁，必须保证无缓存。

### Phase 8: 测试

- [ ] 8.1 单元测试：`ToolRegistry` 并发注册/卸载/更新安全（多 tokio task 同时操作）
- [ ] 8.2 单元测试：`ToolService` 的 `register_tool`, `unregister_tool`, `list_tool_names` 正确性
- [ ] 8.3 集成测试：Extension 注册后，其工具立即可通过 `ToolService::list_tool_names()` 获取
- [ ] 8.4 集成测试：Extension 卸载后，其工具从工具列表中消失（清理）
- [ ] 8.5 集成测试：新注册的工具在下一轮 LLM 调用中出现在 `tool_defs()` 返回的工具列表中
- [ ] 8.6 集成测试：同名工具注册返回冲突，`PUT` 更新正常工作
- [ ] 8.7 集成测试：治理层对动态注册的工具正常执行 blocklist 和 policy evaluation

---

## Verification Criteria

- [Criterion 1] Extension 在 `register()` 后，`GET /v1/tools` 立即返回其注册的工具
- [Criterion 2] Extension 卸载后，其注册的工具自动从 `GET /v1/tools` 中消失
- [Criterion 3] 运行时通过 `POST /v1/tools/register` 注册的新工具，LLM 在下一轮调用中即可调用，无需重启或 `/reload`
- [Criterion 4] 所有扩展工具经过 `GovernedToolRegistry` 的完整治理检查（blocklist + policy）
- [Criterion 5] `RuntimeHost::run_llm_conversation()` 每轮重新获取 tool definitions，无缓存
- [Criterion 6] `torque-kernel` 中的 `Tool` trait 被 `torque-extension` 和 `torque-harness` 共同引用，无循环依赖

---

## Execution Path Analysis

工具注册后的可执行路径：

```
[注册入口]
  ExtensionService::register()
  POST /v1/tools/register
  └──→ ToolService::register_tool(tool)
        └──→ ToolRegistry::register(tool)             ← 写入 HashMap

[LLM 调用路径]
  RuntimeHost::run_llm_conversation()
    → tool_executor.tool_defs()
      → GovernedToolRegistry.to_llm_tools()
        → ToolRegistry.list()                         ← 从 HashMap 读取，新工具可见
    → model_driver.run_turn(msgs, tool_defs, sink)    ← LLM 收到新工具定义
    → LLM 返回 ToolCalls → 新工具被调用
      → tool_executor.execute(ctx, name, args)
        → GovernedToolRegistry.execute_with_context()
          → governance check (blocklist + policy)    ← 治理检查
          → ToolRegistry.execute() → tool.execute()  ← 实际执行
```

---

## Files to Modify

| File | Change | Rationale |
|------|--------|-----------|
| `crates/torque-kernel/Cargo.toml` | 修改 | 添加 serde/serde_json 依赖（ToolResult 需要） |
| `crates/torque-kernel/src/lib.rs` | 修改 | 暴露新的 `tool` 模块 |
| `crates/torque-kernel/src/tool.rs` | 新建 | 定义 `Tool` trait、`ToolResult`、`ToolArc` |
| `crates/torque-extension/Cargo.toml` | 修改 | 添加对 `torque-kernel` 的依赖 |
| `crates/torque-extension/src/actor.rs` | 修改 | 添加 `fn tools(&self) -> Vec<ToolArc>` 默认方法 |
| `crates/torque-extension/src/lib.rs` | 修改 | 确认导出 |
| `crates/torque-harness/src/tools/mod.rs` | 修改 | 重新导出 `torque-kernel` 的 `Tool`/`ToolResult`/`ToolArc` |
| `crates/torque-harness/src/infra/tool_registry.rs` | 修改 | 添加 `remove()`、`update()` 方法 |
| `crates/torque-harness/src/service/tool.rs` | 修改 | 添加 `register_tool()`、`unregister_tool()`、`list_tools()` |
| `crates/torque-harness/src/service/mod.rs` | 修改 | 向 `ExtensionService` 注入 `ToolService` |
| `crates/torque-harness/src/extension/service.rs` | 修改 | 集成工具注册/清理，添加映射追踪 |
| `crates/torque-harness/src/api/v1/mod.rs` | 修改 | 挂载新路由 |
| `crates/torque-harness/src/api/v1/tools.rs` | 新建 | 工具 API handler（POST/DELETE/GET/PUT） |
| `crates/torque-harness/src/models/v1/mod.rs` | 新建 model | 工具注册请求/响应结构体 |
| `crates/torque-harness/src/app.rs` | 修改 | 确认装配路径 |

---

## Potential Risks and Mitigations

1. **工具名冲突（两个 Extension 注册同名工具）**
   Mitigation：`register_tool()` 检测名称冲突，默认返回错误而非静默覆盖。使用 `PUT` 语义明确表明更新意图。系统维护 `ExtensionId ↔ tool_names` 映射可审计冲突来源。

2. **恶意 Extension 注册高风险工具绕过治理**
   Mitigation：所有工具都经过 `GovernedToolRegistry` 的治理检查。系统级 deny list 在 `ToolGovernanceConfig` 中具有最高优先级。

3. **Extension 非正常断开（崩溃）后工具未清理**
   Mitigation：在 `ExtensionService::unregister()` 中自动清理。对于非正常断开，可后续引入心跳机制超时清理。

4. **工具提升到 torque-kernel 导致 API 变更影响范围大**
   Mitigation：保持 trait 签名完全一致（name → &str，description → &str，parameters_schema → Value，execute → async），仅移动位置。使用类型别名平滑过渡。

5. **并发注册/卸载导致 LLM 调用读到不一致状态**
   Mitigation：`ToolRegistry` 使用 `RwLock`，读操作（list）不会被写操作（register/remove）阻塞，始终保证一致快照。Rust 的所有权系统保证引用安全。
