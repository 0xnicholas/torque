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

`RuntimeHost::run_llm_conversation()`（`crates/torque-runtime/src/host.rs:242`）和 `AgentLoop::run()`（`crates/torque-harness/src/harness/react.rs:196`）**每轮**都重新调用 `tool_defs()` 获取工具列表。审查确认 6 层包装（RuntimeHost → HarnessToolExecutor → GovernedToolRegistry → ToolRegistry → RwLock → HashMap）**全部无缓存**。因此任何新注册到 `ToolRegistry` 的工具在**下一轮 LLM 调用**即可使用。

---

## Implementation Plan

### Phase 1: 将 Tool 契约提升到 torque-kernel

**目标**：将 `Tool`、`ToolResult`、`ToolArc` 从 `torque-harness` 提升到 `torque-kernel` 作为共享契约类型，使 `torque-extension` 和 `torque-harness` 都能引用同一组类型。

- [ ] 1.0 在 `torque-kernel/Cargo.toml` 中添加 `async-trait` 和 `anyhow` 依赖

**理由**：`Tool` trait 需要 `#[async_trait]`，`execute()` 返回 `anyhow::Result<ToolResult>`。当前 `torque-kernel` 只有 5 个依赖（serde, serde_json, uuid, chrono, thiserror），缺少这两个。

- [ ] 1.1 在 `torque-kernel/src/tool.rs` 中定义 `Tool` trait、`ToolResult` 结构体、`ToolArc` 类型别名（从 `torque-harness/src/tools/mod.rs` 照搬，签名保持不变）

**理由**：`AGENTS.md` 指导"prefer placing core runtime concepts in kernel-oriented crates"。`Tool` 是执行契约的核心部分，与 `AgentInstance`、`Task`、`ExecutionRequest` 同级。`torque-extension` 已依赖 `torque-kernel`，提升后二者可直接共享同一类型。

- [ ] 1.2 将 `torque-harness/src/tools/mod.rs` 改为从 `torque-kernel` re-export：

```rust
// torque-harness/src/tools/mod.rs (简化)
pub use torque_kernel::tool::{Tool, ToolArc, ToolResult};
// ... 其他模块 pub mod 保持不变
```

**理由**：审查发现 22 个文件引用了 `crate::tools::{Tool, ToolArc, ToolResult}`。使用 re-export 策略可零改动全部现有导入路径。外部测试文件引用 `torque_harness::tools::*` 也继续工作。

- [ ] 1.3 在 `torque-extension/src/actor.rs` 中为 `ExtensionActor` 添加可选方法 `fn tools(&self) -> Vec<ToolArc>`，默认返回空 vec

**理由**：Extension 需要标准方式来声明其提供的工具。默认实现确保现有 extension 不受影响。`torque-extension` 已依赖 `torque-kernel`，可直接使用 `torque_kernel::tool::ToolArc`。

### Phase 2: 扩展 ToolRegistry CRUD

**目标**：为 `ToolRegistry` 添加完整的工具生命周期管理能力。

- [ ] 2.1 在 `ToolRegistry`（`infra/tool_registry.rs`）中添加 `remove(name: &str) -> bool` 方法，从 HashMap 中移除对应工具，返回是否找到并移除

**理由**：当前只有单向 `register()`。卸载是工具生命周期管理的基本操作。

- [ ] 2.2 在 `ToolRegistry` 中添加 `update(name: &str, tool: ToolArc) -> bool` 方法，原地替换同名工具（若不存在则返回 false）

**理由**：extension 升级或配置变更时需要更新已有工具。`update` 原子性强于 `remove + register`，避免竞态窗口。

### Phase 3: 扩展 ToolService 公开 API

**目标**：在 `ToolService`（`service/tool.rs`）上提供供 Extension 和外部代码调用的公开方法。

- [ ] 3.1 添加 `pub async fn register_tool(&self, tool: ToolArc)`，委托给 `self.registry.register(tool)`

**理由**：当前 `ToolRegistry::register()` 是 `pub` 但被 `block_on` 包裹在构造函数中。需要一个公开的 async 入口供所有外部调用者使用。

- [ ] 3.2 添加 `pub async fn unregister_tool(&self, name: &str) -> bool`，委托给 `ToolRegistry::remove()`

**理由**：工具卸载的公开入口。

- [ ] 3.3 添加 `pub async fn list_tool_names(&self) -> Vec<String>` 和 `pub async fn get_tool(&self, name: &str) -> Option<ToolArc>`，委托给 `ToolRegistry`

**理由**：提供只读查询接口，用于 API 发现和诊断。

### Phase 4: Extension 工具注册集成

**目标**：在 Extension 注册/卸载时自动触发工具注册/清理。此 Phase 仅在 `extension` feature 启用时编译。

- [ ] 4.1 在 `ExtensionService`（`extension/service.rs`）中添加 `tool_service: Option<Arc<ToolService>>` 字段（使用 `Option` 是因为 `ToolService` 始终可用，而 `ExtensionService` 受 feature gate 控制，运行时可选注入；`tool_service` 为 `None` 时所有工具注册/清理静默跳过）

**理由**：`ExtensionService` 需要持有 `ToolService` 引用来调用 `register_tool()`。`Option` 保持灵活性，配合 `with_tool_service()` builder 方法注入。

- [ ] 4.2 在 `ExtensionService` 中添加 `tool_map: RwLock<HashMap<ExtensionId, Vec<String>>>`，记录每个 extension 注册了哪些工具名。使用 `RwLock` 确保并发注册/卸载安全。

**理由**：extension 卸载时需要知道要清理哪些工具。`RwLock` 与 `ToolRegistry` 的锁模式一致。

- [ ] 4.3 在 `ExtensionService::register()` 中，在成功注册 extension 后调用 `extension.tools()`，遍历返回的工具列表，对每个工具调用 `tool_service.register_tool()`（若 `tool_service` 存在），并记录到 `tool_map`

**理由**：核心 hook 点：每次 extension 注册时自动注册其声明的工具，无论是启动时还是运行时。

- [ ] 4.4 在 `ExtensionService::unregister()` 中，从 `tool_map` 查出该 extension 注册的工具名列表，调用 `tool_service.unregister_tool()` 清理，然后从 `tool_map` 中清除该 extension 的条目

**理由**：防止卸载后残留过期工具导致僵尸工具可被 LLM 调用。

- [ ] 4.5 在 `ServiceContainer::new()` 中将 `ToolService` 注入到 `ExtensionService`，通过 `with_tool_service(tool_service.clone())` builder 调用

**理由**：依赖注入的装配实现。

### Phase 5: 运行时 HTTP API

**目标**：提供 HTTP API 以在运行时动态注册/更新/卸载工具。为 Extension 提供的自动注册之外的一个补充通道。

- [ ] 5.1 新建 `src/api/v1/tools.rs`，实现以下 handler：
  - `POST /v1/tools/register` — 接收 JSON 格式的工具定义（name, description, parameters schema, optional source），注册到 `ToolRegistry`
  - `DELETE /v1/tools/:name` — 从 `ToolRegistry` 中移除工具
  - `GET /v1/tools` — 返回所有已注册工具的名称和元数据列表
  - `PUT /v1/tools/:name` — 更新已有工具的定义或执行逻辑

**理由**：除了 extension 自动注册外，管理员也需要手动注册工具的能力。`source` 字段用于追踪工具来源（`"manual"` 或 `"extension:<ext_id>"`）。

- [ ] 5.2 在 `src/api/v1/mod.rs` 中挂载新路由

**理由**：将工具 API 暴露到 Axum router 中。

- [ ] 5.3 handler 注册时检测工具名冲突，若已存在则返回 `409 Conflict`（要求调用方明确使用 `PUT` 进行更新）

**理由**：防止意外覆盖已有工具。

### Phase 6: 工具治理集成

**目标**：确保动态注册的工具纳入治理检查。

- [ ] 6.1 确认 `GovernedToolRegistry::execute_with_context()` 对动态注册的工具执行 blocklist 检查

**理由**：`GovernedToolRegistry`（`governed_tool.rs:35-82`）有三步检查：blocklist → policy evaluation → 执行。工具注册到同一个 `ToolRegistry`，blocklist 检查自动覆盖。**注意**：当前生产路径 `HarnessToolExecutor::execute()` 传入 `policy_sources = None`，policy evaluation 步骤会被跳过。这是一项**已知盲区**，不影响本计划的"即时生效"目标，但影响治理完整性。6.2 处理此问题。

- [ ] 6.2 评估 policy evaluation 的接入方案

**理由**：`HarnessToolExecutor::execute()`（`adapters/tool_executor.rs:36-38`）传递 `None` 给 `policy_sources` 参数，导致 policy evaluation 在生产路径上不执行。可选方案：
  - 方案 A：在 `RuntimeHost` 运行上下文中持有当前 `AgentDefinitionId`，自动构造 `PolicyInput` 并传递
  - 方案 B：在 `GovernedToolRegistry` 层缓存默认 policy（从 `ToolPolicyRepository` 加载），当 `policy_sources` 为 `None` 时用默认 policy 兜底
  - 方案 C：将此问题记录为独立技术债务，本计划暂不处理（不影响工具注册核心功能）

**建议**：采用方案 C，在 `Phase 8` 测试中注明此盲区。将方案 A/B 作为后续的独立计划。

- [ ] 6.3 在 `POST /v1/tools/register` 的 payload 中添加可选治理字段 `risk_level: Option<ToolRiskLevel>` 和 `requires_approval: Option<bool>`（默认从系统配置继承）

**理由**：允许管理员在手动注册工具时声明风险等级。即使 policy evaluation 层尚未接入，这些字段为后续治理接入提供了数据。

### Phase 7: 正确性保障

**目标**：验证所有执行路径上动态注册的工具即时生效。审查确认此 Phase 的所有断言以代码审查结果为准。

- [ ] 7.1 审查确认 `RuntimeHost::run_llm_conversation()`（`crates/torque-runtime/src/host.rs:242`）在 `loop {}` 内直接调用 `tool_executor.tool_defs().await?`，无缓存层（如 `OnceCell`、`lazy_static` 或缓存字段）

**理由**：这是"即时生效"的核心保证。

- [ ] 7.2 审查确认 `AgentLoop::run()`（`crates/torque-harness/src/harness/react.rs:196`）在 `execute_step()` 中每轮调用 `self.tools.to_llm_tools().await`，无缓存

**理由**：团队 Supervisor 使用 `AgentLoop`，需要与 RuntimeHost 同等保证。

- [ ] 7.3 审查确认 `HarnessToolExecutor::tool_defs()`（`crates/torque-harness/src/runtime/adapters/tool_executor.rs:52-60`）直接委托给 `self.governed.to_llm_tools().await`，无缓存

**理由**：`HarnessToolExecutor` 是 Kernel 层和执行器之间的桥梁，必须保证无缓存。内部类型转换（`llm::ToolDef → RuntimeToolDef`）使用 `Vec::into_iter().map()` 逐元素转换，无累积缓存。

### Phase 8: 测试

- [ ] 8.1 单元测试：`ToolRegistry` 并发注册/卸载/更新安全（多 tokio task 同时操作 RwLock）
- [ ] 8.2 单元测试：`ToolService` 的 `register_tool`、`unregister_tool`、`list_tool_names` 正确性
- [ ] 8.3 集成测试：Extension 注册后，其工具立即可通过 `ToolService::list_tool_names()` 获取
- [ ] 8.4 集成测试：Extension 卸载后，其工具从工具列表中消失
- [ ] 8.5 集成测试：新注册的工具在下一轮 LLM 调用中出现在 `tool_defs()` 返回的工具列表中
- [ ] 8.6 集成测试：同名工具注册返回冲突，`PUT` 更新正常工作
- [ ] 8.7 集成测试：blocklist 对动态注册的工具正常生效（已知 policy evaluation 为盲区，不在本测试覆盖范围内——注明为已知限制）

---

## Verification Criteria

- [Criterion 1] Extension 在 `register()` 后，`GET /v1/tools` 立即返回其注册的工具
- [Criterion 2] Extension 卸载后，其注册的工具自动从 `GET /v1/tools` 中消失
- [Criterion 3] 运行时通过 `POST /v1/tools/register` 注册的新工具，LLM 在下一轮调用中即可调用，无需重启或 `/reload`
- [Criterion 4] blocklist 治理对动态注册的工具生效（policy evaluation 标记为已知盲区，需后续计划处理）
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
    → tool_executor.tool_defs()                         ← 每轮重新读取
      → GovernedToolRegistry.to_llm_tools()
        → ToolRegistry.list()                          ← 从 HashMap 读，新工具可见
    → model_driver.run_turn(msgs, tool_defs, sink)     ← LLM 收到新工具定义
    → LLM 返回 ToolCalls → 新工具被调用
      → tool_executor.execute(ctx, name, args)
        → GovernedToolRegistry.execute_with_context()
          → blocklist check                            ← 治理检查（policy evaluation 盲区）
          → ToolRegistry.execute() → tool.execute()    ← 实际执行
```

---

## Files to Modify

| File | Change | Rationale |
|------|--------|-----------|
| `crates/torque-kernel/Cargo.toml` | 修改 | 添加 `async-trait`、`anyhow` 依赖 |
| `crates/torque-kernel/src/tool.rs` | 新建 | 定义 `Tool` trait、`ToolResult`、`ToolArc` |
| `crates/torque-kernel/src/lib.rs` | 修改 | 暴露 `tool` 模块和 `pub use` 导出 |
| `crates/torque-extension/src/actor.rs` | 修改 | 添加 `fn tools(&self) -> Vec<ToolArc>` 默认方法 |
| `crates/torque-extension/src/lib.rs` | 修改 | 确认导出 |
| `crates/torque-harness/src/tools/mod.rs` | 修改 | 改为 `pub use torque_kernel::tool::*`，保留 `pub mod` |
| `crates/torque-harness/src/infra/tool_registry.rs` | 修改 | 添加 `remove()`、`update()` 方法 |
| `crates/torque-harness/src/service/tool.rs` | 修改 | 添加 `register_tool()`、`unregister_tool()`、`list_tools()` |
| `crates/torque-harness/src/service/mod.rs` | 修改 | 向 `ExtensionService` 注入 `ToolService` |
| `crates/torque-harness/src/extension/service.rs` | 修改 | 添加 `tool_service` 字段、`tool_map`、工具注册/清理逻辑 |
| `crates/torque-harness/src/api/v1/mod.rs` | 修改 | 挂载新路由 |
| `crates/torque-harness/src/api/v1/tools.rs` | 新建 | 工具 API handler（POST/DELETE/GET/PUT） |
| `crates/torque-harness/src/models/v1/mod.rs` | 修改 | 添加工具注册请求/响应结构体 |
| `crates/torque-harness/src/app.rs` | 修改 | 确认装配路径 |

---

## Known Limitations and Technical Debt

1. **Policy evaluation 盲区**：`HarnessToolExecutor::execute()` 传入 `policy_sources = None`，导致 `GovernedToolRegistry` 只执行 blocklist 检查，不执行 policy evaluation。不影响工具注册本身，但影响治理完整性。需后续独立计划处理（Phase 6.2 的备选方案）。

2. **`StreamEvent::ToolResult` 字段重复**：`src/agent/stream.rs:18-23` 的 `StreamEvent::ToolResult` 变体内联定义了与 `ToolResult` 结构体重复的字段（`success`、`content`、`error`）。提升到 `torque-kernel` 后可选择重构为 `StreamEvent::ToolResult(ToolResult)`，但非本计划必选。列为低优先级清理项。

---

## Potential Risks and Mitigations

1. **工具名冲突（两个 Extension 注册同名工具）**
   Mitigation：`register_tool()` 检测名称冲突，返回 `Err` 而非静默覆盖。使用 `PUT` 语义明确表明更新意图。`tool_map` 的 `ExtensionId ↔ tool_names` 映射可审计冲突来源。

2. **恶意 Extension 注册高风险工具绕过治理**
   Mitigation：所有工具经过 blocklist 检查。系统级 deny list 在 `ToolGovernanceConfig` 中优先级最高。policy evaluation 盲区（见 Known Limitations）需后续填补。

3. **Extension 非正常断开（崩溃）后工具未清理**
   Mitigation：在 `ExtensionService::unregister()` 中自动清理。对于非正常断开，可后续引入心跳机制超时清理。

4. **22 个导入站点的迁移风险**
   Mitigation：采用 re-export 策略（Phase 1.2），零改动全部现有导入路径。第一阶段不修改任何现有 `use crate::tools::{Tool, ToolArc, ToolResult}` 语句。

5. **并发注册/卸载导致 LLM 调用读到不一致状态**
   Mitigation：`ToolRegistry` 使用 `RwLock`，读操作（list）不会被写操作（register/remove）阻塞，始终保证一致快照。Rust 的所有权系统保证引用安全。

---

## 实施状态

**全部 8 个 Phase 已于 2026-05-01 完成。**

| Phase | 状态 | 说明 |
|-------|------|------|
| Phase 1 | DONE | Tool trait 提升到 torque-kernel，re-export 策略，ExtensionActor 默认方法 |
| Phase 2 | DONE | ToolRegistry 添加 remove() / update() CRUD |
| Phase 3 | DONE | ToolService 添加 register_tool / unregister_tool / update_tool / get_tool / list_tools |
| Phase 4 | DONE | ExtensionService 集成 ToolService 注入 + 自动注册/清理 |
| Phase 5 | DONE | HTTP API：POST register / DELETE unregister / GET list / PUT update |
| Phase 6 | DONE | 治理覆盖确认（blocklist 自动覆盖，policy 盲区记入技术债务） |
| Phase 7 | DONE | 三层无缓存路径审查（AgentLoop、RuntimeHost、GovernedToolRegistry） |
| Phase 8 | DONE | 20 个新测试（15 单元 + 5 集成）全部通过 |

### 测试结果汇总

| 测试集 | 用例数 | 结果 |
|--------|--------|------|
| torque-kernel | 34 | 全部通过 |
| torque-harness (lib) | 13 | 全部通过 |
| dynamic_tool_registration_tests | 15 | 全部通过 |
| dynamic_tool_integration_tests | 5 | 全部通过 |
| runtime_adapter_tests | 4 | 全部通过 |
| torque-extension | 216 | 全部通过 |

### 已知技术债务

1. Policy evaluation 盲区：`HarnessToolExecutor::execute()` 传 `None` 给 `policy_sources`
2. `ToolResult` 字段 `success` 和 `error` 为 Option 但不互斥
