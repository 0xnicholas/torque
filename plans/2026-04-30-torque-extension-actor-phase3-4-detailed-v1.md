# Torque Extension Actor 系统 - Phase 3 & 4 详细设计

## 概述

本文档细化 Phase 3 (Harness 集成) 和 Phase 4 (内置 Extension 示例) 的具体实现。

---

## Phase 3: 与 Torque Harness 集成

### 3.1 集成架构

```
┌─────────────────────────────────────────────────────────────────┐
│                         App / Server                             │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────────┐    │
│  │              ServiceContainer                             │    │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │    │
│  │  │ TeamSupervisor│  │AgentInstance │  │ Notification │  │    │
│  │  └──────┬───────┘  └──────────────┘  └──────────────┘  │    │
│  │         │                                             │    │
│  │  ┌──────▼──────────────────────────────────────┐      │    │
│  │  │         ExtensionService (NEW)               │      │    │
│  │  │  ┌────────────────────────────────────────┐  │      │    │
│  │  │  │   InMemoryExtensionRuntime            │  │      │    │
│  │  │  │   ┌────────┐ ┌────────┐ ┌────────┐   │  │      │    │
│  │  │  │   │Ext A   │ │Ext B   │ │Ext C   │   │  │      │    │
│  │  │  │   └────────┘ └────────┘ └────────┘   │  │      │    │
│  │  │  └────────────────────────────────────────┘  │      │    │
│  │  └─────────────────────────────────────────────────┘      │    │
│  └─────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
```

### 3.2 新增文件结构

```
crates/torque-harness/src/
├── extension/
│   ├── mod.rs              # 模块导出
│   ├── service.rs           # ExtensionService
│   ├── runtime_handle.rs    # HarnessExtensionRuntimeHandle
│   ├── hooks.rs            # Hook 触发器实现
│   ├── api.rs              # Extension API handlers
│   └── config.rs           # Extension 配置
```

### 3.3 Extension 配置 (src/extension/config.rs)

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Extension 配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExtensionConfig {
    /// 是否启用 Extension 系统
    pub enabled: bool,
    /// 内置 Extension 配置
    pub builtins: Vec<BuiltinExtensionConfig>,
    /// 自定义 Extension 配置
    pub custom: HashMap<String, serde_json::Value>,
    /// Extension 加载顺序
    pub load_order: Vec<String>,
    /// 默认超时 (毫秒)
    pub default_timeout_ms: u64,
}

/// 内置 Extension 配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BuiltinExtensionConfig {
    pub name: String,
    pub enabled: bool,
    pub config: serde_json::Value,
}

impl Default for ExtensionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            builtins: vec![
                // 默认启用日志和指标扩展
                BuiltinExtensionConfig {
                    name: "logging".into(),
                    enabled: true,
                    config: serde_json::json!({}),
                },
                BuiltinExtensionConfig {
                    name: "metrics".into(),
                    enabled: true,
                    config: serde_json::json!({}),
                },
            ],
            custom: HashMap::new(),
            load_order: vec![],
            default_timeout_ms: 30_000,
        }
    }
}

impl ExtensionConfig {
    pub fn from_env() -> Self {
        let enabled = std::env::var("TORQUE_EXTENSION_ENABLED")
            .map(|v| v == "true")
            .unwrap_or(true);
        
        let default_timeout_ms = std::env::var("TORQUE_EXTENSION_TIMEOUT_MS")
            .and_then(|v| v.parse().ok())
            .unwrap_or(30_000);
        
        Self {
            enabled,
            builtins: Self::load_builtins_from_env(),
            custom: HashMap::new(),
            load_order: vec![],
            default_timeout_ms,
        }
    }
    
    fn load_builtins_from_env() -> Vec<BuiltinExtensionConfig> {
        let mut builtins = Vec::new();
        
        if std::env::var("TORQUE_LOGGING_EXTENSION_ENABLED")
            .map(|v| v == "true")
            .unwrap_or(true)
        {
            builtins.push(BuiltinExtensionConfig {
                name: "logging".into(),
                enabled: true,
                config: serde_json::json!({
                    "level": std::env::var("TORQUE_LOG_LEVEL").unwrap_or_else(|_| "info".into()),
                }),
            });
        }
        
        if std::env::var("TORQUE_METRICS_EXTENSION_ENABLED")
            .map(|v| v == "true")
            .unwrap_or(true)
        {
            builtins.push(BuiltinExtensionConfig {
                name: "metrics".into(),
                enabled: true,
                config: serde_json::json!({
                    "export_interval_seconds": 60,
                }),
            });
        }
        
        builtins
    }
}
```

### 3.4 Extension Runtime Handle (src/extension/runtime_handle.rs)

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock, oneshot};
use uuid::Uuid;

use torque_extension::{
    ExtensionId, ExtensionTopic, ExtensionMessage, ExtensionEvent,
    ExtensionRequest, ExtensionResponse, ExtensionRuntimeHandle,
    EventHandler, EventEmitter, Result, ExtensionError,
};

/// Harness 中 Extension Runtime 的内部实现
pub struct HarnessExtensionRuntimeHandle {
    runtime: Arc<dyn torque_extension::ExtensionRuntime>,
    name_index: RwLock<HashMap<String, ExtensionId>>,
    reply_channels: RwLock<HashMap<Uuid, mpsc::Sender<ExtensionResponse>>>,
}

impl HarnessExtensionRuntimeHandle {
    pub fn new(runtime: Arc<dyn torque_extension::ExtensionRuntime>) -> Self {
        Self {
            runtime,
            name_index: RwLock::new(HashMap::new()),
            reply_channels: RwLock::new(HashMap::new()),
        }
    }
    
    /// 注册名称索引
    pub async fn register_name(&self, name: &str, id: ExtensionId) {
        let mut index = self.name_index.write().await;
        index.insert(name.to_string(), id);
    }
    
    /// 注销名称索引
    pub async fn unregister_name(&self, name: &str) {
        let mut index = self.name_index.write().await;
        index.remove(name);
    }
}

impl ExtensionRuntimeHandle for HarnessExtensionRuntimeHandle {
    fn id(&self) -> ExtensionId {
        // Runtime 本身的 ID (特殊用途)
        ExtensionId::new()
    }
    
    async fn send(&self, target: ExtensionId, msg: ExtensionMessage) -> Result<()> {
        self.runtime.send(target, msg).await
    }
    
    async fn call(&self, target: ExtensionId, req: ExtensionRequest) -> Result<ExtensionResponse> {
        self.runtime.call(target, req).await
    }
    
    async fn publish(&self, topic: ExtensionTopic, event: ExtensionEvent) -> Result<()> {
        self.runtime.publish(topic, event).await
    }
    
    fn find_extension(&self, name: &str) -> Option<ExtensionId> {
        let index = self.name_index.blocking_read();
        index.get(name).copied()
    }
    
    fn create_mailbox(&self) -> (Uuid, mpsc::Sender<ExtensionResponse>) {
        let (tx, rx) = mpsc::channel(1);
        let id = Uuid::new_v4();
        
        // 存储 channel 以便响应时可以找到
        // 注意：这里需要改进，实际应该存储在 Extension Runtime 中
        (id, tx)
    }
}

impl EventEmitter for HarnessExtensionRuntimeHandle {
    fn emit(&self, event: ExtensionEvent) -> Result<()> {
        // 将事件发布到默认主题
        let topic = event.topic.clone();
        let rt = self.runtime.clone();
        tokio::spawn(async move {
            let _ = rt.publish(topic, event).await;
        });
        Ok(())
    }
    
    fn subscribe(&self, topic: ExtensionTopic, handler: Arc<dyn EventHandler>) -> Result<()> {
        // TODO: 实现订阅逻辑
        Ok(())
    }
    
    fn unsubscribe(&self, topic: ExtensionTopic) -> Result<()> {
        // TODO: 实现取消订阅逻辑
        Ok(())
    }
}
```

### 3.5 Hook 触发器 (src/extension/hooks.rs)

```rust
use torque_extension::{HookPoint, HookResult, ExtensionTopic, topics};
use crate::service::ServiceContainer;
use std::sync::Arc;

/// Hook 触发器，负责在适当的时机触发 Extension Hooks
pub struct ExtensionHookTrigger {
    runtime: Arc<dyn torque_extension::ExtensionRuntime>,
}

impl ExtensionHookTrigger {
    pub fn new(runtime: Arc<dyn torque_extension::ExtensionRuntime>) -> Self {
        Self { runtime }
    }
    
    /// 在执行前触发 PreExecution Hook
    pub async fn pre_execution(
        &self,
        instance_id: &torque_kernel::AgentInstanceId,
        request: &torque_kernel::ExecutionRequest,
    ) -> HookResult {
        let payload = serde_json::json!({
            "instance_id": instance_id.to_string(),
            "goal": request.goal(),
            "instructions": request.instructions(),
        });
        
        let result = self.trigger_hook(HookPoint::PreExecution, payload).await;
        result.unwrap_or(HookResult::Continue)
    }
    
    /// 在执行后触发 PostExecution Hook
    pub async fn post_execution(
        &self,
        instance_id: &torque_kernel::AgentInstanceId,
        result: &torque_kernel::ExecutionResult,
    ) -> HookResult {
        let payload = serde_json::json!({
            "instance_id": instance_id.to_string(),
            "outcome": format!("{:?}", result.outcome),
            "artifact_count": result.artifact_ids.len(),
        });
        
        let result = self.trigger_hook(HookPoint::PostExecution, payload).await;
        result.unwrap_or(HookResult::Continue)
    }
    
    /// 在 Tool 调用前触发 PreToolCall Hook
    pub async fn pre_tool_call(
        &self,
        instance_id: &torque_kernel::AgentInstanceId,
        tool_name: &str,
        arguments: &serde_json::Value,
    ) -> HookResult {
        let payload = serde_json::json!({
            "instance_id": instance_id.to_string(),
            "tool_name": tool_name,
            "arguments": arguments,
        });
        
        let result = self.trigger_hook(HookPoint::PreToolCall, payload).await;
        result.unwrap_or(HookResult::Continue)
    }
    
    /// 在 Tool 调用后触发 PostToolCall Hook
    pub async fn post_tool_call(
        &self,
        instance_id: &torque_kernel::AgentInstanceId,
        tool_name: &str,
        result: &str,
    ) -> HookResult {
        let payload = serde_json::json!({
            "instance_id": instance_id.to_string(),
            "tool_name": tool_name,
            "result": result,
        });
        
        let result = self.trigger_hook(HookPoint::PostToolCall, payload).await;
        result.unwrap_or(HookResult::Continue)
    }
    
    /// 在 Artifact 创建后触发 OnArtifactCreated Hook
    pub async fn on_artifact_created(
        &self,
        instance_id: &torque_kernel::AgentInstanceId,
        artifact_id: &torque_kernel::ArtifactId,
    ) -> HookResult {
        let payload = serde_json::json!({
            "instance_id": instance_id.to_string(),
            "artifact_id": artifact_id.to_string(),
        });
        
        let result = self.trigger_hook(HookPoint::OnArtifactCreated, payload).await;
        result.unwrap_or(HookResult::Continue)
    }
    
    /// 在 Delegation 创建后触发 OnDelegationCreated Hook
    pub async fn on_delegation_created(
        &self,
        parent_id: &torque_kernel::AgentInstanceId,
        delegation_id: &torque_kernel::DelegationRequestId,
    ) -> HookResult {
        let payload = serde_json::json!({
            "parent_instance_id": parent_id.to_string(),
            "delegation_id": delegation_id.to_string(),
        });
        
        let result = self.trigger_hook(HookPoint::PostDelegation, payload).await;
        result.unwrap_or(HookResult::Continue)
    }
    
    async fn trigger_hook(
        &self,
        hook_point: HookPoint,
        payload: serde_json::Value,
    ) -> Option<HookResult> {
        let context = serde_json::json!({
            "hook_point": format!("{:?}", hook_point),
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "payload": payload,
        });
        
        // 触发 Hook 并收集结果
        let _ = self.runtime.trigger_hook(hook_point, context).await;
        
        // TODO: 收集所有 Extension 的 Hook 结果并进行合并
        // 当前简化处理：只返回 Continue
        Some(HookResult::Continue)
    }
}

/// Hook 与现有 Service 的集成点
pub struct HookIntegration {
    services: Arc<ServiceContainer>,
    trigger: Arc<ExtensionHookTrigger>,
}

impl HookIntegration {
    pub fn new(
        services: Arc<ServiceContainer>,
        trigger: Arc<ExtensionHookTrigger>,
    ) -> Self {
        Self { services, trigger }
    }
    
    /// 在 AgentInstanceService 执行前后添加 Hook
    pub async fn wrap_execution<F, R>(
        &self,
        instance_id: &torque_kernel::AgentInstanceId,
        request: &torque_kernel::ExecutionRequest,
        f: F,
    ) -> Result<torque_kernel::ExecutionResult, Box<dyn std::error::Error + Send + Sync>>
    where
        F: Future<Output = Result<torque_kernel::ExecutionResult, Box<dyn std::error::Error + Send + Sync>>>,
    {
        // Pre Hook
        let pre_result = self.trigger.pre_execution(instance_id, request).await;
        match pre_result {
            HookResult::Blocked { reason } => {
                return Err(format!("Pre-execution hook blocked: {}", reason).into());
            }
            HookResult::ShortCircuit(value) => {
                // 短路返回自定义结果
                return Err(format!("Short-circuited: {:?}", value).into());
            }
            HookResult::Modified(_) => {
                // TODO: 应用修改后的请求
            }
            HookResult::Continue => {}
        }
        
        // 执行
        let result = f.await?;
        
        // Post Hook
        let _ = self.trigger.post_execution(instance_id, &result).await;
        
        Ok(result)
    }
}
```

### 3.6 ExtensionService (src/extension/service.rs)

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use torque_extension::{
    ExtensionActor, ExtensionRuntime, ExtensionId, ExtensionTopic,
    ExtensionMessage, ExtensionEvent, ExtensionVersion, HookPoint,
    error::{ExtensionError, Result},
};

use super::{ExtensionConfig, HarnessExtensionRuntimeHandle};
use super::runtime_handle::HarnessExtensionRuntimeHandle;

/// Extension 管理服务
pub struct ExtensionService {
    runtime: Arc<dyn ExtensionRuntime>,
    runtime_handle: Arc<HarnessExtensionRuntimeHandle>,
    config: ExtensionConfig,
    // Extension 元数据缓存
    metadata: RwLock<HashMap<ExtensionId, ExtensionMetadata>>,
}

struct ExtensionMetadata {
    name: String,
    version: ExtensionVersion,
    hooks: Vec<HookPoint>,
}

impl ExtensionService {
    /// 创建 ExtensionService
    pub async fn new(
        runtime: Arc<dyn ExtensionRuntime>,
        config: ExtensionConfig,
    ) -> Result<Self> {
        let runtime_handle = Arc::new(HarnessExtensionRuntimeHandle::new(runtime.clone()));
        
        let service = Self {
            runtime,
            runtime_handle,
            config,
            metadata: RwLock::new(HashMap::new()),
        };
        
        // 加载内置 Extension
        service.load_builtins().await?;
        
        Ok(service)
    }
    
    /// 加载内置 Extension
    async fn load_builtins(&self) -> Result<()> {
        for builtin in &self.config.builtins {
            if builtin.enabled {
                self.register_builtin(&builtin.name, &builtin.config).await?;
            }
        }
        Ok(())
    }
    
    /// 注册内置 Extension
    async fn register_builtin(
        &self,
        name: &str,
        config: &serde_json::Value,
    ) -> Result<ExtensionId> {
        let extension: Arc<dyn ExtensionActor> = match name {
            "logging" => Arc::new(self.create_logging_extension(config)?),
            "metrics" => Arc::new(self.create_metrics_extension(config)?),
            _ => return Err(ExtensionError::NotFound(
                ExtensionId::from_uuid(uuid::Uuid::nil())
            )),
        };
        
        let id = self.register(extension).await?;
        
        // 记录元数据
        {
            let mut meta = self.metadata.write().await;
            meta.insert(id, ExtensionMetadata {
                name: name.to_string(),
                version: extension.version(),
                hooks: extension.hook_points(),
            });
        }
        
        Ok(id)
    }
    
    /// 注册自定义 Extension
    pub async fn register(&self, extension: Arc<dyn ExtensionActor>) -> Result<ExtensionId> {
        let id = self.runtime.register(extension.clone()).await?;
        
        // 注册名称索引
        self.runtime_handle.register_name(extension.name(), id).await;
        
        Ok(id)
    }
    
    /// 注销 Extension
    pub async fn unregister(&self, id: ExtensionId) -> Result<()> {
        // 获取名称用于清理索引
        let name = {
            let meta = self.metadata.read().await;
            meta.get(&id).map(|m| m.name.clone())
        };
        
        if let Some(name) = name {
            self.runtime_handle.unregister_name(&name).await;
        }
        
        self.runtime.unregister(id).await?;
        
        // 清理元数据
        {
            let mut meta = self.metadata.write().await;
            meta.remove(&id);
        }
        
        Ok(())
    }
    
    /// 列出所有 Extension
    pub async fn list(&self) -> Vec<ExtensionInfo> {
        let ids = self.runtime.list();
        let meta = self.metadata.read().await;
        
        ids.into_iter()
            .map(|id| {
                let lifecycle = self.runtime.lifecycle(id);
                let metadata = meta.get(&id);
                
                ExtensionInfo {
                    id,
                    name: metadata.map(|m| m.name.clone()).unwrap_or_default(),
                    version: metadata.map(|m| m.version.clone()).unwrap_or_else(|| ExtensionVersion::new(0, 0, 0)),
                    hooks: metadata.map(|m| m.hooks.clone()).unwrap_or_default(),
                    lifecycle,
                }
            })
            .collect()
    }
    
    /// 发送消息给 Extension
    pub async fn send(&self, target: ExtensionId, msg: ExtensionMessage) -> Result<()> {
        self.runtime.send(target, msg).await
    }
    
    /// 订阅主题
    pub async fn subscribe(&self, id: ExtensionId, topic: ExtensionTopic) -> Result<()> {
        self.runtime.subscribe(id, topic).await
    }
    
    /// 发布事件
    pub async fn publish(&self, topic: ExtensionTopic, event: ExtensionEvent) -> Result<()> {
        self.runtime.publish(topic, event).await
    }
    
    // === 内置 Extension 工厂方法 ===
    
    fn create_logging_extension(&self, config: &serde_json::Value) -> Result<LoggingExtension> {
        let level = config
            .get("level")
            .and_then(|v| v.as_str())
            .unwrap_or("info");
        
        Ok(LoggingExtension { level: level.to_string() })
    }
    
    fn create_metrics_extension(&self, config: &serde_json::Value) -> Result<MetricsExtension> {
        let export_interval = config
            .get("export_interval_seconds")
            .and_then(|v| v.as_u64())
            .unwrap_or(60);
        
        Ok(MetricsExtension {
            export_interval_secs: export_interval as u32,
            counters: std::sync::Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            histograms: std::sync::Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        })
    }
}

/// Extension 信息
#[derive(Debug, Clone)]
pub struct ExtensionInfo {
    pub id: ExtensionId,
    pub name: String,
    pub version: ExtensionVersion,
    pub hooks: Vec<HookPoint>,
    pub lifecycle: Option<torque_extension::ExtensionLifecycle>,
}
```

### 3.7 mod.rs 导出 (src/extension/mod.rs)

```rust
pub mod config;
pub mod service;
pub mod runtime_handle;
pub mod hooks;
pub mod api;

pub use config::{ExtensionConfig, BuiltinExtensionConfig};
pub use service::{ExtensionService, ExtensionInfo};
pub use hooks::{ExtensionHookTrigger, HookIntegration};
```

### 3.8 ServiceContainer 集成

修改 `src/service/mod.rs`:

```rust
// 添加新字段
pub struct ServiceContainer {
    // ... 现有字段 ...
    
    /// Extension 服务 (新增)
    pub extension: std::sync::Arc<ExtensionService>,
    
    /// Extension Hook 触发器 (新增)
    pub extension_hook_trigger: std::sync::Arc<ExtensionHookTrigger>,
}

impl ServiceContainer {
    pub fn new(/* ... */, extension_config: ExtensionConfig) -> Self {
        // ... 现有初始化 ...
        
        // Extension 初始化 (新增)
        let extension_runtime = Arc::new(
            torque_extension::InMemoryExtensionRuntime::new(
                torque_extension::RuntimeConfig::default()
            )
        );
        
        let extension_service = std::sync::Arc::new(
            ExtensionService::new(extension_runtime.clone(), extension_config).await
                .expect("Failed to initialize ExtensionService")
        );
        
        let extension_hook_trigger = std::sync::Arc::new(
            ExtensionHookTrigger::new(extension_runtime)
        );
        
        Self {
            // ... 现有字段 ...
            
            extension: extension_service,
            extension_hook_trigger,
        }
    }
}
```

### 3.9 API 端点 (src/extension/api.rs)

```rust
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use std::sync::Arc;

use crate::service::ExtensionService;
use torque_extension::{ExtensionMessage, ExtensionTopic};

pub struct ExtensionApiState {
    service: Arc<ExtensionService>,
}

impl ExtensionApiState {
    pub fn new(service: Arc<ExtensionService>) -> Self {
        Self { service }
    }
}

pub fn routes(state: ExtensionApiState) -> Router {
    Router::new()
        .route("/extensions", get(list_extensions))
        .route("/extensions/:id", get(get_extension).delete(delete_extension))
        .route("/extensions/:id/messages", post(send_message))
        .route("/extensions/:id/subscriptions", post(subscribe).delete(unsubscribe))
        .with_state(state)
}

async fn list_extensions(
    State(state): State<ExtensionApiState>,
) -> impl IntoResponse {
    let extensions = state.service.list().await;
    Json(extensions)
}

async fn get_extension(
    State(state): State<ExtensionApiState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let id = match uuid::Uuid::parse_str(&id) {
        Ok(id) => torque_extension::ExtensionId::from_uuid(id),
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid ID").into_response(),
    };
    
    match state.service.list().await.into_iter().find(|e| e.id == id) {
        Some(ext) => Json(ext).into_response(),
        None => (StatusCode::NOT_FOUND, "Extension not found").into_response(),
    }
}

async fn delete_extension(
    State(state): State<ExtensionApiState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let id = match uuid::Uuid::parse_str(&id) {
        Ok(id) => torque_extension::ExtensionId::from_uuid(id),
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid ID").into_response(),
    };
    
    match state.service.unregister(id).await {
        Ok(()) => (StatusCode::NO_CONTENT, ()).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(serde::Deserialize)]
struct SendMessageRequest {
    action_type: String,
    payload: serde_json::Value,
}

async fn send_message(
    State(state): State<ExtensionApiState>,
    Path(id): Path<String>,
    Json(body): Json<SendMessageRequest>,
) -> impl IntoResponse {
    let id = match uuid::Uuid::parse_str(&id) {
        Ok(id) => torque_extension::ExtensionId::from_uuid(id),
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid ID").into_response(),
    };
    
    let msg = ExtensionMessage::Command {
        action: torque_extension::ExtensionAction::Custom {
            namespace: body.action_type,
            name: "api".to_string(),
            payload: body.payload,
        },
        correlation_id: Some(uuid::Uuid::new_v4()),
    };
    
    match state.service.send(id, msg).await {
        Ok(()) => (StatusCode::ACCEPTED, ()).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

#[derive(serde::Deserialize)]
struct SubscribeRequest {
    topic: String,
}

async fn subscribe(
    State(state): State<ExtensionApiState>,
    Path(id): Path<String>,
    Json(body): Json<SubscribeRequest>,
) -> impl IntoResponse {
    let id = match uuid::Uuid::parse_str(&id) {
        Ok(id) => torque_extension::ExtensionId::from_uuid(id),
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid ID").into_response(),
    };
    
    let topic = ExtensionTopic::new("custom", &body.topic);
    
    match state.service.subscribe(id, topic).await {
        Ok(()) => (StatusCode::OK, ()).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn unsubscribe(
    State(state): State<ExtensionApiState>,
    Path((id, topic)): Path<(String, String)>,
) -> impl IntoResponse {
    let id = match uuid::Uuid::parse_str(&id) {
        Ok(id) => torque_extension::ExtensionId::from_uuid(id),
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid ID").into_response(),
    };
    
    let topic = ExtensionTopic::new("custom", &topic);
    
    match state.service.unsubscribe(id, topic).await {
        Ok(()) => (StatusCode::OK, ()).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}
```

---

## Phase 4: 内置 Extension 示例

### 4.1 Logging Extension

```rust
use std::sync::Arc;
use torque_extension::{
    ExtensionActor, ExtensionContext, ExtensionMessage, ExtensionResponse,
    ExtensionVersion, HookPoint, HookResult, topics,
    error::{ExtensionError, Result},
};

pub struct LoggingExtension {
    level: String,
}

impl LoggingExtension {
    pub fn new(level: &str) -> Self {
        Self {
            level: level.to_string(),
        }
    }
}

impl ExtensionActor for LoggingExtension {
    fn id(&self) -> ExtensionId {
        // 每次创建新的 UUID
        ExtensionId::new()
    }
    
    fn name(&self) -> &'static str {
        "logging"
    }
    
    fn version(&self) -> ExtensionVersion {
        ExtensionVersion::new(1, 0, 0)
    }
    
    fn description(&self) -> &'static str {
        "Logs all Torque execution events"
    }
    
    fn hook_points(&self) -> Vec<HookPoint> {
        vec![
            HookPoint::PreExecution,
            HookPoint::PostExecution,
            HookPoint::PostToolCall,
            HookPoint::OnArtifactCreated,
            HookPoint::OnDelegationCreated,
        ]
    }
    
    async fn on_start(&self, _ctx: &ExtensionContext) -> Result<()> {
        tracing::info!("Logging Extension started with level: {}", self.level);
        Ok(())
    }
    
    async fn handle(
        &self,
        ctx: &ExtensionContext,
        msg: ExtensionMessage,
    ) -> Result<ExtensionResponse> {
        match msg {
            ExtensionMessage::Command { action, .. } => {
                self.log_action(&action).await?;
                Ok(ExtensionResponse {
                    request_id: uuid::Uuid::nil(),
                    status: torque_extension::ResponseStatus::Success,
                    result: Some(serde_json::json!({"logged": true})),
                    error: None,
                })
            }
            _ => Err(ExtensionError::RuntimeError("Unexpected message type".into())),
        }
    }
    
    async fn on_hook(
        &self,
        ctx: &ExtensionContext,
        hook_point: HookPoint,
        context: serde_json::Value,
    ) -> Result<HookResult> {
        let log_entry = serde_json::json!({
            "extension": self.name(),
            "hook_point": format!("{:?}", hook_point),
            "context": context,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });
        
        match self.level.as_str() {
            "debug" => tracing::debug!("Hook: {}", serde_json::to_string(&log_entry).unwrap()),
            "info" => tracing::info!("Hook: {}", serde_json::to_string(&log_entry).unwrap()),
            "warn" => tracing::warn!("Hook: {}", serde_json::to_string(&log_entry).unwrap()),
            "error" => tracing::error!("Hook: {}", serde_json::to_string(&log_entry).unwrap()),
            _ => tracing::info!("Hook: {}", serde_json::to_string(&log_entry).unwrap()),
        }
        
        Ok(HookResult::Continue)
    }
}

impl LoggingExtension {
    async fn log_action(&self, action: &ExtensionAction) -> Result<()> {
        match action {
            ExtensionAction::Execute { goal, instructions } => {
                tracing::info!("Execute: goal={}, instructions={:?}", goal, instructions);
            }
            ExtensionAction::Publish { topic, event } => {
                tracing::info!("Publish: topic={}, event={:?}", topic, event);
            }
            ExtensionAction::Custom { namespace, name, payload } => {
                tracing::info!("Custom: {}/{} -> {:?}", namespace, name, payload);
            }
            _ => {}
        }
        Ok(())
    }
}
```

### 4.2 Metrics Extension

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use torque_extension::{
    ExtensionActor, ExtensionContext, ExtensionMessage, ExtensionResponse,
    ExtensionVersion, HookPoint, HookResult, ExtensionTopic,
    error::{ExtensionError, Result},
};

pub struct MetricsExtension {
    export_interval_secs: u32,
    counters: Arc<RwLock<HashMap<String, u64>>>,
    histograms: Arc<RwLock<HashMap<String, Vec<u64>>>>,
}

impl MetricsExtension {
    pub fn new(export_interval_secs: u32) -> Self {
        Self {
            export_interval_secs,
            counters: Arc::new(RwLock::new(HashMap::new())),
            histograms: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    async fn increment_counter(&self, name: &str, value: u64) {
        let mut counters = self.counters.write().await;
        *counters.entry(name.to_string()).or_insert(0) += value;
    }
    
    async fn record_histogram(&self, name: &str, value: u64) {
        let mut histograms = self.histograms.write().await;
        histograms
            .entry(name.to_string())
            .or_insert_with(Vec::new)
            .push(value);
    }
    
    pub async fn get_metrics(&self) -> serde_json::Value {
        let counters = self.counters.read().await;
        let histograms = self.histograms.read().await;
        
        let mut histogram_stats = serde_json::Map::new();
        for (name, values) in histograms.iter() {
            if values.is_empty() {
                continue;
            }
            let mut sorted = values.clone();
            sorted.sort();
            let sum: u64 = values.iter().sum();
            let count = values.len() as u64;
            histogram_stats.insert(name.clone(), serde_json::json!({
                "count": count,
                "sum": sum,
                "min": sorted.first(),
                "max": sorted.last(),
                "avg": sum as f64 / count as f64,
                "p50": sorted[sorted.len() / 2],
                "p95": sorted[(sorted.len() as f64 * 0.95) as usize],
                "p99": sorted[(sorted.len() as f64 * 0.99) as usize],
            }));
        }
        
        serde_json::json!({
            "counters": *counters,
            "histograms": histogram_stats,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        })
    }
}

impl ExtensionActor for MetricsExtension {
    fn id(&self) -> ExtensionId {
        ExtensionId::new()
    }
    
    fn name(&self) -> &'static str {
        "metrics"
    }
    
    fn version(&self) -> ExtensionVersion {
        ExtensionVersion::new(1, 0, 0)
    }
    
    fn description(&self) -> &'static str {
        "Collects and exports Torque execution metrics"
    }
    
    fn hook_points(&self) -> Vec<HookPoint> {
        vec![
            HookPoint::PreExecution,
            HookPoint::PostExecution,
            HookPoint::PreToolCall,
            HookPoint::PostToolCall,
        ]
    }
    
    async fn on_start(&self, ctx: &ExtensionContext) -> Result<()> {
        tracing::info!("Metrics Extension started, export interval: {}s", self.export_interval_secs);
        
        // 启动定期导出任务
        let counters = self.counters.clone();
        let histograms = self.histograms.clone();
        let interval = self.export_interval_secs;
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                tokio::time::Duration::from_secs(interval as u64)
            );
            
            loop {
                interval.tick().await;
                let metrics = {
                    let counters = counters.read().await;
                    let histograms = histograms.read().await;
                    serde_json::json!({
                        "counters": *counters,
                        "histograms": *histograms,
                    })
                };
                tracing::info!("Metrics export: {}", serde_json::to_string(&metrics).unwrap());
            }
        });
        
        Ok(())
    }
    
    async fn handle(
        &self,
        ctx: &ExtensionContext,
        msg: ExtensionMessage,
    ) -> Result<ExtensionResponse> {
        match msg {
            ExtensionMessage::Command { action, .. } => {
                match action {
                    ExtensionAction::Custom { namespace, name, payload } => {
                        if namespace == "metrics" {
                            match name.as_str() {
                                "get" => {
                                    let metrics = self.get_metrics().await;
                                    return Ok(ExtensionResponse {
                                        request_id: uuid::Uuid::nil(),
                                        status: torque_extension::ResponseStatus::Success,
                                        result: Some(metrics),
                                        error: None,
                                    });
                                }
                                "increment" => {
                                    let counter = payload.get("name")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("unknown");
                                    let value = payload.get("value")
                                        .and_then(|v| v.as_u64())
                                        .unwrap_or(1);
                                    self.increment_counter(counter, value).await;
                                }
                                "histogram" => {
                                    let name = payload.get("name")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("unknown");
                                    let value = payload.get("value")
                                        .and_then(|v| v.as_u64())
                                        .unwrap_or(0);
                                    self.record_histogram(name, value).await;
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
                
                Ok(ExtensionResponse {
                    request_id: uuid::Uuid::nil(),
                    status: torque_extension::ResponseStatus::Success,
                    result: Some(serde_json::json!({"processed": true})),
                    error: None,
                })
            }
            _ => Err(ExtensionError::RuntimeError("Unexpected message type".into())),
        }
    }
    
    async fn on_hook(
        &self,
        ctx: &ExtensionContext,
        hook_point: HookPoint,
        context: serde_json::Value,
    ) -> Result<HookResult> {
        match hook_point {
            HookPoint::PreExecution => {
                self.increment_counter("execution.started", 1).await;
            }
            HookPoint::PostExecution => {
                self.increment_counter("execution.completed", 1).await;
                
                // 记录执行时长
                if let Some(start_time) = context.get("payload")
                    .and_then(|p| p.get("instance_id"))
                {
                    // 简化处理：记录完成事件
                    self.record_histogram("execution.duration_ms", 0).await;
                }
            }
            HookPoint::PreToolCall => {
                self.increment_counter("tool.call.total", 1).await;
                
                if let Some(tool_name) = context.get("payload")
                    .and_then(|p| p.get("tool_name"))
                    .and_then(|v| v.as_str())
                {
                    self.increment_counter(&format!("tool.call.{}", tool_name), 1).await;
                }
            }
            HookPoint::PostToolCall => {
                // 记录 Tool 执行时长
                self.record_histogram("tool.call.duration_ms", 0).await;
            }
            _ => {}
        }
        
        Ok(HookResult::Continue)
    }
}
```

---

## 验证清单

### Phase 3 验证
- [ ] ExtensionService 可正常创建
- [ ] 内置 Extension 可加载
- [ ] API 端点可访问
- [ ] Hook 点可触发
- [ ] 消息可路由到 Extension

### Phase 4 验证
- [ ] Logging Extension 记录所有 Hook 事件
- [ ] Metrics Extension 收集执行指标
- [ ] 指标可导出 (通过 API)
- [ ] Extension 可通过配置启用/禁用

---

## 文件清单

```
crates/torque-harness/src/extension/
├── Cargo.toml              # 依赖 torque-extension
├── mod.rs
├── config.rs
├── service.rs
├── runtime_handle.rs
├── hooks.rs
├── api.rs
└── builtin/
    ├── mod.rs
    ├── logging.rs
    └── metrics.rs
```
