# Torque Extension Actor 系统 - Phase 1 & 2 详细设计

## 概述

本文档细化 Phase 1 (核心抽象层) 和 Phase 2 (Extension Runtime) 的具体实现。

---

## Phase 1: 核心抽象层

### 1.1 Crate 结构

```
crates/torque-extension/
├── Cargo.toml
└── src/
    ├── lib.rs              # 入口，导出公共类型
    ├── id.rs               # ID 类型定义
    ├── actor.rs            # ExtensionActor trait
    ├── context.rs          # ExtensionContext
    ├── message.rs          # 消息类型
    ├── error.rs            # 错误类型
    ├── hook.rs             # HookPoint 定义
    ├── topic.rs            # ExtensionTopic
    └── state.rs            # ExtensionState
```

### 1.2 Cargo.toml

```toml
[package]
name = "torque-extension"
version.workspace = true
edition.workspace = true

[dependencies]
# 核心依赖
torque-kernel = { path = "../torque-kernel" }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["v4", "serde"] }
thiserror = "1"
tracing = "0.1"
async-trait = "0.1"
futures = "0.3"

# 可选依赖 (feature-gated)
tracing-subscriber = { version = "0.3", optional = true }
prometheus = { version = "0.13", optional = true }

[features]
default = []
tracing = ["tracing-subscriber"]
metrics = ["prometheus"]
```

### 1.3 ID 类型 (src/id.rs)

```rust
use torque_kernel::AgentInstanceId;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Extension 唯一标识
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExtensionId(AgentInstanceId);

impl ExtensionId {
    pub fn new() -> Self {
        Self(AgentInstanceId::new())
    }
    
    pub fn from_uuid(uuid: uuid::Uuid) -> Self {
        Self(AgentInstanceId::from(uuid))
    }
    
    pub fn as_uuid(&self) -> uuid::Uuid {
        self.0.as_uuid()
    }
}

impl Default for ExtensionId {
    fn default() -> Self {
        Self::new()
    }
}

/// Extension 语义化版本
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub pre: Option<String>,
}

impl ExtensionVersion {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self { major, minor, patch, pre: None }
    }
    
    pub fn with_pre(mut self, pre: impl Into<String>) -> Self {
        self.pre = Some(pre.into());
        self
    }
    
    pub fn is_compatible_with(&self, other: &ExtensionVersion) -> bool {
        self.major == other.major
    }
}

impl std::fmt::Display for ExtensionVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.pre {
            Some(pre) => write!(f, "{}.{}.{}-{}", self.major, self.minor, self.patch, pre),
            None => write!(f, "{}.{}.{}", self.major, self.minor, self.patch),
        }
    }
}
```

### 1.4 Topic 类型 (src/topic.rs)

```rust
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;

/// Extension 事件主题
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExtensionTopic {
    namespace: Arc<str>,
    name: Arc<str>,
    version: u32,
}

impl ExtensionTopic {
    pub fn new(namespace: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into().into(),
            name: name.into().into(),
            version: 1,
        }
    }
    
    pub fn with_version(mut self, version: u32) -> Self {
        self.version = version;
        self
    }
    
    /// 解析通配符主题，如 "torque.*"
    pub fn matches(&self, pattern: &ExtensionTopic) -> bool {
        if pattern.namespace == "*" || pattern.namespace == self.namespace {
            if pattern.name == "*" || pattern.name == self.name {
                return true;
            }
        }
        false
    }
}

impl fmt::Display for ExtensionTopic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}/v{}", self.namespace, self.name, self.version)
    }
}

// 预定义主题常量
pub mod topics {
    use super::ExtensionTopic;
    
    pub fn execution_started() -> ExtensionTopic {
        ExtensionTopic::new("torque", "execution.started")
    }
    
    pub fn execution_completed() -> ExtensionTopic {
        ExtensionTopic::new("torque", "execution.completed")
    }
    
    pub fn execution_failed() -> ExtensionTopic {
        ExtensionTopic::new("torque", "execution.failed")
    }
    
    pub fn tool_called() -> ExtensionTopic {
        ExtensionTopic::new("torque", "tool.called")
    }
    
    pub fn artifact_created() -> ExtensionTopic {
        ExtensionTopic::new("torque", "artifact.created")
    }
    
    pub fn delegation_created() -> ExtensionTopic {
        ExtensionTopic::new("torque", "delegation.created")
    }
    
    pub fn team_event() -> ExtensionTopic {
        ExtensionTopic::new("torque", "team.event")
    }
}
```

### 1.5 Hook Point (src/hook.rs)

```rust
use serde::{Deserialize, Serialize};

/// Extension 可挂载的 Hook 点
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HookPoint {
    /// Agent 执行前
    PreExecution,
    /// Agent 执行后
    PostExecution,
    /// Tool 调用前
    PreToolCall,
    /// Tool 调用后
    PostToolCall,
    /// Tool 执行中 (可拦截)
    InterceptToolCall,
    /// Delegation 创建前
    PreDelegation,
    /// Delegation 创建后
    PostDelegation,
    /// Artifact 创建后
    OnArtifactCreated,
    /// Approval 请求时
    OnApprovalRequested,
    /// Checkpoint 创建时
    OnCheckpoint,
    /// Recovery 时
    OnRecovery,
    /// Team 事件
    OnTeamEvent,
    /// Agent 状态变更
    OnAgentStateChanged,
}

impl HookPoint {
    pub fn category(&self) -> &'static str {
        match self {
            HookPoint::PreExecution | HookPoint::PostExecution => "execution",
            HookPoint::PreToolCall | HookPoint::PostToolCall | HookPoint::InterceptToolCall => "tool",
            HookPoint::PreDelegation | HookPoint::PostDelegation => "delegation",
            HookPoint::OnArtifactCreated => "artifact",
            HookPoint::OnApprovalRequested => "approval",
            HookPoint::OnCheckpoint | HookPoint::OnRecovery => "recovery",
            HookPoint::OnTeamEvent => "team",
            HookPoint::OnAgentStateChanged => "state",
        }
    }
}

/// Hook 执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HookResult {
    /// 继续执行
    Continue,
    /// 阻止执行
    Blocked { reason: String },
    /// 修改后的上下文
    Modified(serde_json::Value),
    /// 短路执行 (用于 Pre* hooks)
    ShortCircuit(serde_json::Value),
}

/// Hook 上下文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookContext {
    pub extension_id: ExtensionId,
    pub hook_point: HookPoint,
    pub instance_id: Option<AgentInstanceId>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub metadata: serde_json::Value,
}

#[cfg(feature = "kernel-types")]
use torque_kernel::AgentInstanceId;
```

### 1.6 消息类型 (src/message.rs)

```rust
use serde::{Deserialize, Serialize};
use torque_kernel::DelegationRequest;
use uuid::Uuid;

/// Extension 间消息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ExtensionMessage {
    /// 发送命令 (Fire-and-Forget)
    Command {
        action: ExtensionAction,
        #[serde(default)]
        correlation_id: Option<Uuid>,
    },
    
    /// 请求-响应
    Request {
        request_id: Uuid,
        action: ExtensionAction,
        reply_to: Uuid,  // 响应目标 mailbox
        timeout_ms: Option<u64>,
    },
    
    /// 直接包装 DelegationRequest (复用 Kernel 类型)
    Delegation(Box<DelegationRequest>),
}

/// Extension 执行动作
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action")]
pub enum ExtensionAction {
    /// 执行目标
    Execute {
        goal: String,
        instructions: Vec<String>,
    },
    
    /// 查询状态
    Query { key: String },
    
    /// 设置状态
    SetState { key: String, value: serde_json::Value },
    
    /// 发布事件
    Publish { topic: ExtensionTopic, event: ExtensionEvent },
    
    /// 订阅主题
    Subscribe { topic: ExtensionTopic },
    
    /// 取消订阅
    Unsubscribe { topic: ExtensionTopic },
    
    /// 注册 Hook
    RegisterHook { hook_point: HookPoint },
    
    /// 注销 Hook
    UnregisterHook { hook_point: HookPoint },
    
    /// 扩展特定动作
    Custom {
        namespace: String,
        name: String,
        payload: serde_json::Value,
    },
}

impl ExtensionAction {
    pub fn namespace(&self) -> Option<&str> {
        match self {
            ExtensionAction::Custom { namespace, .. } => Some(namespace),
            _ => None,
        }
    }
}

/// Extension 事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionEvent {
    pub id: Uuid,
    pub topic: ExtensionTopic,
    pub source: ExtensionId,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub payload: serde_json::Value,
    pub correlation_id: Option<Uuid>,
}

impl ExtensionEvent {
    pub fn new(topic: ExtensionTopic, source: ExtensionId, payload: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            topic,
            source,
            timestamp: chrono::Utc::now(),
            payload,
            correlation_id: None,
        }
    }
    
    pub fn with_correlation(mut self, correlation_id: Uuid) -> Self {
        self.correlation_id = Some(correlation_id);
        self
    }
}

/// Extension 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionResponse {
    pub request_id: Uuid,
    pub status: ResponseStatus,
    pub result: Option<serde_json::Value>,
    pub error: Option<ExtensionErrorInfo>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ResponseStatus {
    Success,
    Failure,
    Timeout,
    NotFound,
}

/// Extension 错误信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionErrorInfo {
    pub code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
}
```

### 1.7 错误类型 (src/error.rs)

```rust
use thiserror::Error;
use crate::id::ExtensionId;

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
    
    #[error("Topic error: {0}")]
    TopicError(String),
    
    #[error("State error: {0}")]
    StateError(String),
    
    #[error("Runtime error: {0}")]
    RuntimeError(String),
    
    #[error("Extension panicked: {0}")]
    Panicked(String),
}

impl ExtensionError {
    pub fn code(&self) -> &'static str {
        match self {
            ExtensionError::NotFound(_) => "EXT_NOT_FOUND",
            ExtensionError::AlreadyRegistered(_) => "EXT_ALREADY_REGISTERED",
            ExtensionError::Timeout(_) => "EXT_TIMEOUT",
            ExtensionError::SerializationError(_) => "EXT_SERIALIZATION_ERROR",
            ExtensionError::HookPointError(_) => "EXT_HOOK_ERROR",
            ExtensionError::LifecycleError(_) => "EXT_LIFECYCLE_ERROR",
            ExtensionError::TopicError(_) => "EXT_TOPIC_ERROR",
            ExtensionError::StateError(_) => "EXT_STATE_ERROR",
            ExtensionError::RuntimeError(_) => "EXT_RUNTIME_ERROR",
            ExtensionError::Panicked(_) => "EXT_PANICKED",
        }
    }
}

pub type Result<T> = std::result::Result<T, ExtensionError>;
```

### 1.8 State 类型 (src/state.rs)

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;
use std::sync::Arc;

/// Extension 私有状态
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtensionState {
    /// 键值存储
    values: HashMap<String, serde_json::Value>,
    /// 版本号 (乐观锁)
    version: u64,
}

impl ExtensionState {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn get(&self, key: &str) -> Option<&serde_json::Value> {
        self.values.get(key)
    }
    
    pub fn set(&mut self, key: impl Into<String>, value: serde_json::Value) {
        self.values.insert(key.into(), value);
        self.version += 1;
    }
    
    pub fn remove(&mut self, key: &str) -> Option<serde_json::Value> {
        self.values.remove(key)
    }
    
    pub fn version(&self) -> u64 {
        self.version
    }
    
    pub fn clear(&mut self) {
        self.values.clear();
        self.version += 1;
    }
}

/// 线程安全的 State 包装
pub struct SharedExtensionState {
    inner: Arc<RwLock<ExtensionState>>,
}

impl SharedExtensionState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(ExtensionState::new())),
        }
    }
    
    pub fn get(&self, key: &str) -> Option<serde_json::Value> {
        self.inner.read().unwrap().get(key).cloned()
    }
    
    pub fn set(&self, key: impl Into<String>, value: serde_json::Value) {
        self.inner.write().unwrap().set(key, value);
    }
    
    pub fn snapshot(&self) -> ExtensionState {
        self.inner.read().unwrap().clone()
    }
    
    pub fn restore(&self, state: ExtensionState) {
        *self.inner.write().unwrap() = state;
    }
}

impl Clone for SharedExtensionState {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}
```

### 1.9 ExtensionActor Trait (src/actor.rs)

```rust
use async_trait::async_trait;
use crate::{
    ExtensionId,
    ExtensionVersion,
    ExtensionContext,
    ExtensionMessage,
    ExtensionResponse,
    HookPoint,
    Result,
};

/// Extension Actor 主 Trait
#[async_trait]
pub trait ExtensionActor: Send + Sync {
    /// Extension 唯一 ID
    fn id(&self) -> ExtensionId;
    
    /// Extension 名称
    fn name(&self) -> &'static str;
    
    /// Extension 版本
    fn version(&self) -> ExtensionVersion;
    
    /// Extension 描述
    fn description(&self) -> &'static str {
        ""
    }
    
    /// 声明关心的 Hook 点
    fn hook_points(&self) -> Vec<HookPoint> {
        vec![]
    }
    
    /// Extension 启动时调用
    async fn on_start(&self, ctx: &ExtensionContext) -> Result<()> {
        let _ = ctx;
        Ok(())
    }
    
    /// Extension 停止时调用
    async fn on_stop(&self, ctx: &ExtensionContext) -> Result<()> {
        let _ = ctx;
        Ok(())
    }
    
    /// Extension 暂停时调用 (可选)
    async fn on_suspend(&self, _ctx: &ExtensionContext) -> Result<()> {
        Ok(())
    }
    
    /// Extension 恢复时调用 (可选)
    async fn on_resume(&self, _ctx: &ExtensionContext) -> Result<()> {
        Ok(())
    }
    
    /// 处理收到的消息
    async fn handle(
        &self,
        ctx: &ExtensionContext,
        msg: ExtensionMessage,
    ) -> Result<ExtensionResponse>;
    
    /// 处理 Hook (如果注册了 Hook 点)
    async fn on_hook(
        &self,
        ctx: &ExtensionContext,
        hook_point: HookPoint,
        context: serde_json::Value,
    ) -> Result<HookResult> {
        let _ = (ctx, hook_point, context);
        Ok(HookResult::Continue)
    }
}

/// Extension 生命周期状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionLifecycle {
    /// 已加载但未注册
    Loaded,
    /// 已注册到 Runtime
    Registered,
    /// 已初始化
    Initialized,
    /// 运行中
    Running,
    /// 已暂停
    Suspended,
    /// 已停止
    Stopped,
    /// 错误状态
    Failed,
}

impl ExtensionLifecycle {
    pub fn can_transition_to(&self, next: ExtensionLifecycle) -> bool {
        match (self, next) {
            (ExtensionLifecycle::Loaded, ExtensionLifecycle::Registered) => true,
            (ExtensionLifecycle::Registered, ExtensionLifecycle::Initialized) => true,
            (ExtensionLifecycle::Initialized, ExtensionLifecycle::Running) => true,
            (ExtensionLifecycle::Running, ExtensionLifecycle::Suspended) => true,
            (ExtensionLifecycle::Suspended, ExtensionLifecycle::Running) => true,
            (ExtensionLifecycle::Running, ExtensionLifecycle::Stopped) => true,
            (ExtensionLifecycle::Initialized, ExtensionLifecycle::Failed) => true,
            (ExtensionLifecycle::Running, ExtensionLifecycle::Failed) => true,
            _ => false,
        }
    }
}
```

### 1.10 ExtensionContext (src/context.rs)

```rust
use std::sync::Arc;
use tokio::sync::mpsc;
use crate::{
    ExtensionId,
    ExtensionTopic,
    ExtensionMessage,
    ExtensionEvent,
    ExtensionRequest,
    ExtensionResponse,
    ExtensionActor,
    state::SharedExtensionState,
    error::{ExtensionError, Result},
};

/// Extension 运行时句柄
pub struct ExtensionContext {
    runtime: Arc<dyn ExtensionRuntimeHandle>,
    self_id: ExtensionId,
    state: SharedExtensionState,
    event_emitter: Arc<dyn EventEmitter>,
}

/// Extension Runtime 内部接口
pub trait ExtensionRuntimeHandle: Send + Sync {
    fn id(&self) -> ExtensionId;
    
    /// 发送消息 (Fire-and-Forget)
    async fn send(&self, target: ExtensionId, msg: ExtensionMessage) -> Result<()>;
    
    /// 发送请求并等待响应
    async fn call(&self, target: ExtensionId, req: ExtensionRequest) -> Result<ExtensionResponse>;
    
    /// 发布事件
    async fn publish(&self, topic: ExtensionTopic, event: ExtensionEvent) -> Result<()>;
    
    /// 获取 Extension 引用
    fn find_extension(&self, name: &str) -> Option<ExtensionId>;
    
    /// 创建响应 mailbox
    fn create_mailbox(&self) -> (Uuid, mpsc::Sender<ExtensionResponse>);
}

/// 事件发射器接口
pub trait EventEmitter: Send + Sync {
    fn emit(&self, event: ExtensionEvent) -> Result<()>;
    fn subscribe(&self, topic: ExtensionTopic, handler: Arc<dyn EventHandler>) -> Result<()>;
    fn unsubscribe(&self, topic: ExtensionTopic) -> Result<()>;
}

pub trait EventHandler: Send + Sync {
    fn handle(&self, event: ExtensionEvent);
}

impl ExtensionContext {
    pub fn new(
        runtime: Arc<dyn ExtensionRuntimeHandle>,
        self_id: ExtensionId,
    ) -> Self {
        Self {
            runtime,
            self_id,
            state: SharedExtensionState::new(),
            event_emitter: Arc::new(NoOpEventEmitter),
        }
    }
    
    /// Extension 自己的 ID
    pub fn id(&self) -> ExtensionId {
        self.self_id
    }
    
    /// 发送消息给其他 Extension (Fire-and-Forget)
    pub async fn send(&self, target: ExtensionId, msg: ExtensionMessage) -> Result<()> {
        if target == self.self_id {
            return Err(ExtensionError::RuntimeError("Cannot send to self".into()));
        }
        self.runtime.send(target, msg).await
    }
    
    /// 发送请求并等待响应
    pub async fn call(&self, target: ExtensionId, req: ExtensionRequest) -> Result<ExtensionResponse> {
        if target == self.self_id {
            return Err(ExtensionError::RuntimeError("Cannot call self".into()));
        }
        self.runtime.call(target, req).await
    }
    
    /// 发布事件到指定主题
    pub async fn publish(&self, topic: ExtensionTopic, event: ExtensionEvent) -> Result<()> {
        self.runtime.publish(topic, event).await
    }
    
    /// 订阅主题
    pub fn subscribe(
        &self,
        topic: ExtensionTopic,
        handler: Arc<dyn EventHandler>,
    ) -> Result<()> {
        self.event_emitter.subscribe(topic, handler)
    }
    
    /// 获取状态
    pub fn get_state(&self, key: &str) -> Option<serde_json::Value> {
        self.state.get(key)
    }
    
    /// 设置状态
    pub fn set_state(&self, key: &str, value: serde_json::Value) {
        self.state.set(key, value);
    }
    
    /// 快照状态
    pub fn snapshot_state(&self) -> crate::state::ExtensionState {
        self.state.snapshot()
    }
    
    /// 恢复状态
    pub fn restore_state(&self, state: crate::state::ExtensionState) {
        self.state.restore(state);
    }
}

/// 空实现 EventEmitter
struct NoOpEventEmitter;

impl EventEmitter for NoOpEventEmitter {
    fn emit(&self, _event: ExtensionEvent) -> Result<()> {
        Ok(())
    }
    
    fn subscribe(&self, _topic: ExtensionTopic, _handler: Arc<dyn EventHandler>) -> Result<()> {
        Ok(())
    }
    
    fn unsubscribe(&self, _topic: ExtensionTopic) -> Result<()> {
        Ok(())
    }
}
```

### 1.11 lib.rs 导出

```rust
pub mod id;
pub mod actor;
pub mod context;
pub mod message;
pub mod error;
pub mod hook;
pub mod topic;
pub mod state;

pub use id::{ExtensionId, ExtensionVersion};
pub use actor::{ExtensionActor, ExtensionLifecycle};
pub use context::ExtensionContext;
pub use message::{
    ExtensionMessage,
    ExtensionAction,
    ExtensionEvent,
    ExtensionResponse,
    ExtensionRequest,
    ResponseStatus,
    ExtensionErrorInfo,
};
pub use error::{ExtensionError, Result};
pub use hook::{HookPoint, HookResult, HookContext};
pub use topic::{ExtensionTopic, topics};
pub use state::{ExtensionState, SharedExtensionState};
```

---

## Phase 2: Extension Runtime

### 2.1 Runtime 结构

```
src/
├── runtime/
│   ├── mod.rs
│   ├── trait.rs           # ExtensionRuntime trait
│   ├── in_memory.rs       # InMemoryExtensionRuntime 实现
│   ├── mailbox.rs         # Mailbox 实现
│   ├── router.rs          # 消息路由器
│   └── registry.rs        # Extension 注册表
```

### 2.2 Runtime Trait (src/runtime/trait.rs)

```rust
use std::sync::Arc;
use async_trait::async_trait;
use crate::{
    ExtensionId,
    ExtensionTopic,
    ExtensionActor,
    ExtensionMessage,
    ExtensionRequest,
    ExtensionResponse,
    ExtensionEvent,
    HookPoint,
    state::ExtensionState,
    error::Result,
};

/// Extension Runtime 主接口
#[async_trait]
pub trait ExtensionRuntime: Send + Sync {
    // === 生命周期 ===
    
    /// 注册并启动 Extension
    async fn register(&self, extension: Arc<dyn ExtensionActor>) -> Result<ExtensionId>;
    
    /// 注销 Extension
    async fn unregister(&self, id: ExtensionId) -> Result<()>;
    
    /// 暂停 Extension
    async fn suspend(&self, id: ExtensionId) -> Result<()>;
    
    /// 恢复 Extension
    async fn resume(&self, id: ExtensionId) -> Result<()>;
    
    // === 消息传递 ===
    
    /// 发送消息 (Fire-and-Forget)
    async fn send(&self, target: ExtensionId, msg: ExtensionMessage) -> Result<()>;
    
    /// 发送请求并等待响应
    async fn call(&self, target: ExtensionId, req: ExtensionRequest) -> Result<ExtensionResponse>;
    
    // === 发布/订阅 ===
    
    /// 发布事件到主题
    async fn publish(&self, topic: ExtensionTopic, event: ExtensionEvent) -> Result<()>;
    
    /// 订阅主题
    async fn subscribe(&self, id: ExtensionId, topic: ExtensionTopic) -> Result<()>;
    
    /// 取消订阅
    async fn unsubscribe(&self, id: ExtensionId, topic: ExtensionTopic) -> Result<()>;
    
    // === 查询 ===
    
    /// 通过名称查找 Extension
    fn find(&self, name: &str) -> Option<ExtensionId>;
    
    /// 列出所有 Extension
    fn list(&self) -> Vec<ExtensionId>;
    
    /// 获取 Extension 状态
    fn lifecycle(&self, id: ExtensionId) -> Option<ExtensionLifecycle>;
    
    /// 获取 Extension 快照
    fn snapshot(&self, id: ExtensionId) -> Result<ExtensionState>;
    
    // === Hook ===
    
    /// 注册 Hook
    async fn register_hook(
        &self,
        id: ExtensionId,
        hook_point: HookPoint,
    ) -> Result<()>;
    
    /// 触发 Hook
    async fn trigger_hook(
        &self,
        hook_point: HookPoint,
        context: serde_json::Value,
    ) -> Result<()>;
}
```

### 2.3 Mailbox 实现 (src/runtime/mailbox.rs)

```rust
use tokio::sync::{mpsc, oneshot, watch};
use crate::{
    ExtensionMessage,
    ExtensionResponse,
    ExtensionId,
    error::{ExtensionError, Result},
};
use std::sync::Arc;

/// Mailbox 容量配置
pub struct MailboxConfig {
    pub capacity: usize,
    pub response_capacity: usize,
}

impl Default for MailboxConfig {
    fn default() -> Self {
        Self {
            capacity: 1000,
            response_capacity: 100,
        }
    }
}

/// Extension Mailbox
pub struct Mailbox {
    /// 接收消息的 channel
    receiver: mpsc::Receiver<MailboxMessage>,
    /// Extension ID
    owner: ExtensionId,
    /// 生命周期 watch channel
    lifecycle_rx: watch::Receiver<MailboxLifecycle>,
}

enum MailboxMessage {
    Message(ExtensionMessage),
    Shutdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailboxLifecycle {
    Active,
    Suspended,
    Stopped,
}

impl Mailbox {
    pub fn new(owner: ExtensionId, config: MailboxConfig) -> (Self, MailboxSender) {
        let (tx, rx) = mpsc::channel(config.capacity);
        let (lifecycle_tx, lifecycle_rx) = watch::channel(MailboxLifecycle::Active);
        
        let mailbox = Self {
            receiver: rx,
            owner,
            lifecycle_rx,
        };
        
        let sender = MailboxSender {
            tx,
            lifecycle_tx,
            owner,
        };
        
        (mailbox, sender)
    }
    
    /// 获取下一个消息
    pub async fn recv(&mut self) -> Option<MailboxMessage> {
        self.receiver.recv().await
    }
    
    /// 检查生命周期状态
    pub fn is_active(&self) -> bool {
        *self.lifecycle_rx.borrow() == MailboxLifecycle::Active
    }
    
    pub fn owner(&self) -> ExtensionId {
        self.owner
    }
}

#[derive(Clone)]
pub struct MailboxSender {
    tx: mpsc::Sender<MailboxMessage>,
    lifecycle_tx: watch::Sender<MailboxLifecycle>,
    owner: ExtensionId,
}

impl MailboxSender {
    pub async fn send(&self, msg: ExtensionMessage) -> Result<()> {
        if !self.is_active() {
            return Err(ExtensionError::RuntimeError(
                format!("Mailbox {} is not active", self.owner)
            ));
        }
        
        self.tx
            .send(MailboxMessage::Message(msg))
            .await
            .map_err(|_| ExtensionError::RuntimeError("Mailbox closed".into()))
    }
    
    pub fn suspend(&self) -> Result<()> {
        self.lifecycle_tx
            .send(MailboxLifecycle::Suspended)
            .map_err(|_| ExtensionError::RuntimeError("Cannot suspend".into()))
    }
    
    pub fn resume(&self) -> Result<()> {
        self.lifecycle_tx
            .send(MailboxLifecycle::Active)
            .map_err(|_| ExtensionError::RuntimeError("Cannot resume".into()))
    }
    
    pub fn stop(&self) -> Result<()> {
        self.lifecycle_tx
            .send(MailboxLifecycle::Stopped)
            .map_err(|_| ExtensionError::RuntimeError("Cannot stop".into()))
    }
    
    pub fn is_active(&self) -> bool {
        *self.lifecycle_tx.borrow() == MailboxLifecycle::Active
    }
}
```

### 2.4 InMemory Runtime (src/runtime/in_memory.rs)

```rust
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock, oneshot};
use async_trait::async_trait;
use uuid::Uuid;

use crate::{
    ExtensionId, ExtensionTopic, ExtensionActor, ExtensionLifecycle,
    ExtensionMessage, ExtensionRequest, ExtensionResponse, ExtensionEvent,
    HookPoint, state::ExtensionState, error::{ExtensionError, Result},
};
use super::{
    ExtensionRuntime, Mailbox, MailboxSender, MailboxConfig,
    MailboxLifecycle, MailboxMessage,
};

/// Extension Actor Handle
struct ExtensionHandle {
    actor: Arc<dyn ExtensionActor>,
    mailbox: Mailbox,
    mailbox_sender: MailboxSender,
    lifecycle: ExtensionLifecycle,
    subscribed_topics: HashSet<ExtensionTopic>,
    registered_hooks: Vec<HookPoint>,
}

/// In-Memory Extension Runtime
pub struct InMemoryExtensionRuntime {
    extensions: RwLock<HashMap<ExtensionId, ExtensionHandle>>,
    topic_subscribers: RwLock<HashMap<ExtensionTopic, HashSet<ExtensionId>>>,
    hooks: RwLock<HashMap<HookPoint, Vec<ExtensionId>>>,
    name_index: RwLock<HashMap<&'static str, ExtensionId>>,
    config: RuntimeConfig,
}

pub struct RuntimeConfig {
    pub mailbox_config: MailboxConfig,
    pub default_timeout_ms: u64,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            mailbox_config: MailboxConfig::default(),
            default_timeout_ms: 30_000,
        }
    }
}

impl InMemoryExtensionRuntime {
    pub fn new(config: RuntimeConfig) -> Self {
        Self {
            extensions: RwLock::new(HashMap::new()),
            topic_subscribers: RwLock::new(HashMap::new()),
            hooks: RwLock::new(HashMap::new()),
            name_index: RwLock::new(HashMap::new()),
            config,
        }
    }
    
    /// 启动 Extension 的消息处理循环
    async fn start_message_loop(
        self: Arc<Self>,
        id: ExtensionId,
        actor: Arc<dyn ExtensionActor>,
        mut mailbox: Mailbox,
    ) {
        let ctx = crate::ExtensionContext::new(
            self.clone() as Arc<dyn ExtensionRuntimeHandle>,
            id,
        );
        
        loop {
            tokio::select! {
                Some(msg) = mailbox.recv() => {
                    match msg {
                        MailboxMessage::Message(msg) => {
                            let result = actor.handle(&ctx, msg).await;
                            if let Err(e) = result {
                                tracing::error!("Extension {} error: {}", id, e);
                            }
                        }
                        MailboxMessage::Shutdown => {
                            tracing::info!("Extension {} shutting down", id);
                            break;
                        }
                    }
                }
                _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {
                    // Keep-alive tick
                }
            }
        }
    }
}

#[async_trait]
impl ExtensionRuntime for InMemoryExtensionRuntime {
    async fn register(&self, extension: Arc<dyn ExtensionActor>) -> Result<ExtensionId> {
        let id = extension.id();
        let name = extension.name();
        
        // 检查是否已注册
        {
            let exts = self.extensions.read().await;
            if exts.contains_key(&id) {
                return Err(ExtensionError::AlreadyRegistered(id));
            }
        }
        
        // 创建 mailbox
        let (mailbox, sender) = Mailbox::new(id, self.config.mailbox_config.clone());
        
        // 注册 Hook 点
        for hook_point in extension.hook_points() {
            let mut hooks = self.hooks.write().await;
            hooks.entry(hook_point).or_default().push(id);
        }
        
        // 存储 Handle
        let handle = ExtensionHandle {
            actor: extension.clone(),
            mailbox,
            mailbox_sender: sender,
            lifecycle: ExtensionLifecycle::Registered,
            subscribed_topics: HashSet::new(),
            registered_hooks: extension.hook_points(),
        };
        
        {
            let mut exts = self.extensions.write().await;
            exts.insert(id, handle);
        }
        
        {
            let mut names = self.name_index.write().await;
            names.insert(name, id);
        }
        
        // 调用 on_start
        let ctx = crate::ExtensionContext::new(
            self.clone() as Arc<dyn ExtensionRuntimeHandle>,
            id,
        );
        extension.on_start(&ctx).await?;
        
        // 更新生命周期
        {
            let mut exts = self.extensions.write().await;
            if let Some(handle) = exts.get_mut(&id) {
                handle.lifecycle = ExtensionLifecycle::Initialized;
            }
        }
        
        // 启动消息循环
        let rt = self.clone();
        tokio::spawn(async move {
            rt.start_message_loop(id, extension, mailbox).await;
        });
        
        // 更新为 Running
        {
            let mut exts = self.extensions.write().await;
            if let Some(handle) = exts.get_mut(&id) {
                handle.lifecycle = ExtensionLifecycle::Running;
            }
        }
        
        Ok(id)
    }
    
    async fn unregister(&self, id: ExtensionId) -> Result<()> {
        let extension = {
            let mut exts = self.extensions.write().await;
            exts.remove(&id)
        }.ok_or(ExtensionError::NotFound(id))?;
        
        // 调用 on_stop
        let ctx = crate::ExtensionContext::new(
            self.clone() as Arc<dyn ExtensionRuntimeHandle>,
            id,
        );
        extension.actor.on_stop(&ctx).await?;
        
        // 清理订阅
        {
            let mut topics = self.topic_subscribers.write().await;
            for topic in extension.subscribed_topics {
                if let Some(subs) = topics.get_mut(&topic) {
                    subs.remove(&id);
                }
            }
        }
        
        // 清理 Hook
        {
            let mut hooks = self.hooks.write().await;
            for hook_point in extension.registered_hooks {
                if let Some(exts) = hooks.get_mut(&hook_point) {
                    exts.retain(|e| *e != id);
                }
            }
        }
        
        // 清理 name index
        {
            let mut names = self.name_index.write().await;
            names.retain(|_, v| *v != id);
        }
        
        Ok(())
    }
    
    async fn send(&self, target: ExtensionId, msg: ExtensionMessage) -> Result<()> {
        let exts = self.extensions.read().await;
        let handle = exts.get(&target)
            .ok_or(ExtensionError::NotFound(target))?;
        
        handle.mailbox_sender.send(msg).await
    }
    
    async fn call(&self, target: ExtensionId, req: ExtensionRequest) -> Result<ExtensionResponse> {
        let (reply_tx, reply_rx) = oneshot::channel();
        let request_id = req.request_id;
        
        // 创建响应 mailbox
        let (tx, mut rx) = mpsc::channel(1);
        
        // 启动等待响应任务
        tokio::spawn(async move {
            let timeout = tokio::time::sleep(std::time::Duration::from_millis(30_000));
            tokio::select! {
                resp = rx.recv() => {
                    if let Some(r) = resp {
                        let _ = reply_tx.send(r);
                    }
                }
                _ = timeout => {
                    let _ = reply_tx.send(ExtensionResponse {
                        request_id,
                        status: crate::ResponseStatus::Timeout,
                        result: None,
                        error: Some(crate::ExtensionErrorInfo {
                            code: "TIMEOUT".into(),
                            message: "Request timed out".into(),
                            details: None,
                        }),
                    });
                }
            }
        });
        
        // 发送消息
        let msg = ExtensionMessage::Request {
            request_id,
            action: req.action,
            reply_to: Uuid::new_v4(), // 简化处理
            timeout_ms: req.timeout_ms,
        };
        
        self.send(target, msg).await?;
        
        // 等待响应
        reply_rx.await.map_err(|_| ExtensionError::RuntimeError("Response channel closed".into()))
    }
    
    async fn publish(&self, topic: ExtensionTopic, event: ExtensionEvent) -> Result<()> {
        let subscribers = {
            let topics = self.topic_subscribers.read().await;
            topics.get(&topic).cloned().unwrap_or_default()
        };
        
        for subscriber_id in subscribers {
            let msg = ExtensionMessage::Command {
                action: crate::ExtensionAction::Publish {
                    topic: topic.clone(),
                    event: event.clone(),
                },
                correlation_id: Some(event.correlation_id.unwrap_or_else(Uuid::new_v4)),
            };
            
            let _ = self.send(subscriber_id, msg).await;
        }
        
        Ok(())
    }
    
    async fn subscribe(&self, id: ExtensionId, topic: ExtensionTopic) -> Result<()> {
        let mut topics = self.topic_subscribers.write().await;
        topics.entry(topic.clone()).or_default().insert(id);
        
        let mut exts = self.extensions.write().await;
        if let Some(handle) = exts.get_mut(&id) {
            handle.subscribed_topics.insert(topic);
        }
        
        Ok(())
    }
    
    async fn unsubscribe(&self, id: ExtensionId, topic: ExtensionTopic) -> Result<()> {
        let mut topics = self.topic_subscribers.write().await;
        if let Some(subs) = topics.get_mut(&topic) {
            subs.remove(&id);
        }
        
        let mut exts = self.extensions.write().await;
        if let Some(handle) = exts.get_mut(&id) {
            handle.subscribed_topics.remove(&topic);
        }
        
        Ok(())
    }
    
    fn find(&self, name: &str) -> Option<ExtensionId> {
        // 同步查询
        let names = self.name_index.blocking_read();
        names.get(name).copied()
    }
    
    fn list(&self) -> Vec<ExtensionId> {
        let exts = self.extensions.blocking_read();
        exts.keys().copied().collect()
    }
    
    fn lifecycle(&self, id: ExtensionId) -> Option<ExtensionLifecycle> {
        let exts = self.extensions.blocking_read();
        exts.get(&id).map(|h| h.lifecycle)
    }
    
    fn snapshot(&self, id: ExtensionId) -> Result<ExtensionState> {
        // 实现快照逻辑
        Ok(ExtensionState::new())
    }
    
    async fn register_hook(&self, id: ExtensionId, hook_point: HookPoint) -> Result<()> {
        let mut hooks = self.hooks.write().await;
        hooks.entry(hook_point).or_default().push(id);
        
        let mut exts = self.extensions.write().await;
        if let Some(handle) = exts.get_mut(&id) {
            if !handle.registered_hooks.contains(&hook_point) {
                handle.registered_hooks.push(hook_point);
            }
        }
        
        Ok(())
    }
    
    async fn trigger_hook(
        &self,
        hook_point: HookPoint,
        context: serde_json::Value,
    ) -> Result<()> {
        let subscribers = {
            let hooks = self.hooks.read().await;
            hooks.get(&hook_point).cloned().unwrap_or_default()
        };
        
        for id in subscribers {
            let msg = ExtensionMessage::Command {
                action: crate::ExtensionAction::Custom {
                    namespace: "hook".into(),
                    name: format!("{:?}", hook_point),
                    payload: context.clone(),
                },
                correlation_id: None,
            };
            
            let _ = self.send(id, msg).await;
        }
        
        Ok(())
    }
}
```

---

## 验证标准

### Phase 1 验证
- [ ] 所有类型可编译
- [ ] 基础单元测试覆盖
- [ ] 与 torque-kernel 无循环依赖

### Phase 2 验证
- [ ] Extension 可注册/注销
- [ ] Fire-and-Forget 消息可达
- [ ] Request/Reply 消息正确响应
- [ ] Pub/Sub 事件正确分发
- [ ] Hook 点可注册和触发
- [ ] 生命周期状态正确转换

---

## 文件清单

```
crates/torque-extension/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── id.rs
    ├── actor.rs
    ├── context.rs
    ├── error.rs
    ├── hook.rs
    ├── message.rs
    ├── state.rs
    ├── topic.rs
    └── runtime/
        ├── mod.rs
        ├── trait.rs
        ├── in_memory.rs
        └── mailbox.rs
```
