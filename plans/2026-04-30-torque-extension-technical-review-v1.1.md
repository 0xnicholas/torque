# Torque Extension Actor 系统 - 技术评审文档

**版本**: v1.1  
**日期**: 2026-04-30  
**状态**: 评审中 (v1.1: 已修正 Crate 依赖关系)

---

## 目录

1. [背景与目标](#1-背景与目标)
2. [技术方案](#2-技术方案)
3. [API 设计](#3-api-设计)
4. [核心类型定义](#4-核心类型定义)
5. [生命周期与状态机](#5-生命周期与状态机)
6. [性能与安全约束](#6-性能与安全约束)
7. [开发计划](#7-开发计划)
8. [评审要点](#8-评审要点)
9. [开放问题](#9-开放问题)
10. [后续扩展](#10-后续扩展)

---

## 1. 背景与目标

### 1.1 现状

| 问题 | 说明 |
|------|------|
| 无插件机制 | Torque 目前没有插件扩展机制 |
| 无法开发扩展 | 第三方无法基于 Torque 开发扩展 |
| 扩展间无法通信 | 扩展之间没有通信方式 |

### 1.2 目标

在 Torque 中实现 Actor 模式的扩展系统：

- **Extension as Actor**: 每个扩展是一个独立的 Actor
- **消息驱动**: 扩展间通过 Mailbox 传递消息
- **Hook 机制**: 扩展可挂载到 Torque 执行流程的关键点
- **可观测性**: 支持日志、指标、追踪
- **独立运行**: Harness 可不带 Extension 功能独立运行

### 1.3 设计原则

1. **复用现有架构**: Extension 复用 `AgentInstance` 模式
2. **分层解耦**: `torque-extension` crate 独立于 `torque-harness`
3. **Feature-Gated**: Extension 功能可选编译，Harness 可独立运行
4. **渐进式实现**: 从单机内存开始，逐步扩展
5. **向后兼容**: 不影响现有 Torque 功能

---

## 2. 技术方案

### 2.1 为什么选择 Actor 模型

| 候选方案 | 优势 | 劣势 | 决策 |
|----------|------|------|------|
| Actor | 状态隔离、消息驱动、适合分布式 | 异步复杂性 | **采用** |
| Event Bus | 实现简单 | 耦合度高、难以隔离 | 放弃 |
| Shared State | 实现简单 | 扩展性差、竞态风险 | 放弃 |

**采用 Actor 的理由**:

1. **复用现有模式**: `AgentInstance` 本身就是 Actor 模型
2. **状态隔离**: 每个 Extension 有独立的 Mailbox 和状态
3. **多种通信模式**: 自然支持 Request/Reply + Pub/Sub
4. **易于恢复**: 可复用 Checkpoint 机制

### 2.2 Crate 结构

**核心原则**: Harness 可不带 Extension 功能独立运行

```
torque-kernel (无依赖)
       │
       ▼
torque-runtime (依赖 kernel)
       │
       ▼
torque-harness ───────────────┐
       │                       │ optional (feature-gated)
       │                       ▼
       │              torque-extension
       │                 (独立 crate)
       │
       ▼
  [可独立运行]
```

#### Feature-Gated 设计

```toml
# torque-harness/Cargo.toml
[dependencies]
torque-runtime = { path = "../torque-runtime" }

# 可选依赖 (默认不启用)
torque-extension = { path = "../torque-extension", optional = true }

[features]
default = []
extension = ["torque-extension"]  # 启用 Extension 功能
```

#### 代码结构

```rust
// torque-harness/src/service/mod.rs

// Extension 功能是可选的
#[cfg(feature = "extension")]
mod extension_service;

pub struct ServiceContainer {
    // 核心服务 (始终存在)
    agent_registry: Arc<AgentRegistry>,
    capability_registry: Arc<CapabilityRegistry>,
    // ...
    
    // Extension 功能 (可选)
    #[cfg(feature = "extension")]
    extension: Option<Arc<ExtensionService>>,
}

impl ServiceContainer {
    // 核心方法始终可用
    pub fn new(/* ... */) -> Self { /* ... */ }
    
    // Extension 方法仅在启用 feature 时可用
    #[cfg(feature = "extension")]
    pub fn extension_service(&self) -> Option<&Arc<ExtensionService>> {
        self.extension.as_ref()
    }
}
```

#### 运行模式

| 模式 | 编译方式 | 功能 |
|------|----------|------|
| 基础模式 | `cargo build` | Harness 独立运行，无 Extension |
| 完整模式 | `cargo build --features extension` | Harness + Extension 功能 |

| Crate | 职责 | 依赖 |
|-------|------|------|
| `torque-kernel` | 核心类型，无外部依赖 | 无 |
| `torque-runtime` | 运行时环境 | kernel |
| `torque-harness` | Harness 核心，可独立运行 | runtime |
| `torque-extension` | Extension 抽象和 Runtime | kernel, 可选被 harness 依赖 |

### 2.3 Extension 与 Torque 交互边界

```
Extension ──→ Hook 点 ──→ Torque 执行流程
Extension ──→ 消息 ──→ 其他 Extension
Extension ──→ 事件 ──→ 外部系统

不在边界内:
✗ Extension 不直接操作数据库
✗ Extension 不直接调用 LLM
✗ Extension 不直接访问文件系统
```

### 2.4 通信模式

| 模式 | 说明 | 用例 |
|------|------|------|
| Fire-and-Forget | 发送后不等待响应 | 日志、指标 |
| Request/Reply | 同步等待响应，超时返回错误 | 查询、调用 |
| Pub/Sub | 异步分发，不等待处理 | 事件通知 |

---

## 3. API 设计

### 3.1 REST API

#### Extension 管理

```http
### 注册 Extension
POST /v1/extensions
Content-Type: application/json

{
  "name": "my-extension",
  "version": "1.0.0",
  "hooks": ["post_execution", "on_tool_call"],
  "config": {}
}

Response: 201 Created
{
  "id": "uuid",
  "name": "my-extension",
  "status": "registered"
}

### 列出所有 Extension
GET /v1/extensions

Response: 200 OK
{
  "extensions": [...]
}

### 获取 Extension 信息
GET /v1/extensions/{id}

### 注销 Extension
DELETE /v1/extensions/{id}
```

#### 消息传递

```http
### 发送消息
POST /v1/extensions/{id}/messages
Content-Type: application/json

{
  "type": "command",
  "action": {
    "type": "execute",
    "goal": "analyze this"
  }
}

Response: 202 Accepted
{
  "correlation_id": "uuid"
}
```

#### 订阅管理

```http
### 订阅主题
POST /v1/extensions/{id}/subscriptions
Content-Type: application/json

{
  "topic": "torque:execution.completed"
}

### 取消订阅
DELETE /v1/extensions/{id}/subscriptions/{topic}
```

#### Hook 管理

```http
### 注册 Hook
POST /v1/extensions/{id}/hooks
Content-Type: application/json

{
  "hook_point": "post_execution"
}

### 注销 Hook
DELETE /v1/extensions/{id}/hooks/{hook_point}
```

### 3.2 Rust API

#### ExtensionActor Trait

```rust
#[async_trait]
pub trait ExtensionActor: Send + Sync {
    fn id(&self) -> ExtensionId;
    fn name(&self) -> &'static str;
    fn version(&self) -> ExtensionVersion;
    
    /// 声明关心的 Hook 点
    fn hook_points(&self) -> Vec<HookPoint> {
        vec![]
    }
    
    /// Extension 启动时调用
    async fn on_start(&self, ctx: &ExtensionContext) -> Result<()>;
    
    /// Extension 停止时调用
    async fn on_stop(&self, ctx: &ExtensionContext) -> Result<()>;
    
    /// 处理收到的消息
    async fn handle(
        &self,
        ctx: &ExtensionContext,
        msg: ExtensionMessage,
    ) -> Result<ExtensionResponse>;
}
```

#### ExtensionContext

```rust
impl ExtensionContext {
    /// Extension 自己的 ID
    pub fn id(&self) -> ExtensionId;
    
    /// 发送消息给其他 Extension (Fire-and-Forget)
    pub async fn send(&self, target: ExtensionId, msg: ExtensionMessage) -> Result<()>;
    
    /// 发送请求并等待响应
    pub async fn call(&self, target: ExtensionId, req: ExtensionRequest) -> Result<ExtensionResponse>;
    
    /// 发布事件到指定主题
    pub async fn publish(&self, topic: ExtensionTopic, event: ExtensionEvent) -> Result<()>;
    
    /// 订阅主题
    pub fn subscribe(&self, topic: ExtensionTopic, handler: Arc<dyn EventHandler>) -> Result<()>;
    
    /// 状态管理
    pub fn get_state(&self, key: &str) -> Option<serde_json::Value>;
    pub fn set_state(&self, key: &str, value: serde_json::Value);
}
```

#### ExtensionRuntime Trait

```rust
#[async_trait]
pub trait ExtensionRuntime: Send + Sync {
    /// 注册并启动 Extension
    async fn register(&self, extension: Arc<dyn ExtensionActor>) -> Result<ExtensionId>;
    
    /// 注销 Extension
    async fn unregister(&self, id: ExtensionId) -> Result<()>;
    
    /// 暂停 Extension
    async fn suspend(&self, id: ExtensionId) -> Result<()>;
    
    /// 恢复 Extension
    async fn resume(&self, id: ExtensionId) -> Result<()>;
    
    /// 发送消息
    async fn send(&self, target: ExtensionId, msg: ExtensionMessage) -> Result<()>;
    
    /// 发送请求并等待响应
    async fn call(&self, target: ExtensionId, req: ExtensionRequest) -> Result<ExtensionResponse>;
    
    /// 发布事件
    async fn publish(&self, topic: ExtensionTopic, event: ExtensionEvent) -> Result<()>;
    
    /// 订阅/取消订阅
    async fn subscribe(&self, id: ExtensionId, topic: ExtensionTopic) -> Result<()>;
    async fn unsubscribe(&self, id: ExtensionId, topic: ExtensionTopic) -> Result<()>;
    
    /// 查询
    fn find(&self, name: &str) -> Option<ExtensionId>;
    fn list(&self) -> Vec<ExtensionId>;
    fn lifecycle(&self, id: ExtensionId) -> Option<ExtensionLifecycle>;
}
```

---

## 4. 核心类型定义

### 4.1 ID 类型

```rust
/// Extension 唯一标识 (基于 AgentInstanceId)
pub struct ExtensionId(AgentInstanceId);

/// Extension 语义化版本
pub struct ExtensionVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub pre: Option<String>,
}
```

### 4.2 Hook 点

```rust
pub enum HookPoint {
    /// Agent 执行前/后
    PreExecution,
    PostExecution,
    
    /// Tool 调用前/后/拦截
    PreToolCall,
    PostToolCall,
    InterceptToolCall,
    
    /// Delegation 创建前/后
    PreDelegation,
    PostDelegation,
    
    /// 其他事件
    OnArtifactCreated,
    OnApprovalRequested,
    OnCheckpoint,
    OnRecovery,
    OnTeamEvent,
    OnAgentStateChanged,
}

/// Hook 执行结果
pub enum HookResult {
    Continue,                      // 继续执行
    Blocked { reason: String },   // 阻止执行
    Modified(serde_json::Value), // 修改后继续
    ShortCircuit(serde_json::Value), // 短路返回
}
```

### 4.3 消息类型

```rust
#[serde(tag = "type")]
pub enum ExtensionMessage {
    /// 发送命令 (Fire-and-Forget)
    Command {
        action: ExtensionAction,
        correlation_id: Option<Uuid>,
    },
    
    /// 请求-响应
    Request {
        request_id: Uuid,
        action: ExtensionAction,
        reply_to: Uuid,
        timeout_ms: Option<u64>,
    },
    
    /// 包装 DelegationRequest
    Delegation(Box<DelegationRequest>),
}

pub enum ExtensionAction {
    Execute { goal: String, instructions: Vec<String> },
    Query { key: String },
    SetState { key: String, value: serde_json::Value },
    Publish { topic: ExtensionTopic, event: ExtensionEvent },
    Subscribe { topic: ExtensionTopic },
    Unsubscribe { topic: ExtensionTopic },
    Custom { namespace: String, name: String, payload: serde_json::Value },
}
```

### 4.4 主题

```rust
pub struct ExtensionTopic {
    namespace: Arc<str>,  // 如 "torque"
    name: Arc<str>,       // 如 "execution.completed"
    version: u32,
}

// 预定义主题
pub mod topics {
    pub fn execution_started() -> ExtensionTopic;
    pub fn execution_completed() -> ExtensionTopic;
    pub fn execution_failed() -> ExtensionTopic;
    pub fn tool_called() -> ExtensionTopic;
    pub fn artifact_created() -> ExtensionTopic;
    pub fn delegation_created() -> ExtensionTopic;
    pub fn team_event() -> ExtensionTopic;
}
```

### 4.5 错误类型

```rust
#[derive(Error, Debug)]
pub enum ExtensionError {
    #[error("Extension not found: {0}")]
    NotFound(ExtensionId),
    
    #[error("Extension already registered: {0}")]
    AlreadyRegistered(ExtensionId),
    
    #[error("Message timeout: {0}")]
    Timeout(ExtensionId),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    #[error("Hook point error: {0}")]
    HookPointError(String),
    
    #[error("Lifecycle error: {0}")]
    LifecycleError(String),
    
    #[error("Runtime error: {0}")]
    RuntimeError(String),
    
    #[error("Extension panicked: {0}")]
    Panicked(String),
}
```

---

## 5. 生命周期与状态机

### 5.1 状态转换

```
Loaded ──► Registered ──► Initialized ──► Running
  │            │               │             │
  │            │               │             ▼
  │            │               │         Suspended
  │            │               │             │
  │            │               │             ▼
  │            │               │          Resumed
  │            │               │             │
  │            │               │             ▼
  │            │               │          Stopped
  │            │               │             │
  ▼            ▼               ▼             ▼
Unloaded    Unregistered    Failed       Cleanup
```

### 5.2 生命周期状态

| 状态 | 说明 | 可转换到 |
|------|------|----------|
| Loaded | 已加载但未注册 | Registered |
| Registered | 已注册到 Runtime | Initialized, Unregistered |
| Initialized | 已初始化 | Running, Failed |
| Running | 运行中 | Suspended, Stopped, Failed |
| Suspended | 已暂停 | Running |
| Stopped | 已停止 | Cleanup |
| Failed | 错误状态 | Cleanup |
| Cleanup | 清理完成 | - |

### 5.3 错误处理策略

| 场景 | 处理策略 |
|------|----------|
| Extension panic | 隔离，不影响其他 Extension |
| 消息超时 | 默认 30 秒，返回 Timeout 错误 |
| Hook 失败 | 记录日志，继续执行 (可配置为阻止) |

---

## 6. 性能与安全约束

### 6.1 性能目标

| 指标 | 目标 |
|------|------|
| Extension 消息延迟 | < 10ms (同进程) |
| Hook 触发开销 | < 1ms |
| 支持并发 Extension 数 | 100+ |
| Mailbox 默认容量 | 1000 条消息 |
| 默认消息超时 | 30 秒 |

### 6.2 安全约束

| 阶段 | 约束 |
|------|------|
| 初版 (Phase 1-5) | 进程内运行，无沙箱隔离 |
| 未来 (Phase 6+) | 进程隔离、权限控制、资源限制 |

**当前安全保证**: 依赖 Rust 类型系统和内存安全

### 6.3 可观测性

- Extension 生命周期事件日志
- 消息流量监控
- Hook 执行指标

---

## 7. 开发计划

### 7.1 里程碑

| 阶段 | 里程碑 | 预估时间 | 交付物 |
|------|--------|----------|--------|
| **Phase 0** | 预研完成 | 1 周 | 技术方案、API 设计 |
| **Phase 1** | 核心抽象 | 2-3 周 | Trait、类型定义 |
| **Phase 2** | Runtime | 2-3 周 | InMemory Runtime |
| **Phase 3** | 集成 | 2-3 周 | Hook、API |
| **Phase 4** | 示例 | 1-2 周 | Logging、Metrics |
| **Phase 5** | 持久化 | 1-2 周 | Snapshot、Recovery |
| **Phase 6** | 分布式 (可选) | 3-4 周 | Remote Runtime |

**总计**: 10-17 周 (不含 Phase 6)

### 7.2 Phase 0 任务 (预研)

| Day | 上午 | 下午 |
|-----|------|------|
| 1 | Actor 方案最终确认 | API 边界确认 |
| 2 | REST API 评审 | Rust API 评审 |
| 3 | 生命周期/状态机 | 性能/安全考量 |
| 4 | 编写评审报告 | Phase 1 启动准备 |
| 5 | 内部评审会议 | 最终交付 |

### 7.3 Phase 1 任务 (核心抽象)

- [ ] 创建 `crates/torque-extension/Cargo.toml`
- [ ] 定义 `ExtensionId`, `ExtensionVersion`
- [ ] 定义 `ExtensionActor` trait
- [ ] 定义 `ExtensionContext`
- [ ] 定义消息类型 (`ExtensionMessage`, `ExtensionAction`, `ExtensionEvent`)
- [ ] 定义 `HookPoint`, `HookResult`
- [ ] 定义 `ExtensionTopic`, `ExtensionState`
- [ ] 定义 `ExtensionError`
- [ ] 编写单元测试

### 7.4 Phase 2 任务 (Runtime)

- [ ] 定义 `ExtensionRuntime` trait
- [ ] 实现 `InMemoryExtensionRuntime`
- [ ] 实现 `Mailbox`
- [ ] 实现消息路由
- [ ] 实现主题订阅管理
- [ ] 编写集成测试

---

## 8. 评审要点

### 8.1 技术方案评审

- [ ] Actor 模型是否适合 Torque 场景？
- [ ] crate 结构是否清晰？
- [ ] Feature-Gated 设计是否满足独立运行需求？
- [ ] 依赖关系是否正确？

### 8.2 API 设计评审

| 问题 | 影响 | 待确认 |
|------|------|--------|
| Hook 合并策略 | 高 | 多个 Extension 注册同一 Hook 如何合并结果？ |
| 配置热更新 | 中 | 是否需要支持运行时配置更新？ |
| 扩展版本升级 | 中 | 如何处理 Extension 版本升级？ |

**预选方案**:
- Hook 合并: 最严格者优先 (Blocked > Modified > Continue)
- 配置热更新: 通过 API 支持
- 版本升级: 版本协商机制

### 8.3 Rust API 评审

- [ ] `on_start` 是否应该是同步的？
- [ ] 是否需要 `on_error` 回调？
- [ ] 是否需要 `on_config_update` 回调？
- [ ] `Modified` 的修改范围是什么？
- [ ] `ShortCircuit` 的返回值如何处理？

### 8.4 可行性评审

- [ ] 技术复杂度是否可接受？
- [ ] 开发周期是否合理？
- [ ] 风险是否已识别？
- [ ] 是否有备选方案？

---

## 9. 开放问题

| # | 问题 | 影响 | 处理方案 | 状态 |
|---|------|------|----------|------|
| O1 | Hook 合并策略 | 高 | 最严格者优先 | 待评审 |
| O2 | 配置热更新 | 中 | 通过 API 支持 | 待确认 |
| O3 | 扩展版本升级 | 中 | 需版本协商 | 待设计 |
| O4 | 资源配额限制 | 低 | Phase 6+ | 延期 |
| O5 | Extension 优先级 | 低 | 暂不支持 | 延期 |

---

## 10. 后续扩展

| 方向 | 说明 |
|------|------|
| 动态加载 | 运行时加载 .so 扩展 |
| 安全沙箱 | Extension 权限控制 |
| 可视化 | Extension 状态监控面板 |
| Marketplace | 扩展市场 |

---

## 附录: 文件结构

```
crates/torque-extension/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── id.rs
    ├── actor.rs
    ├── context.rs
    ├── message.rs
    ├── error.rs
    ├── hook.rs
    ├── topic.rs
    ├── state.rs
    └── runtime/
        ├── mod.rs
        ├── trait.rs
        ├── in_memory.rs
        └── mailbox.rs

crates/torque-harness/src/
├── service/
│   ├── mod.rs          # ServiceContainer (含 feature-gated Extension)
│   └── ...
└── api/
    ├── mod.rs
    └── extension/      # Extension API 路由
```

---

## 评审 Checklist (会议使用)

### 决策项

- [x] ~~确认采用 Actor 模型~~ (会议中确认)
- [x] 确认 crate 结构 (Feature-Gated)
- [x] 确认 API 边界
- [x] 确认 Hook 合并策略
- [x] 确认 Phase 1 验收标准

### 评审结果

| 项目 | 通过 | 需修改 | 备注 |
|------|------|--------|------|
| 技术方案 | ☑ | ☐ | v1.1: 已修正 Crate 依赖关系 |
| API 设计 | ☐ | ☐ | |
| 开发计划 | ☐ | ☐ | |
| 风险评估 | ☐ | ☐ | |

**评审结论**: ________________________

**下一步**: ________________________

---

## 修订历史

| 版本 | 日期 | 变更内容 |
|------|------|----------|
| v1.0 | 2026-04-30 | 初始版本 |
| v1.1 | 2026-04-30 | 修正 Crate 依赖关系: harness 改为 optional 依赖 extension |
