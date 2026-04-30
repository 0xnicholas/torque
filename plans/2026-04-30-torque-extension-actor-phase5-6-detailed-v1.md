# Torque Extension Actor 系统 - Phase 5 & 6 详细设计

## 概述

本文档细化 Phase 5 (持久化和恢复) 和 Phase 6 (分布式支持) 的具体实现。

---

## Phase 5: 持久化和恢复

### 5.1 设计目标

Extension 持久化需要解决:
1. Extension 状态快照 (类似 AgentInstance Checkpoint)
2. Extension 注册信息持久化
3. 消息队列持久化 (可选)
4. 版本迁移支持

### 5.2 核心类型

#### 5.2.1 Extension Snapshot

```rust
// crates/torque-extension/src/snapshot.rs

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::{ExtensionId, ExtensionVersion, ExtensionState, ExtensionLifecycle};
use crate::topic::ExtensionTopic;

/// Extension 快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionSnapshot {
    /// Extension ID
    pub id: ExtensionId,
    /// Extension 名称
    pub name: String,
    /// Extension 版本
    pub version: ExtensionVersion,
    /// 生命周期状态
    pub lifecycle: ExtensionLifecycle,
    /// 状态快照
    pub state: ExtensionState,
    /// 订阅的主题
    pub subscribed_topics: Vec<ExtensionTopic>,
    /// 注册的 Hook 点
    pub hook_points: Vec<HookPoint>,
    /// 消息邮箱位置 (用于重放)
    pub mailbox_position: u64,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 快照元数据
    pub metadata: SnapshotMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    /// 快照类型
    pub kind: SnapshotKind,
    /// 快照原因
    pub reason: SnapshotReason,
    /// 关联的 Checkpoint ID (如果有)
    pub checkpoint_id: Option<Uuid>,
    /// 序列号
    pub sequence: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnapshotKind {
    /// 正常快照
    Periodic,
    /// 状态变更快照
    LifecycleTransition,
    /// 与 AgentInstance 同步的快照
    Synced,
    /// 关机前快照
    Shutdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnapshotReason {
    /// 定时快照
    Timer,
    /// 手动触发
    Manual,
    /// 状态转换
    StateTransition,
    /// 与 Checkpoint 同步
    CheckpointSync,
    /// 系统关机
    SystemShutdown,
}

/// Extension 注册信息快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionRegistrySnapshot {
    pub extensions: Vec<ExtensionSnapshot>,
    pub topics: Vec<TopicSubscription>,
    pub hooks: Vec<HookRegistration>,
    pub version: u64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicSubscription {
    pub extension_id: ExtensionId,
    pub topic: ExtensionTopic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookRegistration {
    pub extension_id: ExtensionId,
    pub hook_point: HookPoint,
}
```

#### 5.2.2 Recovery State

```rust
// Extension 恢复状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionRecoveryState {
    /// 快照版本
    pub snapshot_version: u64,
    /// 最后快照时间
    pub last_snapshot_at: DateTime<Utc>,
    /// 恢复模式
    pub mode: RecoveryMode,
    /// 需要重放的扩展消息
    pub pending_messages: Vec<PendingMessage>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryMode {
    /// 从快照恢复
    FromSnapshot,
    /// 从事件重放
    FromEvents,
    /// 快照 + 增量重放
    SnapshotWithReplay,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingMessage {
    pub message_id: Uuid,
    pub source: ExtensionId,
    pub target: ExtensionId,
    pub message: ExtensionMessage,
    pub timestamp: DateTime<Utc>,
}
```

### 5.3 Snapshot Manager

```rust
// crates/torque-extension/src/snapshot.rs

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::Utc;
use uuid::Uuid;

use crate::{ExtensionId, ExtensionActor, ExtensionRuntime};
use crate::topic::ExtensionTopic;
use crate::state::ExtensionState;
use super::{ExtensionSnapshot, SnapshotKind, SnapshotReason, SnapshotMetadata};

/// Snapshot 管理器
pub struct SnapshotManager {
    /// 快照存储
    storage: Arc<dyn SnapshotStorage>,
    /// 快照配置
    config: SnapshotConfig,
    /// 最后快照时间
    last_snapshot_at: RwLock<HashMap<ExtensionId, chrono::DateTime<Utc>>>,
    /// 序列号
    sequence: RwLock<u64>,
}

pub struct SnapshotConfig {
    /// 快照间隔 (秒)
    pub interval_secs: u64,
    /// 最大保留快照数
    pub max_snapshots: usize,
    /// 是否启用定时快照
    pub enable_periodic: bool,
}

impl Default for SnapshotConfig {
    fn default() -> Self {
        Self {
            interval_secs: 300, // 5 分钟
            max_snapshots: 3,
            enable_periodic: true,
        }
    }
}

/// 快照存储接口
#[async_trait::async_trait]
pub trait SnapshotStorage: Send + Sync {
    /// 保存快照
    async fn save(&self, snapshot: &ExtensionSnapshot) -> Result<(), ExtensionError>;
    
    /// 加载最新快照
    async fn load_latest(&self, extension_id: ExtensionId) -> Result<Option<ExtensionSnapshot>, ExtensionError>;
    
    /// 加载指定版本快照
    async fn load_version(&self, extension_id: ExtensionId, version: u64) -> Result<Option<ExtensionSnapshot>, ExtensionError>;
    
    /// 列出所有快照
    async fn list(&self, extension_id: ExtensionId) -> Result<Vec<ExtensionSnapshot>, ExtensionError>;
    
    /// 删除快照
    async fn delete(&self, extension_id: ExtensionId, version: u64) -> Result<(), ExtensionError>;
    
    /// 清理旧快照
    async fn prune(&self, extension_id: ExtensionId, keep: usize) -> Result<(), ExtensionError>;
}

impl SnapshotManager {
    pub fn new(storage: Arc<dyn SnapshotStorage>, config: SnapshotConfig) -> Self {
        Self {
            storage,
            config,
            last_snapshot_at: RwLock::new(HashMap::new()),
            sequence: RwLock::new(0),
        }
    }
    
    /// 创建 Extension 快照
    pub async fn snapshot(
        &self,
        extension: &dyn ExtensionActor,
        state: &ExtensionState,
        subscribed_topics: &[ExtensionTopic],
        mailbox_position: u64,
        kind: SnapshotKind,
        reason: SnapshotReason,
        checkpoint_id: Option<Uuid>,
    ) -> Result<ExtensionSnapshot, ExtensionError> {
        let id = extension.id();
        
        // 更新序列号
        let seq = {
            let mut s = self.sequence.write().await;
            *s += 1;
            *s
        };
        
        let snapshot = ExtensionSnapshot {
            id,
            name: extension.name().to_string(),
            version: extension.version(),
            lifecycle: ExtensionLifecycle::Running, // 假设正在运行
            state: state.clone(),
            subscribed_topics: subscribed_topics.to_vec(),
            hook_points: extension.hook_points(),
            mailbox_position,
            created_at: Utc::now(),
            metadata: SnapshotMetadata {
                kind,
                reason,
                checkpoint_id,
                sequence: seq,
            },
        };
        
        // 保存到存储
        self.storage.save(&snapshot).await?;
        
        // 更新最后快照时间
        {
            let mut last = self.last_snapshot_at.write().await;
            last.insert(id, Utc::now());
        }
        
        // 清理旧快照
        self.storage.prune(id, self.config.max_snapshots).await?;
        
        Ok(snapshot)
    }
    
    /// 从快照恢复 Extension
    pub async fn restore(
        &self,
        extension_id: ExtensionId,
    ) -> Result<Option<ExtensionSnapshot>, ExtensionError> {
        self.storage.load_latest(extension_id).await
    }
    
    /// 检查是否需要快照
    pub async fn should_snapshot(&self, extension_id: ExtensionId) -> bool {
        if !self.config.enable_periodic {
            return false;
        }
        
        let last = self.last_snapshot_at.read().await;
        match last.get(&extension_id) {
            Some(t) => {
                let elapsed = Utc::now().signed_duration_since(*t);
                elapsed.num_seconds() >= self.config.interval_secs as i64
            }
            None => true, // 从未快照过
        }
    }
}

/// Postgres 快照存储实现
pub struct PostgresSnapshotStorage {
    db: sqlx::PgPool,
}

impl PostgresSnapshotStorage {
    pub fn new(db: sqlx::PgPool) -> Self {
        Self { db }
    }
}

#[async_trait::async_trait]
impl SnapshotStorage for PostgresSnapshotStorage {
    async fn save(&self, snapshot: &ExtensionSnapshot) -> Result<(), ExtensionError> {
        let json = serde_json::to_string(snapshot)
            .map_err(|e| ExtensionError::SerializationError(e.to_string()))?;
        
        sqlx::query(
            r#"
            INSERT INTO extension_snapshots (id, extension_id, name, version, json_data, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (extension_id, version) DO UPDATE SET json_data = $5, created_at = $6
            "#,
        )
        .bind(snapshot.id.as_uuid())
        .bind(snapshot.metadata.sequence)
        .bind(&snapshot.name)
        .bind(snapshot.version.to_string())
        .bind(&json)
        .bind(snapshot.created_at)
        .execute(&self.db)
        .await
        .map_err(|e| ExtensionError::RuntimeError(e.to_string()))?;
        
        Ok(())
    }
    
    async fn load_latest(&self, extension_id: ExtensionId) -> Result<Option<ExtensionSnapshot>, ExtensionError> {
        let row = sqlx::query_scalar::<_, sqlx::types::Json<String>>(
            r#"
            SELECT json_data FROM extension_snapshots
            WHERE extension_id = $1
            ORDER BY version DESC
            LIMIT 1
            "#,
        )
        .bind(extension_id.as_uuid())
        .fetch_optional(&self.db)
        .await
        .map_err(|e| ExtensionError::RuntimeError(e.to_string()))?;
        
        match row {
            Some(sqlx::types::Json(json)) => {
                serde_json::from_str(&json)
                    .map(Some)
                    .map_err(|e| ExtensionError::SerializationError(e.to_string()))
            }
            None => Ok(None),
        }
    }
    
    async fn load_version(&self, extension_id: ExtensionId, version: u64) -> Result<Option<ExtensionSnapshot>, ExtensionError> {
        let row = sqlx::query_scalar::<_, sqlx::types::Json<String>>(
            r#"
            SELECT json_data FROM extension_snapshots
            WHERE extension_id = $1 AND version = $2
            "#,
        )
        .bind(extension_id.as_uuid())
        .bind(version as i64)
        .fetch_optional(&self.db)
        .await
        .map_err(|e| ExtensionError::RuntimeError(e.to_string()))?;
        
        match row {
            Some(sqlx::types::Json(json)) => {
                serde_json::from_str(&json)
                    .map(Some)
                    .map_err(|e| ExtensionError::SerializationError(e.to_string()))
            }
            None => Ok(None),
        }
    }
    
    async fn list(&self, extension_id: ExtensionId) -> Result<Vec<ExtensionSnapshot>, ExtensionError> {
        let rows = sqlx::query_scalar::<_, sqlx::types::Json<String>>(
            r#"
            SELECT json_data FROM extension_snapshots
            WHERE extension_id = $1
            ORDER BY version DESC
            "#,
        )
        .bind(extension_id.as_uuid())
        .fetch_all(&self.db)
        .await
        .map_err(|e| ExtensionError::RuntimeError(e.to_string()))?;
        
        rows.into_iter()
            .map(|sqlx::types::Json(json)| {
                serde_json::from_str(&json)
                    .map_err(|e| ExtensionError::SerializationError(e.to_string()))
            })
            .collect()
    }
    
    async fn delete(&self, extension_id: ExtensionId, version: u64) -> Result<(), ExtensionError> {
        sqlx::query(
            "DELETE FROM extension_snapshots WHERE extension_id = $1 AND version = $2"
        )
        .bind(extension_id.as_uuid())
        .bind(version as i64)
        .execute(&self.db)
        .await
        .map_err(|e| ExtensionError::RuntimeError(e.to_string()))?;
        
        Ok(())
    }
    
    async fn prune(&self, extension_id: ExtensionId, keep: usize) -> Result<(), ExtensionError> {
        sqlx::query(
            r#"
            DELETE FROM extension_snapshots
            WHERE extension_id = $1
            AND version NOT IN (
                SELECT version FROM extension_snapshots
                WHERE extension_id = $1
                ORDER BY version DESC
                LIMIT $2
            )
            "#,
        )
        .bind(extension_id.as_uuid())
        .bind(keep as i64)
        .execute(&self.db)
        .await
        .map_err(|e| ExtensionError::RuntimeError(e.to_string()))?;
        
        Ok(())
    }
}
```

### 5.4 Recovery Manager

```rust
// crates/torque-extension/src/recovery.rs

use std::sync::Arc;
use chrono::Utc;

use crate::{
    ExtensionId, ExtensionActor, ExtensionRuntime, ExtensionContext,
    ExtensionLifecycle, ExtensionState, ExtensionTopic,
};
use crate::snapshot::{SnapshotManager, ExtensionSnapshot, SnapshotKind, SnapshotReason};
use crate::error::{ExtensionError, Result};

/// Extension 恢复管理器
pub struct ExtensionRecoveryManager {
    runtime: Arc<dyn ExtensionRuntime>,
    snapshot_manager: Arc<SnapshotManager>,
    /// Extension 工厂 (用于重新创建 Extension 实例)
    extension_factory: Arc<dyn ExtensionFactory>,
    /// 恢复配置
    config: RecoveryConfig,
}

pub struct RecoveryConfig {
    /// 最大重试次数
    pub max_retries: u32,
    /// 重试间隔 (毫秒)
    pub retry_interval_ms: u64,
    /// 是否启用自动恢复
    pub auto_recover: bool,
    /// 恢复超时 (秒)
    pub recovery_timeout_secs: u64,
}

impl Default for RecoveryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_interval_ms: 1000,
            auto_recover: true,
            recovery_timeout_secs: 60,
        }
    }
}

/// Extension 工厂接口
pub trait ExtensionFactory: Send + Sync {
    /// 从快照创建 Extension
    async fn create_from_snapshot(
        &self,
        snapshot: &ExtensionSnapshot,
    ) -> Result<Arc<dyn ExtensionActor>>;
    
    /// 创建新的 Extension
    async fn create(
        &self,
        name: &str,
        config: serde_json::Value,
    ) -> Result<Arc<dyn ExtensionActor>>;
}

/// 恢复结果
#[derive(Debug)]
pub struct RecoveryOutcome {
    pub extension_id: ExtensionId,
    pub success: bool,
    pub restored_state: Option<ExtensionState>,
    pub errors: Vec<String>,
    pub recovered_at: chrono::DateTime<Utc>,
}

impl ExtensionRecoveryManager {
    pub fn new(
        runtime: Arc<dyn ExtensionRuntime>,
        snapshot_manager: Arc<SnapshotManager>,
        extension_factory: Arc<dyn ExtensionFactory>,
        config: RecoveryConfig,
    ) -> Self {
        Self {
            runtime,
            snapshot_manager,
            extension_factory,
            config,
        }
    }
    
    /// 恢复 Extension
    pub async fn recover(&self, extension_id: ExtensionId) -> Result<RecoveryOutcome> {
        // 1. 加载最新快照
        let snapshot = match self.snapshot_manager.restore(extension_id).await? {
            Some(s) => s,
            None => {
                return Ok(RecoveryOutcome {
                    extension_id,
                    success: false,
                    restored_state: None,
                    errors: vec!["No snapshot found".to_string()],
                    recovered_at: Utc::now(),
                });
            }
        };
        
        // 2. 检查 Extension 是否已注册
        let existing = self.runtime.lifecycle(extension_id);
        if existing.is_some() {
            // Extension 已存在，可能需要先注销
            if let Err(e) = self.runtime.unregister(extension_id).await {
                tracing::warn!("Failed to unregister existing extension: {}", e);
            }
        }
        
        // 3. 重新创建 Extension
        let mut retries = 0;
        let mut errors = Vec::new();
        
        while retries < self.config.max_retries {
            match self.extension_factory.create_from_snapshot(&snapshot).await {
                Ok(extension) => {
                    // 4. 重新注册到 Runtime
                    match self.runtime.register(extension).await {
                        Ok(_) => {
                            tracing::info!("Extension {} recovered successfully", extension_id);
                            
                            return Ok(RecoveryOutcome {
                                extension_id,
                                success: true,
                                restored_state: Some(snapshot.state),
                                errors,
                                recovered_at: Utc::now(),
                            });
                        }
                        Err(e) => {
                            errors.push(format!("Registration failed: {}", e));
                            retries += 1;
                        }
                    }
                }
                Err(e) => {
                    errors.push(format!("Creation failed: {}", e));
                    retries += 1;
                }
            }
            
            if retries < self.config.max_retries {
                tokio::time::sleep(
                    tokio::time::Duration::from_millis(
                        self.config.retry_interval_ms * retries as u64
                    )
                ).await;
            }
        }
        
        Ok(RecoveryOutcome {
            extension_id,
            success: false,
            restored_state: None,
            errors,
            recovered_at: Utc::now(),
        })
    }
    
    /// 批量恢复所有 Extension
    pub async fn recover_all(&self) -> Vec<RecoveryOutcome> {
        let ids = self.runtime.list();
        let mut outcomes = Vec::new();
        
        for id in ids {
            let outcome = self.recover(id).await;
            match outcome {
                Ok(o) => outcomes.push(o),
                Err(e) => outcomes.push(RecoveryOutcome {
                    extension_id: id,
                    success: false,
                    restored_state: None,
                    errors: vec![e.to_string()],
                    recovered_at: Utc::now(),
                }),
            }
        }
        
        outcomes
    }
    
    /// 与 AgentInstance Checkpoint 同步
    pub async fn sync_with_checkpoint(
        &self,
        checkpoint_id: Uuid,
        instance_ids: &[AgentInstanceId],
    ) -> Result<()> {
        for id in instance_ids {
            // 触发所有运行中的 Extension 的快照
            let extensions = self.runtime.list();
            
            for ext_id in extensions {
                // 获取 Extension 的运行时句柄并创建快照
                // 这里需要通过 Runtime 获取状态信息
                tracing::debug!(
                    "Syncing extension {} with checkpoint {} for instance {}",
                    ext_id, checkpoint_id, id
                );
            }
        }
        
        Ok(())
    }
}
```

### 5.5 数据库 Schema

```sql
-- Extension 快照表
CREATE TABLE extension_snapshots (
    id UUID PRIMARY KEY,
    extension_id UUID NOT NULL,
    version BIGINT NOT NULL,
    name VARCHAR(255) NOT NULL,
    version_str VARCHAR(50) NOT NULL,
    json_data JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    UNIQUE(extension_id, version)
);

CREATE INDEX idx_extension_snapshots_extension_id ON extension_snapshots(extension_id);
CREATE INDEX idx_extension_snapshots_created_at ON extension_snapshots(created_at);

-- Extension 注册表
CREATE TABLE extension_registry (
    extension_id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL UNIQUE,
    version VARCHAR(50) NOT NULL,
    lifecycle VARCHAR(50) NOT NULL,
    registered_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    metadata JSONB
);

-- Extension 主题订阅表
CREATE TABLE extension_topic_subscriptions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    extension_id UUID NOT NULL REFERENCES extension_registry(extension_id),
    topic_namespace VARCHAR(255) NOT NULL,
    topic_name VARCHAR(255) NOT NULL,
    topic_version INT NOT NULL DEFAULT 1,
    subscribed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    UNIQUE(extension_id, topic_namespace, topic_name, topic_version)
);

-- Extension Hook 注册表
CREATE TABLE extension_hooks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    extension_id UUID NOT NULL REFERENCES extension_registry(extension_id),
    hook_point VARCHAR(100) NOT NULL,
    registered_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    UNIQUE(extension_id, hook_point)
);

-- Extension 消息日志表 (用于重放)
CREATE TABLE extension_message_log (
    id UUID PRIMARY KEY,
    source_extension_id UUID,
    target_extension_id UUID,
    message_type VARCHAR(50) NOT NULL,
    message_data JSONB NOT NULL,
    sequence BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CHECK (source_extension_id IS NOT NULL OR target_extension_id IS NOT NULL)
);

CREATE INDEX idx_extension_message_log_sequence ON extension_message_log(sequence);
CREATE INDEX idx_extension_message_log_target ON extension_message_log(target_extension_id, sequence);
```

---

## Phase 6: 分布式支持

### 6.1 设计目标

Extension 分布式支持需要解决:
1. 跨进程 Extension 通信
2. Extension 位置透明性
3. 负载均衡
4. 服务发现

### 6.2 架构概览

```
┌─────────────────────────────────────────────────────────────────┐
│                    Distributed Extension Runtime                    │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─────────────┐      ┌─────────────┐      ┌─────────────┐     │
│  │   Node A    │      │   Node B    │      │   Node C    │     │
│  │ ┌─────────┐ │      │ ┌─────────┐ │      │ ┌─────────┐ │     │
│  │ │Ext A    │ │      │ │Ext B    │ │      │ │Ext C    │ │     │
│  │ │Ext D    │ │      │ │         │ │      │ │Ext E    │ │     │
│  │ └─────────┘ │      │ └─────────┘ │      │ └─────────┘ │     │
│  └──────┬──────┘      └──────┬──────┘      └──────┬──────┘     │
│         │                     │                     │            │
│         └─────────────────────┼─────────────────────┘            │
│                               ▼                                  │
│                    ┌─────────────────────┐                       │
│                    │   Service Registry  │                       │
│                    │   (Redis/Consul)    │                       │
│                    └─────────────────────┘                       │
│                               │                                  │
│                    ┌──────────┴──────────┐                       │
│                    │   Message Router    │                       │
│                    │   (Redis Streams)   │                       │
│                    └─────────────────────┘                       │
└─────────────────────────────────────────────────────────────────┘
```

### 6.3 Remote Extension Runtime

```rust
// crates/torque-extension/src/distributed/mod.rs

pub mod remote;
pub mod registry;
pub mod router;
pub mod transport;

pub use remote::RemoteExtensionRuntime;
pub use registry::ServiceRegistry;
pub use router::MessageRouter;
pub use transport::{Transport, GrpcTransport, RedisTransport};
```

#### 6.3.1 Transport 接口

```rust
// crates/torque-extension/src/distributed/transport.rs

use async_trait::async_trait;
use crate::{ExtensionMessage, ExtensionResponse, ExtensionEvent};

/// 传输层接口
#[async_trait]
pub trait Transport: Send + Sync {
    /// 发送消息
    async fn send(&self, target: &RemoteEndpoint, message: ExtensionMessage) -> Result<(), TransportError>;
    
    /// 发送请求并等待响应
    async fn call(
        &self,
        target: &RemoteEndpoint,
        request: ExtensionMessage,
        timeout: std::time::Duration,
    ) -> Result<ExtensionResponse, TransportError>;
    
    /// 发布事件
    async fn publish(&self, topic: &str, event: ExtensionEvent) -> Result<(), TransportError>;
    
    /// 订阅主题
    async fn subscribe(
        &self,
        topic: &str,
        handler: Arc<dyn EventHandler>,
    ) -> Result<(), TransportError>;
    
    /// 连接健康检查
    async fn health_check(&self) -> Result<bool, TransportError>;
}

/// 远程端点
#[derive(Debug, Clone)]
pub struct RemoteEndpoint {
    pub node_id: String,
    pub extension_id: ExtensionId,
    pub address: String,
    pub port: u16,
}

/// Redis Stream 传输实现
pub struct RedisTransport {
    client: redis::aio::ConnectionManager,
    stream_prefix: String,
}

impl RedisTransport {
    pub async fn new(redis_url: &str) -> Result<Self, TransportError> {
        let client = redis::Client::open(redis_url)
            .map_err(|e| TransportError::ConnectionError(e.to_string()))?;
        
        let conn = redis::aio::ConnectionManager::new(client)
            .await
            .map_err(|e| TransportError::ConnectionError(e.to_string()))?;
        
        Ok(Self {
            conn,
            stream_prefix: "torque:ext:".to_string(),
        })
    }
    
    fn stream_key(&self, endpoint: &RemoteEndpoint) -> String {
        format!("{}messages:{}:{}", self.stream_prefix, endpoint.node_id, endpoint.extension_id)
    }
}

#[async_trait]
impl Transport for RedisTransport {
    async fn send(&self, target: &RemoteEndpoint, message: ExtensionMessage) -> Result<(), TransportError> {
        let key = self.stream_key(target);
        let data = serde_json::to_string(&message)
            .map_err(|e| TransportError::SerializationError(e.to_string()))?;
        
        redis::cmd("XADD")
            .arg(&key)
            .arg("*")
            .arg("data")
            .arg(&data)
            .query_async(&mut self.conn.clone())
            .await
            .map_err(|e| TransportError::SendError(e.to_string()))?;
        
        Ok(())
    }
    
    async fn call(
        &self,
        target: &RemoteEndpoint,
        request: ExtensionMessage,
        timeout: std::time::Duration,
    ) -> Result<ExtensionResponse, TransportError> {
        // 1. 创建响应队列
        let response_key = format!("{}response:{}", self.stream_prefix, uuid::Uuid::new_v4());
        
        // 2. 发送请求
        let request_data = serde_json::to_string(&request)
            .map_err(|e| TransportError::SerializationError(e.to_string()))?;
        
        redis::cmd("XADD")
            .arg(&self.stream_key(target))
            .arg("*")
            .arg("data")
            .arg(&request_data)
            .arg("response_key")
            .arg(&response_key)
            .query_async(&mut self.conn.clone())
            .await
            .map_err(|e| TransportError::SendError(e.to_string()))?;
        
        // 3. 等待响应
        let result = tokio::time::timeout(
            timeout,
            self.wait_for_response(&response_key)
        ).await
        .map_err(|_| TransportError::Timeout)?;
        
        result
    }
    
    async fn publish(&self, topic: &str, event: ExtensionEvent) -> Result<(), TransportError> {
        let key = format!("{}pubsub:{}", self.stream_prefix, topic);
        let data = serde_json::to_string(&event)
            .map_err(|e| TransportError::SerializationError(e.to_string()))?;
        
        redis::cmd("PUBLISH")
            .arg(&key)
            .arg(&data)
            .query_async(&mut self.conn.clone())
            .await
            .map_err(|e| TransportError::PublishError(e.to_string()))?;
        
        Ok(())
    }
    
    async fn subscribe(
        &self,
        topic: &str,
        handler: Arc<dyn EventHandler>,
    ) -> Result<(), TransportError> {
        // 使用 Redis PubSub
        let client = redis::Client::open("redis://localhost")
            .map_err(|e| TransportError::ConnectionError(e.to_string()))?;
        
        let pubsub = client.get_async_pubsub().await
            .map_err(|e| TransportError::ConnectionError(e.to_string()))?;
        
        let key = format!("{}pubsub:{}", self.stream_prefix, topic);
        let mut pubsub = pubsub;
        
        pubsub.subscribe(&key).await
            .map_err(|e| TransportError::SubscribeError(e.to_string()))?;
        
        // 在后台启动订阅循环
        let stream_prefix = self.stream_prefix.clone();
        tokio::spawn(async move {
            let mut conn = pubsub.into_connection().await;
            let mut pubsub = conn.into_pubsub();
            
            pubsub.subscribe(&key).await.ok();
            
            loop {
                let msg = pubsub.on_message();
                if let Ok(msg) = msg.await {
                    if let Ok(payload) = msg.get_payload::<String>() {
                        if let Ok(event) = serde_json::from_str::<ExtensionEvent>(&payload) {
                            handler.handle(event);
                        }
                    }
                }
            }
        });
        
        Ok(())
    }
    
    async fn health_check(&self) -> Result<bool, TransportError> {
        redis::cmd("PING")
            .query_async(&mut self.conn.clone())
            .await
            .map(|r: String| r == "PONG")
            .map_err(|e| TransportError::HealthCheckError(e.to_string()))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("Connection error: {0}")]
    ConnectionError(String),
    
    #[error("Send error: {0}")]
    SendError(String),
    
    #[error("Timeout")]
    Timeout,
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    #[error("Publish error: {0}")]
    PublishError(String),
    
    #[error("Subscribe error: {0}")]
    SubscribeError(String),
    
    #[error("Health check error: {0}")]
    HealthCheckError(String),
}
```

#### 6.3.2 Service Registry

```rust
// crates/torque-extension/src/distributed/registry.rs

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::Duration;

use crate::ExtensionId;
use super::transport::RemoteEndpoint;

/// 服务注册表
pub struct ServiceRegistry {
    /// 本地节点 ID
    node_id: String,
    /// 注册信息存储 (使用 Redis)
    redis: redis::aio::ConnectionManager,
    /// 本地缓存
    cache: RwLock<HashMap<ExtensionId, RemoteEndpoint>>,
    /// 缓存 TTL
    cache_ttl: Duration,
}

impl ServiceRegistry {
    pub async fn new(node_id: String, redis_url: &str) -> Result<Self, RegistryError> {
        let client = redis::Client::open(redis_url)
            .map_err(|e| RegistryError::ConnectionError(e.to_string()))?;
        
        let conn = redis::aio::ConnectionManager::new(client)
            .await
            .map_err(|e| RegistryError::ConnectionError(e.to_string()))?;
        
        Ok(Self {
            node_id,
            redis: conn,
            cache: RwLock::new(HashMap::new()),
            cache_ttl: Duration::from_secs(30),
        })
    }
    
    /// 注册 Extension
    pub async fn register(
        &self,
        extension_id: ExtensionId,
        address: &str,
        port: u16,
    ) -> Result<(), RegistryError> {
        let endpoint = RemoteEndpoint {
            node_id: self.node_id.clone(),
            extension_id,
            address: address.to_string(),
            port,
        };
        
        let key = format!("extension:{}", extension_id);
        let value = serde_json::to_string(&endpoint)
            .map_err(|e| RegistryError::SerializationError(e.to_string()))?;
        
        redis::cmd("SETEX")
            .arg(&key)
            .arg(self.cache_ttl.as_secs() as i64)
            .arg(&value)
            .query_async(&mut self.redis.clone())
            .await
            .map_err(|e| RegistryError::RegisterError(e.to_string()))?;
        
        // 更新本地缓存
        {
            let mut cache = self.cache.write().await;
            cache.insert(extension_id, endpoint);
        }
        
        // 添加到节点索引
        let node_key = format!("node:{}:extensions", self.node_id);
        redis::cmd("SADD")
            .arg(&node_key)
            .arg(extension_id.as_uuid().to_string())
            .query_async(&mut self.redis.clone())
            .await
            .map_err(|e| RegistryError::RegisterError(e.to_string()))?;
        
        Ok(())
    }
    
    /// 注销 Extension
    pub async fn unregister(&self, extension_id: ExtensionId) -> Result<(), RegistryError> {
        let key = format!("extension:{}", extension_id);
        
        redis::cmd("DEL")
            .arg(&key)
            .query_async(&mut self.redis.clone())
            .await
            .map_err(|e| RegistryError::UnregisterError(e.to_string()))?;
        
        // 更新本地缓存
        {
            let mut cache = self.cache.write().await;
            cache.remove(&extension_id);
        }
        
        // 从节点索引移除
        let node_key = format!("node:{}:extensions", self.node_id);
        redis::cmd("SREM")
            .arg(&node_key)
            .arg(extension_id.as_uuid().to_string())
            .query_async(&mut self.redis.clone())
            .await
            .map_err(|e| RegistryError::UnregisterError(e.to_string()))?;
        
        Ok(())
    }
    
    /// 查找 Extension 位置
    pub async fn lookup(&self, extension_id: ExtensionId) -> Result<Option<RemoteEndpoint>, RegistryError> {
        // 先检查本地缓存
        {
            let cache = self.cache.read().await;
            if let Some(endpoint) = cache.get(&extension_id) {
                return Ok(Some(endpoint.clone()));
            }
        }
        
        // 从 Redis 查找
        let key = format!("extension:{}", extension_id);
        let value: Option<String> = redis::cmd("GET")
            .arg(&key)
            .query_async(&mut self.redis.clone())
            .await
            .map_err(|e| RegistryError::LookupError(e.to_string()))?;
        
        match value {
            Some(v) => {
                let endpoint: RemoteEndpoint = serde_json::from_str(&v)
                    .map_err(|e| RegistryError::SerializationError(e.to_string()))?;
                
                // 更新本地缓存
                {
                    let mut cache = self.cache.write().await;
                    cache.insert(extension_id, endpoint.clone());
                }
                
                Ok(Some(endpoint))
            }
            None => Ok(None),
        }
    }
    
    /// 列出节点上的所有 Extension
    pub async fn list_node_extensions(&self) -> Result<Vec<ExtensionId>, RegistryError> {
        let node_key = format!("node:{}:extensions", self.node_id);
        let ids: Vec<String> = redis::cmd("SMEMBERS")
            .arg(&node_key)
            .query_async(&mut self.redis.clone())
            .await
            .map_err(|e| RegistryError::LookupError(e.to_string()))?;
        
        ids.into_iter()
            .filter_map(|s| uuid::Uuid::parse_str(&s).ok())
            .map(|u| ExtensionId::from_uuid(u))
            .collect::<Vec<_>>();
        
        Ok(result)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("Connection error: {0}")]
    ConnectionError(String),
    
    #[error("Register error: {0}")]
    RegisterError(String),
    
    #[error("Unregister error: {0}")]
    UnregisterError(String),
    
    #[error("Lookup error: {0}")]
    LookupError(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
}
```

#### 6.3.3 Remote Runtime

```rust
// crates/torque-extension/src/distributed/remote.rs

use std::sync::Arc;
use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::{
    ExtensionId, ExtensionTopic, ExtensionActor, ExtensionLifecycle,
    ExtensionMessage, ExtensionRequest, ExtensionResponse, ExtensionEvent,
    HookPoint, state::ExtensionState, error::{ExtensionError, Result},
};
use super::{ExtensionRuntime, Transport, ServiceRegistry, RemoteEndpoint};

/// 远程 Extension Runtime
pub struct RemoteExtensionRuntime {
    /// 本地 Extension Runtime (用于同进程通信)
    local: Arc<dyn ExtensionRuntime>,
    /// 传输层
    transport: Arc<dyn Transport>,
    /// 服务注册表
    registry: Arc<ServiceRegistry>,
    /// 本地节点 ID
    node_id: String,
    /// 远程 Extension 缓存
    remote_cache: RwLock<std::collections::HashMap<ExtensionId, RemoteEndpoint>>,
}

impl RemoteExtensionRuntime {
    pub fn new(
        local: Arc<dyn ExtensionRuntime>,
        transport: Arc<dyn Transport>,
        registry: Arc<ServiceRegistry>,
        node_id: String,
    ) -> Self {
        Self {
            local,
            transport,
            registry,
            node_id,
            remote_cache: RwLock::new(std::collections::HashMap::new()),
        }
    }
    
    /// 判断目标是否在本地
    fn is_local(&self, id: ExtensionId) -> bool {
        // 检查是否是本节点注册的 Extension
        let cache = self.remote_cache.blocking_read();
        match cache.get(&id) {
            Some(endpoint) => endpoint.node_id == self.node_id,
            None => true, // 假设未缓存的都是本地的
        }
    }
    
    /// 路由到正确的目标
    async fn route(&self, id: ExtensionId) -> Result<RemoteEndpoint> {
        // 先检查缓存
        {
            let cache = self.remote_cache.read().await;
            if let Some(endpoint) = cache.get(&id) {
                return Ok(endpoint.clone());
            }
        }
        
        // 从注册表查找
        match self.registry.lookup(id).await? {
            Some(endpoint) => {
                let mut cache = self.remote_cache.write().await;
                cache.insert(id, endpoint.clone());
                Ok(endpoint)
            }
            None => Err(ExtensionError::NotFound(id)),
        }
    }
}

#[async_trait]
impl ExtensionRuntime for RemoteExtensionRuntime {
    async fn register(&self, extension: Arc<dyn ExtensionActor>) -> Result<ExtensionId> {
        // 先注册到本地
        let id = self.local.register(extension).await?;
        
        // 注册到服务发现
        // 注意：这里需要知道监听地址
        let addr = "0.0.0.0".to_string(); // 应该从配置获取
        let port = 9090; // 应该从配置获取
        
        self.registry.register(id, &addr, port).await?;
        
        Ok(id)
    }
    
    async fn unregister(&self, id: ExtensionId) -> Result<()> {
        // 从服务发现注销
        self.registry.unregister(id).await?;
        
        // 从本地注销
        self.local.unregister(id).await
    }
    
    async fn send(&self, target: ExtensionId, msg: ExtensionMessage) -> Result<()> {
        if self.is_local(target) {
            self.local.send(target, msg).await
        } else {
            let endpoint = self.route(target).await?;
            self.transport.send(&endpoint, msg).await
                .map_err(|e| ExtensionError::RuntimeError(e.to_string()))
        }
    }
    
    async fn call(&self, target: ExtensionId, req: ExtensionRequest) -> Result<ExtensionResponse> {
        if self.is_local(target) {
            self.local.call(target, req).await
        } else {
            let endpoint = self.route(target).await?;
            let timeout = std::time::Duration::from_millis(
                req.timeout_ms.unwrap_or(30_000)
            );
            self.transport.call(&endpoint, ExtensionMessage::Request {
                request_id: req.request_id,
                action: req.action,
                reply_to: req.reply_to,
                timeout_ms: req.timeout_ms,
            }, timeout).await
                .map_err(|e| ExtensionError::RuntimeError(e.to_string()))
        }
    }
    
    async fn publish(&self, topic: ExtensionTopic, event: ExtensionEvent) -> Result<()> {
        // 本地发布
        self.local.publish(topic.clone(), event.clone()).await?;
        
        // 远程发布
        self.transport.publish(&topic.to_string(), event).await
            .map_err(|e| ExtensionError::RuntimeError(e.to_string()))
    }
    
    async fn subscribe(&self, id: ExtensionId, topic: ExtensionTopic) -> Result<()> {
        // 订阅本地主题
        self.local.subscribe(id, topic.clone()).await?;
        
        // 如果是远程 Extension，添加远程订阅处理
        if !self.is_local(id) {
            // 远程订阅通过消息路由实现
        }
        
        Ok(())
    }
    
    fn find(&self, name: &str) -> Option<ExtensionId> {
        self.local.find(name)
    }
    
    fn list(&self) -> Vec<ExtensionId> {
        // 合并本地和远程 Extension 列表
        let local_ids = self.local.list();
        
        // 从注册表获取远程 Extension (简化实现)
        // 实际应该异步获取
        local_ids
    }
    
    fn lifecycle(&self, id: ExtensionId) -> Option<ExtensionLifecycle> {
        if self.is_local(id) {
            self.local.lifecycle(id)
        } else {
            // 远程 Extension 假设都是 Running
            Some(ExtensionLifecycle::Running)
        }
    }
    
    fn snapshot(&self, id: ExtensionId) -> Result<ExtensionState> {
        if self.is_local(id) {
            self.local.snapshot(id)
        } else {
            Err(ExtensionError::RuntimeError("Cannot snapshot remote extension".into()))
        }
    }
    
    async fn register_hook(&self, id: ExtensionId, hook_point: HookPoint) -> Result<()> {
        if self.is_local(id) {
            self.local.register_hook(id, hook_point).await
        } else {
            Err(ExtensionError::RuntimeError("Cannot register hook on remote extension".into()))
        }
    }
    
    async fn trigger_hook(&self, hook_point: HookPoint, context: serde_json::Value) -> Result<()> {
        // 本地触发 Hook
        self.local.trigger_hook(hook_point, context).await
    }
}
```

### 6.4 负载均衡

```rust
// crates/torque-extension/src/distributed/load_balancer.rs

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 负载均衡策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadBalancingStrategy {
    /// 轮询
    RoundRobin,
    /// 最少连接
    LeastConnections,
    /// 随机
    Random,
    /// 权重随机
    WeightedRandom,
    /// 一致性哈希
    ConsistentHash,
}

/// 负载均衡器
pub struct LoadBalancer {
    strategy: LoadBalancingStrategy,
    /// 每个 Extension 的连接数
    connection_counts: RwLock<HashMap<ExtensionId, usize>>,
    /// 权重配置
    weights: RwLock<HashMap<ExtensionId, u32>>,
    /// 轮询计数器
    round_robin_counter: RwLock<usize>,
}

impl LoadBalancer {
    pub fn new(strategy: LoadBalancingStrategy) -> Self {
        Self {
            strategy,
            connection_counts: RwLock::new(HashMap::new()),
            weights: RwLock::new(HashMap::new()),
            round_robin_counter: RwLock::new(0),
        }
    }
    
    /// 选择目标 Extension
    pub async fn select(&self, targets: &[ExtensionId]) -> Option<ExtensionId> {
        if targets.is_empty() {
            return None;
        }
        
        match self.strategy {
            LoadBalancingStrategy::RoundRobin => {
                let mut counter = self.round_robin_counter.write().await;
                let index = *counter % targets.len();
                *counter += 1;
                Some(targets[index])
            }
            
            LoadBalancingStrategy::LeastConnections => {
                let counts = self.connection_counts.read().await;
                targets.iter()
                    .min_by_key(|id| counts.get(*id).copied().unwrap_or(0))
                    .copied()
            }
            
            LoadBalancingStrategy::Random => {
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};
                
                let mut hasher = DefaultHasher::new();
                std::time::SystemTime::now().hash(&mut hasher);
                let hash = hasher.finish();
                let index = (hash as usize) % targets.len();
                Some(targets[index])
            }
            
            LoadBalancingStrategy::WeightedRandom => {
                let weights = self.weights.read().await;
                let total_weight: u32 = targets.iter()
                    .map(|id| weights.get(id).copied().unwrap_or(1))
                    .sum();
                
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};
                let mut hasher = DefaultHasher::new();
                std::time::SystemTime::now().hash(&mut hasher);
                let hash = hasher.finish();
                let mut rand = (hash % total_weight) as i32;
                
                for id in targets {
                    let weight = weights.get(id).copied().unwrap_or(1) as i32;
                    rand -= weight;
                    if rand < 0 {
                        return Some(*id);
                    }
                }
                
                Some(targets[0])
            }
            
            LoadBalancingStrategy::ConsistentHash => {
                // 简化实现：返回第一个目标
                Some(targets[0])
            }
        }
    }
    
    /// 记录连接
    pub async fn record_connection(&self, id: ExtensionId) {
        let mut counts = self.connection_counts.write().await;
        *counts.entry(id).or_insert(0) += 1;
    }
    
    /// 释放连接
    pub async fn release_connection(&self, id: ExtensionId) {
        let mut counts = self.connection_counts.write().await;
        if let Some(count) = counts.get_mut(&id) {
            *count = count.saturating_sub(1);
        }
    }
    
    /// 设置权重
    pub async fn set_weight(&self, id: ExtensionId, weight: u32) {
        let mut weights = self.weights.write().await;
        weights.insert(id, weight);
    }
}
```

### 6.5 完整文件结构

```
crates/torque-extension/
├── src/
│   ├── lib.rs
│   ├── id.rs
│   ├── actor.rs
│   ├── context.rs
│   ├── error.rs
│   ├── hook.rs
│   ├── message.rs
│   ├── state.rs
│   ├── topic.rs
│   ├── runtime/
│   │   ├── mod.rs
│   │   ├── trait.rs
│   │   ├── in_memory.rs
│   │   └── mailbox.rs
│   ├── snapshot/          # Phase 5
│   │   ├── mod.rs
│   │   ├── manager.rs
│   │   ├── storage.rs
│   │   └── recovery.rs
│   └── distributed/        # Phase 6
│       ├── mod.rs
│       ├── remote.rs
│       ├── registry.rs
│       ├── router.rs
│       ├── transport.rs
│       └── load_balancer.rs
```

---

## 验证清单

### Phase 5 验证
- [ ] SnapshotManager 可创建和保存快照
- [ ] Extension 可从快照恢复
- [ ] 数据库 Schema 迁移正确
- [ ] 快照清理策略生效
- [ ] 与 AgentInstance Checkpoint 同步工作

### Phase 6 验证
- [ ] RemoteExtensionRuntime 可注册和查找 Extension
- [ ] 跨进程消息可达
- [ ] ServiceRegistry 正确维护 Extension 位置
- [ ] 负载均衡策略正确选择目标
- [ ] 连接故障时正确处理

---

## 技术风险和缓解

### Phase 5 风险

| 风险 | 影响 | 缓解 |
|------|------|------|
| 快照版本冲突 | Extension 无法恢复 | 实现版本协商机制 |
| 大状态快照 | 内存/存储压力 | 实现增量快照 |
| 并发快照 | 数据不一致 | 使用 RwLock + Copy-on-Write |

### Phase 6 风险

| 风险 | 影响 | 缓解 |
|------|------|------|
| 网络分区 | Extension 不可达 | 本地缓存 + 最终一致性 |
| 消息丢失 | 通信不可靠 | 消息确认 + 重试机制 |
| 服务发现延迟 | 路由错误 | 缓存 + TTL 过期策略 |
