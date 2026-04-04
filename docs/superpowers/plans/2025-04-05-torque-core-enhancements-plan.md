# Torque Core Enhancements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement three core enhancements: Checkpointer (state snapshots + crash recovery), Virtual File System (VFS), and Context Manager (intelligent context optimization)

**Architecture:** Three independent but composable modules. Checkpointer provides state persistence for crash recovery. VFS wraps ContextStore with path-based abstraction. ContextManager optimizes LLM context to prevent token overflow. All three integrate into AgentRuntime.

**Tech Stack:** Rust (tokio async), sqlx, redis-rs, aws-sdk-s3

---

## File Structure Overview

```
crates/
├── checkpointer/           # NEW
│   ├── src/
│   │   ├── lib.rs         # Public exports
│   │   ├── trait.rs       # Checkpointer trait + types
│   │   ├── hybrid.rs      # HybridCheckpointer impl
│   │   ├── postgres.rs    # PostgreSQLCheckpointer impl
│   │   └── redis.rs       # RedisCheckpointer impl
│   ├── Cargo.toml
│   └── migrations/
│       └── 001_create_checkpoints.sql
├── context-store/
│   ├── src/
│   │   ├── lib.rs
│   │   ├── store.rs       # Existing ContextStore
│   │   ├── vfs.rs         # NEW: VfsOverlay
│   │   └── vfs_metadata.rs # NEW: VFS metadata operations
│   └── migrations/
│       └── 002_create_vfs_metadata.sql
├── agent-runtime/
│   ├── src/
│   │   ├── runtime.rs      # Existing AgentRuntime
│   │   ├── context_mgr.rs  # NEW: ContextManager
│   │   └── tools/
│   │       ├── mod.rs
│   │       ├── context_tools.rs   # NEW: compress, summarize tools
│   │       └── vfs_tools.rs       # NEW: read_file, write_file, etc.
│   └── migrations/
│       └── 003_add_context_fields.sql
```

---

## Phase 1: Checkpointer (Week 1)

### Task 1: Create checkpointer crate scaffold

**Files:**
- Create: `crates/checkpointer/Cargo.toml`
- Create: `crates/checkpointer/src/lib.rs`

- [ ] **Step 1: Create Cargo.toml**

```toml
[package]
name = "checkpointer"
version = "0.1.0"
edition = "2021"

[dependencies]
types = { path = "../types" }
db = { path = "../db" }
async-trait = "0.1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4", "serde"] }
sha2 = "0.10"
redis = { version = "0.25", features = ["tokio-comp", "connection-manager"] }
thiserror = "1"
anyhow = "1"
tracing = "0.1"
```

- [ ] **Step 2: Create lib.rs with public exports**

```rust
pub mod trait;
pub mod hybrid;
pub mod postgres;
pub mod redis;

pub use trait::{Checkpointer, CheckpointState, CheckpointMeta, CheckpointId};
pub use hybrid::HybridCheckpointer;
pub use postgres::PostgreSQLCheckpointer;
pub use redis::RedisCheckpointer;
```

- [ ] **Step 3: Commit**

```bash
git add crates/checkpointer/
git commit -m "feat(checkpointer): create checkpointer crate scaffold"
```

---

### Task 2: Define Checkpointer trait and types

**Files:**
- Create: `crates/checkpointer/src/trait.rs`
- Create: `crates/checkpointer/src/error.rs`

- [ ] **Step 1: Create error.rs**

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CheckpointerError {
    #[error("Checkpoint not found: {0}")]
    NotFound(String),
    
    #[error("Storage error: {0}")]
    Storage(String),
    
    #[error("Serialization error: {0}")]
    Serialization(String),
    
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),
    
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

pub type Result<T> = std::result::Result<T, CheckpointerError>;
```

- [ ] **Step 2: Create trait.rs**

```rust
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointState {
    pub messages: Vec<Message>,
    pub tool_call_count: u32,
    pub intermediate_results: Vec<ArtifactPointer>,
    pub custom_state: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactPointer {
    pub task_id: String,
    pub storage: String,
    pub location: String,
    pub size_bytes: i64,
    pub content_type: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CheckpointId(pub Uuid);

impl CheckpointId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointMeta {
    pub id: CheckpointId,
    pub run_id: Uuid,
    pub node_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub state_hash: String,
}

#[async_trait]
pub trait Checkpointer: Send + Sync {
    async fn save(
        &self,
        run_id: Uuid,
        node_id: Uuid,
        state: CheckpointState,
    ) -> Result<CheckpointId>;
    
    async fn load(&self, checkpoint_id: CheckpointId) -> Result<CheckpointState>;
    
    async fn list_run_checkpoints(&self, run_id: Uuid) -> Result<Vec<CheckpointMeta>>;
    
    async fn list_node_checkpoints(&self, node_id: Uuid) -> Result<Vec<CheckpointMeta>>;
    
    async fn delete(&self, checkpoint_id: CheckpointId) -> Result<()>;
}
```

- [ ] **Step 3: Run cargo check to verify compilation**

```bash
cd crates/checkpointer && cargo check
```

Expected: Compiles successfully

- [ ] **Step 4: Commit**

```bash
git add crates/checkpointer/src/trait.rs crates/checkpointer/src/error.rs
git commit -m "feat(checkpointer): define Checkpointer trait and types"
```

---

### Task 3: Implement HybridCheckpointer

**Files:**
- Create: `crates/checkpointer/src/hybrid.rs`
- Create: `crates/checkpointer/migrations/001_create_checkpoints.sql`

- [ ] **Step 1: Create migration**

```sql
CREATE TABLE checkpoints (
    id            UUID PRIMARY KEY,
    run_id        UUID REFERENCES runs,
    node_id       UUID REFERENCES nodes,
    tenant_id     UUID REFERENCES tenants,
    state_hash    TEXT NOT NULL,
    storage       TEXT NOT NULL,
    location      TEXT NOT NULL,
    created_at    TIMESTAMPTZ DEFAULT NOW(),
    expires_at    TIMESTAMPTZ
);

CREATE INDEX idx_checkpoints_run_id ON checkpoints(run_id);
CREATE INDEX idx_checkpoints_node_id ON checkpoints(node_id);
CREATE INDEX idx_checkpoints_expires_at ON checkpoints(expires_at);
```

- [ ] **Step 2: Create hybrid.rs**

```rust
use async_trait::async_trait;
use chrono::{Duration, Utc};
use sha2::{Sha256, Digest};
use uuid::Uuid;

use crate::error::{CheckpointerError, Result};
use crate::trait::{CheckpointId, CheckpointMeta, CheckpointState, Checkpointer};

pub struct HybridCheckpointer {
    pool: sqlx::PgPool,
    redis: redis::aio::ConnectionManager,
}

impl HybridCheckpointer {
    pub fn new(pool: sqlx::PgPool, redis: redis::aio::ConnectionManager) -> Self {
        Self { pool, redis }
    }
    
    fn compute_hash(state: &CheckpointState) -> String {
        let mut hasher = Sha256::new();
        hasher.update(serde_json::to_string(state).unwrap_or_default());
        format!("{:x}", hasher.finalize())
    }
    
    fn redis_key(tenant_id: &Uuid, checkpoint_id: &CheckpointId) -> String {
        format!("{}:checkpoint:{}", tenant_id, checkpoint_id.0)
    }
}

#[async_trait]
impl Checkpointer for HybridCheckpointer {
    async fn save(
        &self,
        run_id: Uuid,
        node_id: Uuid,
        state: CheckpointState,
    ) -> Result<CheckpointId> {
        let checkpoint_id = CheckpointId::new();
        let state_hash = Self::compute_hash(&state);
        let redis_key = Self::redis_key(&Uuid::nil(), &checkpoint_id); // tenant_id from context
        
        // Store state snapshot in Redis (TTL 24h)
        let state_json = serde_json::to_string(&state).map_err(|e| {
            CheckpointerError::Serialization(e.to_string())
        })?;
        
        let mut conn = self.redis.clone();
        redis::cmd("SETEX")
            .arg(&redis_key)
            .arg(86400) // 24h TTL
            .arg(&state_json)
            .query_async(&mut conn)
            .await?;
        
        // Store metadata in PostgreSQL
        sqlx::query(
            r#"
            INSERT INTO checkpoints (id, run_id, node_id, tenant_id, state_hash, storage, location, created_at, expires_at)
            VALUES ($1, $2, $3, $4, $5, 'redis', $6, NOW(), NOW() + INTERVAL '24 hours')
            "#,
        )
        .bind(checkpoint_id.0)
        .bind(run_id)
        .bind(node_id)
        .bind(Uuid::nil()) // tenant_id - will be passed properly
        .bind(&state_hash)
        .bind(&redis_key)
        .execute(&self.pool)
        .await?;
        
        Ok(checkpoint_id)
    }
    
    async fn load(&self, checkpoint_id: CheckpointId) -> Result<CheckpointState> {
        // Load from Redis
        let redis_key = format!("checkpoint:{}", checkpoint_id.0);
        let mut conn = self.redis.clone();
        
        let state_json: Option<String> = redis::cmd("GET")
            .arg(&redis_key)
            .query_async(&mut conn)
            .await?;
        
        match state_json {
            Some(json) => {
                serde_json::from_str(&json).map_err(|e| {
                    CheckpointerError::Serialization(e.to_string())
                })
            }
            None => Err(CheckpointerError::NotFound(checkpoint_id.0.to_string())),
        }
    }
    
    async fn list_run_checkpoints(&self, run_id: Uuid) -> Result<Vec<CheckpointMeta>> {
        let rows = sqlx::query_as::<_, (Uuid, Uuid, Uuid, chrono::DateTime<Utc>, String)>(
            "SELECT id, run_id, node_id, created_at, state_hash FROM checkpoints WHERE run_id = $1 ORDER BY created_at DESC",
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await?;
        
        Ok(rows
            .into_iter()
            .map(|(id, run_id, node_id, created_at, state_hash)| CheckpointMeta {
                id: CheckpointId(id),
                run_id,
                node_id,
                created_at,
                state_hash,
            })
            .collect())
    }
    
    async fn list_node_checkpoints(&self, node_id: Uuid) -> Result<Vec<CheckpointMeta>> {
        let rows = sqlx::query_as::<_, (Uuid, Uuid, Uuid, chrono::DateTime<Utc>, String)>(
            "SELECT id, run_id, node_id, created_at, state_hash FROM checkpoints WHERE node_id = $1 ORDER BY created_at DESC",
        )
        .bind(node_id)
        .fetch_all(&self.pool)
        .await?;
        
        Ok(rows
            .into_iter()
            .map(|(id, run_id, node_id, created_at, state_hash)| CheckpointMeta {
                id: CheckpointId(id),
                run_id,
                node_id,
                created_at,
                state_hash,
            })
            .collect())
    }
    
    async fn delete(&self, checkpoint_id: CheckpointId) -> Result<()> {
        let redis_key = format!("checkpoint:{}", checkpoint_id.0);
        
        // Delete from Redis
        let mut conn = self.redis.clone();
        let _: () = redis::cmd("DEL")
            .arg(&redis_key)
            .query_async(&mut conn)
            .await?;
        
        // Delete from PostgreSQL
        sqlx::query("DELETE FROM checkpoints WHERE id = $1")
            .bind(checkpoint_id.0)
            .execute(&self.pool)
            .await?;
        
        Ok(())
    }
}
```

- [ ] **Step 3: Run cargo check**

```bash
cd crates/checkpointer && cargo check
```

- [ ] **Step 4: Commit**

```bash
git add crates/checkpointer/src/hybrid.rs crates/checkpointer/migrations/001_create_checkpoints.sql
git commit -m "feat(checkpointer): implement HybridCheckpointer with PostgreSQL + Redis"
```

---

### Task 4: Integrate Checkpointer into AgentRuntime

**Files:**
- Modify: `crates/agent-runtime/src/runtime.rs`

- [ ] **Step 1: Add Checkpointer field to AgentRuntime**

```rust
pub struct AgentRuntime {
    // ... existing fields
    checkpointer: Arc<dyn Checkpointer>,
    checkpoint_interval_secs: u64,
}
```

- [ ] **Step 2: Add checkpoint after tool call**

```rust
// In tool call loop, after successful tool execution:
if self.should_checkpoint(tool_call_count, last_checkpoint_time) {
    let state = CheckpointState {
        messages: context_manager.get_full_history(),
        tool_call_count,
        intermediate_results: vec![], // from context
        custom_state: None,
    };
    self.checkpointer.save(run_id, node_id, state).await?;
}
```

- [ ] **Step 3: Update crash recovery in Executor**

```rust
// In Executor startup:
async fn recover_crashed_nodes(&self) {
    let running_nodes = self.db.get_running_nodes_older_than(Duration::hours(24)).await?;
    for node in running_nodes {
        if let Some(checkpoint) = self.checkpointer.get_latest(node.id).await? {
            // Restore state and resume
            self.restore_from_checkpoint(node, checkpoint).await?;
        } else {
            // Reset to PENDING, follow failure policy
            self.reset_node_to_pending(node).await?;
        }
    }
}
```

- [ ] **Step 4: Run cargo check on entire workspace**

```bash
cargo check --workspace
```

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(checkpointer): integrate into AgentRuntime and Executor crash recovery"
```

---

## Phase 2: Virtual File System (Week 2)

### Task 5: Create VFS trait and VfsOverlay

**Files:**
- Create: `crates/context-store/src/vfs.rs`
- Create: `crates/context-store/src/vfs_metadata.rs`
- Create: `crates/context-store/migrations/002_create_vfs_metadata.sql`

- [ ] **Step 1: Create VFS trait in vfs.rs**

```rust
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMeta {
    pub path: String,
    pub size_bytes: u64,
    pub content_type: String,
    pub modified_at: DateTime<Utc>,
    pub is_directory: bool,
}

#[async_trait]
pub trait VirtualFileSystem: Send + Sync {
    async fn read(&self, path: &str) -> Result<Vec<u8>, VfsError>;
    async fn write(&self, path: &str, content: &[u8]) -> Result<ArtifactPointer, VfsError>;
    async fn list(&self, dir: &str) -> Result<Vec<FileMeta>, VfsError>;
    async fn exists(&self, path: &str) -> Result<bool, VfsError>;
    async fn delete(&self, path: &str) -> Result<(), VfsError>;
    async fn copy(&self, from: &str, to: &str) -> Result<ArtifactPointer, VfsError>;
}

#[derive(Debug, thiserror::Error)]
pub enum VfsError {
    #[error("Path not found: {0}")]
    NotFound(String),
    
    #[error("Concurrent write conflict: {0}")]
    ConcurrentWriteConflict(String),
    
    #[error("Storage error: {0}")]
    Storage(String),
    
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
}
```

- [ ] **Step 2: Create VfsOverlay implementation**

```rust
pub struct VfsOverlay {
    context_store: Arc<dyn ContextStore>,
    pool: sqlx::PgPool,
    redis: redis::aio::ConnectionManager,
    tenant_id: Uuid,
    run_id: Uuid,
    node_id: Option<Uuid>, // Current node context
}

impl VfsOverlay {
    pub fn new(
        context_store: Arc<dyn ContextStore>,
        pool: sqlx::PgPool,
        redis: redis::aio::ConnectionManager,
        tenant_id: Uuid,
        run_id: Uuid,
        node_id: Option<Uuid>,
    ) -> Self {
        Self {
            context_store,
            pool,
            redis,
            tenant_id,
            run_id,
            node_id,
        }
    }
    
    fn resolve_path(&self, path: &str) -> Result<String, VfsError> {
        // Handle shortcuts
        if path.starts_with("/workspace/") {
            return Ok(format!(
                "/{}/{}/workspace/{}",
                self.tenant_id,
                self.run_id, 
                &path[11:]
            ));
        }
        if path.starts_with("/output/") {
            let node_id = self.node_id.ok_or(VfsError::PermissionDenied(
                "Cannot use /output/ without node context".into()
            ))?;
            return Ok(format!(
                "/{}/{}/{}/output/{}",
                self.tenant_id,
                self.run_id,
                node_id,
                &path[8:]
            ));
        }
        if path.starts_with("/temp/") {
            return Ok(format!(
                "/{}/{}/temp/{}",
                self.tenant_id,
                self.run_id,
                &path[6:]
            ));
        }
        
        // Already absolute path
        Ok(path.to_string())
    }
    
    fn get_lock_key(&self, resolved_path: &str) -> String {
        format!("{}:vfs:lock:{}", self.tenant_id, resolved_path)
    }
}

#[async_trait]
impl VirtualFileSystem for VfsOverlay {
    async fn read(&self, path: &str) -> Result<Vec<u8>, VfsError> {
        let resolved = self.resolve_path(path)?;
        
        // Get artifact pointer from metadata
        let artifact_id = self.get_artifact_id(&resolved).await?;
        
        // Read via ContextStore
        self.context_store.read(&artifact_id).await.map_err(|e| VfsError::Storage(e.to_string()))
    }
    
    async fn write(&self, path: &str, content: &[u8]) -> Result<ArtifactPointer, VfsError> {
        let resolved = self.resolve_path(path)?;
        
        // Acquire lock for shared paths
        if resolved.contains("/workspace/") {
            let lock_key = self.get_lock_key(&resolved);
            let mut conn = self.redis.clone();
            
            let acquired: bool = redis::cmd("SET")
                .arg(&lock_key)
                .arg("locked")
                .arg("NX")
                .arg("EX")
                .arg(30)
                .query_async(&mut conn)
                .await
                .map_err(|e| VfsError::Storage(e.to_string()))?;
            
            if !acquired {
                return Err(VfsError::ConcurrentWriteConflict(resolved));
            }
        }
        
        // Write via ContextStore
        let pointer = self.context_store.write(content).await
            .map_err(|e| VfsError::Storage(e.to_string()))?;
        
        // Update VFS metadata
        self.update_metadata(&resolved, &pointer).await?;
        
        // Release lock
        if resolved.contains("/workspace/") {
            let lock_key = self.get_lock_key(&resolved);
            let mut conn = self.redis.clone();
            let _: () = redis::cmd("DEL")
                .arg(&lock_key)
                .query_async(&mut conn)
                .await
                .map_err(|e| VfsError::Storage(e.to_string()))?;
        }
        
        Ok(pointer.into())
    }
    
    async fn list(&self, dir: &str) -> Result<Vec<FileMeta>, VfsError> {
        let resolved = self.resolve_path(dir)?;
        
        let rows = sqlx::query_as::<_, (String, i64, chrono::DateTime<Utc>)>(
            "SELECT path, size_bytes, modified_at FROM vfs_metadata WHERE path LIKE $1 AND is_directory = false",
        )
        .bind(format!("{}%", resolved))
        .fetch_all(&self.pool)
        .await
        .map_err(|e| VfsError::Storage(e.to_string()))?;
        
        Ok(rows
            .into_iter()
            .map(|(path, size_bytes, modified_at)| FileMeta {
                path,
                size_bytes: size_bytes as u64,
                content_type: "application/octet-stream".into(),
                modified_at,
                is_directory: false,
            })
            .collect())
    }
    
    async fn exists(&self, path: &str) -> Result<bool, VfsError> {
        let resolved = self.resolve_path(path)?;
        
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM vfs_metadata WHERE path = $1",
        )
        .bind(&resolved)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| VfsError::Storage(e.to_string()))?;
        
        Ok(count > 0)
    }
    
    async fn delete(&self, path: &str) -> Result<(), VfsError> {
        let resolved = self.resolve_path(path)?;
        
        sqlx::query("DELETE FROM vfs_metadata WHERE path = $1")
            .bind(&resolved)
            .execute(&self.pool)
            .await
            .map_err(|e| VfsError::Storage(e.to_string()))?;
        
        Ok(())
    }
    
    async fn copy(&self, from: &str, to: &str) -> Result<ArtifactPointer, VfsError> {
        let content = self.read(from).await?;
        self.write(to, &content).await
    }
}
```

- [ ] **Step 3: Create migration**

```sql
CREATE TABLE vfs_metadata (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    run_id        UUID REFERENCES runs,
    node_id       UUID REFERENCES nodes,    -- NULL means shared file
    path          TEXT NOT NULL,
    artifact_id   UUID REFERENCES artifacts,
    is_directory  BOOLEAN DEFAULT false,
    created_at    TIMESTAMPTZ DEFAULT NOW(),
    modified_at   TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(run_id, path)
);

CREATE INDEX idx_vfs_metadata_run_id ON vfs_metadata(run_id);
CREATE INDEX idx_vfs_metadata_node_id ON vfs_metadata(node_id);
CREATE INDEX idx_vfs_metadata_path ON vfs_metadata(path);
```

- [ ] **Step 4: Run cargo check**

```bash
cd crates/context-store && cargo check
```

- [ ] **Step 5: Commit**

```bash
git add crates/context-store/src/vfs.rs crates/context-store/src/vfs_metadata.rs crates/context-store/migrations/002_create_vfs_metadata.sql
git commit -m "feat(vfs): add VirtualFileSystem trait and VfsOverlay implementation"
```

---

### Task 6: Create VFS tools

**Files:**
- Create: `crates/agent-runtime/src/tools/vfs_tools.rs`

- [ ] **Step 1: Define VFS tools**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct ReadFileArgs {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct WriteFileArgs {
    pub path: String,
    pub content: String, // base64 encoded or raw string
}

#[derive(Debug, Deserialize)]
pub struct ListArgs {
    pub dir: String,
}

#[derive(Debug, Deserialize)]
pub struct DeleteFileArgs {
    pub path: String,
}

pub fn register_vfs_tools(registry: &mut ToolRegistry, vfs: Arc<dyn VirtualFileSystem>) {
    registry.register("read_file", move |args: ReadFileArgs| {
        let vfs = vfs.clone();
        async move {
            let content = vfs.read(&args.path).await?;
            Ok(ToolResult {
                output: base64::encode(&content),
                metadata: None,
            })
        }
    });
    
    registry.register("write_file", move |args: WriteFileArgs| {
        let vfs = vfs.clone();
        async move {
            let content = args.content.as_bytes();
            let pointer = vfs.write(&args.path, content).await?;
            Ok(ToolResult {
                output: serde_json::to_string(&pointer).unwrap_or_default(),
                metadata: None,
            })
        }
    });
    
    registry.register("list_files", move |args: ListArgs| {
        let vfs = vfs.clone();
        async move {
            let files = vfs.list(&args.dir).await?;
            Ok(ToolResult {
                output: serde_json::to_string(&files).unwrap_or_default(),
                metadata: None,
            })
        }
    });
    
    registry.register("delete_file", move |args: DeleteFileArgs| {
        let vfs = vfs.clone();
        async move {
            vfs.delete(&args.path).await?;
            Ok(ToolResult {
                output: "deleted".to_string(),
                metadata: None,
            })
        }
    });
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/agent-runtime/src/tools/vfs_tools.rs
git commit -m "feat(vfs): add VFS tools (read_file, write_file, list_files, delete_file)"
```

---

## Phase 3: Context Manager (Week 3)

### Task 7: Create ContextManager

**Files:**
- Create: `crates/agent-runtime/src/context_mgr.rs`

- [ ] **Step 1: Define ContextManager**

```rust
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    pub covers_range: (usize, usize),
    pub content: String,
    pub created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub enum CompressionStrategy {
    KeepLastN(usize),
    SummarizeOlder { summarize_count: usize },
    ExtractiveCompression,
}

pub struct ContextManager {
    max_tokens: usize,
    warning_threshold: f64,
    compression_strategy: CompressionStrategy,
    full_history: Vec<Message>,
    compressed_context: Vec<Message>,
    summary_chain: Vec<Summary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

impl ContextManager {
    pub fn new(max_tokens: usize, strategy: CompressionStrategy) -> Self {
        Self {
            max_tokens,
            warning_threshold: 0.8,
            compression_strategy: strategy,
            full_history: Vec::new(),
            compressed_context: Vec::new(),
            summary_chain: Vec::new(),
        }
    }
    
    pub fn add_message(&mut self, msg: Message) {
        self.full_history.push(msg.clone());
        self.compressed_context.push(msg);
        
        // Check if compression needed
        let token_count = self.estimate_tokens();
        if token_count as f64 > self.max_tokens as f64 * self.warning_threshold {
            // Compression would be triggered
        }
    }
    
    pub fn get_compressed_context(&self) -> &[Message] {
        &self.compressed_context
    }
    
    pub fn get_full_history(&self) -> Vec<Message> {
        self.full_history.clone()
    }
    
    pub fn get_summary_chain(&self) -> &[Summary] {
        &self.summary_chain
    }
    
    fn estimate_tokens(&self) -> usize {
        // Rough estimation: 1 token ≈ 4 chars
        self.full_history
            .iter()
            .map(|m| m.content.len() / 4)
            .sum()
    }
    
    fn compress(&mut self) {
        match &self.compression_strategy {
            CompressionStrategy::KeepLastN(n) => {
                let to_keep = self.full_history.len().saturating_sub(*n);
                self.compressed_context = self.full_history[to_keep..].to_vec();
            }
            CompressionStrategy::SummarizeOlder { summarize_count } => {
                // Summarize older messages
                let to_summarize = &self.full_history[0..*summarize_count];
                let summary = format!("[Summary of {} messages]", to_summarize.len());
                
                self.compressed_context = vec![
                    Message {
                        role: "system".into(),
                        content: format!("Previous conversation summary: {}", summary),
                    }
                ];
                self.compressed_context
                    .extend(self.full_history[*summarize_count..].to_vec());
                
                self.summary_chain.push(Summary {
                    covers_range: (0, *summarize_count),
                    content: summary,
                    created_at: chrono::Utc::now(),
                });
            }
            CompressionStrategy::ExtractiveCompression => {
                // Keep messages with tool calls and final outputs
                self.compressed_context = self
                    .full_history
                    .iter()
                    .filter(|m| m.role == "tool" || m.content.len() > 100)
                    .cloned()
                    .collect();
            }
        }
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/agent-runtime/src/context_mgr.rs
git commit -m "feat(context): add ContextManager for token optimization"
```

---

### Task 8: Create context management tools

**Files:**
- Create: `crates/agent-runtime/src/tools/context_tools.rs`

- [ ] **Step 1: Define context tools**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct CompressArgs {
    pub strategy: Option<String>, // "summarize", "keep_last_n", "extractive"
    pub keep_recent: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct SummarizeRangeArgs {
    pub start_idx: usize,
    pub end_idx: usize,
}

pub fn register_context_tools(
    registry: &mut ToolRegistry,
    context_mgr: Arc<tokio::sync::Mutex<ContextManager>>,
) {
    registry.register("context_compress", move |args: CompressArgs| {
        let ctx = context_mgr.clone();
        async move {
            let mut mgr = ctx.lock().await;
            
            let strategy = args.strategy.as_deref().unwrap_or("summarize");
            let keep_recent = args.keep_recent.unwrap_or(5);
            
            match strategy {
                "summarize" => {
                    mgr.compression_strategy = CompressionStrategy::SummarizeOlder {
                        summarize_count: mgr.full_history.len() - keep_recent,
                    };
                }
                "keep_last_n" => {
                    mgr.compression_strategy = CompressionStrategy::KeepLastN(keep_recent);
                }
                "extractive" => {
                    mgr.compression_strategy = CompressionStrategy::ExtractiveCompression;
                }
                _ => {}
            }
            
            Ok(ToolResult {
                output: "Context compression strategy updated".to_string(),
                metadata: None,
            })
        }
    });
    
    registry.register("context_summarize_range", move |args: SummarizeRangeArgs| {
        let ctx = context_mgr.clone();
        async move {
            let mut mgr = ctx.lock().await;
            
            let range_size = args.end_idx - args.start_idx;
            let summary = format!(
                "Summary of messages {} to {}",
                args.start_idx, args.end_idx
            );
            
            mgr.summary_chain.push(Summary {
                covers_range: (args.start_idx, args.end_idx),
                content: summary.clone(),
                created_at: chrono::Utc::now(),
            });
            
            Ok(ToolResult {
                output: summary,
                metadata: None,
            })
        }
    });
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/agent-runtime/src/tools/context_tools.rs
git commit -m "feat(context): add context management tools (context_compress, context_summarize_range)"
```

---

## Phase 4: Integration & Testing (Week 4)

### Task 9: End-to-end integration

**Files:**
- Modify: `crates/agent-runtime/src/runtime.rs`
- Modify: `crates/executor/src/lib.rs`

- [ ] **Step 1: Integrate all three components in AgentRuntime**

```rust
pub struct AgentRuntime {
    // ... existing fields
    checkpointer: Arc<dyn Checkpointer>,
    context_manager: Arc<tokio::sync::Mutex<ContextManager>>,
    vfs: Arc<dyn VirtualFileSystem>,
}

impl AgentRuntime {
    pub async fn execute(&self, node: Node) -> Result<AgentOutput> {
        let mut context_mgr = ContextManager::new(
            node.max_tokens,
            self.config.compression_strategy.clone(),
        );
        
        // Build initial context
        context_mgr.add_message(Message {
            role: "system".into(),
            content: node.system_prompt,
        });
        context_mgr.add_message(Message {
            role: "user".into(),
            content: node.instruction,
        });
        
        // Tool call loop
        let mut tool_call_count = 0u32;
        loop {
            let response = self.llm
                .chat(context_mgr.get_compressed_context())
                .await?;
            
            context_mgr.add_message(response.clone());
            
            match response {
                Message { role: "assistant", content } if content.starts_with("tool:") => {
                    // Execute tool
                    let result = self.execute_tool(&content).await?;
                    context_mgr.add_message(result);
                    tool_call_count += 1;
                    
                    // Create checkpoint periodically
                    if tool_call_count % 5 == 0 {
                        let state = CheckpointState {
                            messages: context_mgr.get_full_history(),
                            tool_call_count,
                            intermediate_results: vec![],
                            custom_state: None,
                        };
                        self.checkpointer.save(node.run_id, node.id, state).await?;
                    }
                }
                _ => break,
            }
        }
        
        Ok(AgentOutput {
            output: context_mgr.get_compressed_context().last().cloned(),
            summary_chain: context_mgr.get_summary_chain().to_vec(),
        })
    }
}
```

- [ ] **Step 2: Update Executor crash recovery**

```rust
impl Executor {
    async fn recover_crashed_nodes(&self) {
        let running_nodes = self.db.get_running_nodes_older_than(Duration::hours(24)).await?;
        
        for node in running_nodes {
            let checkpoints = self.checkpointer.list_node_checkpoints(node.id).await?;
            
            if let Some(latest) = checkpoints.first() {
                let state = self.checkpointer.load(latest.id).await?;
                
                // Restore and resume
                self.agent_runtime.restore_from_checkpoint(node, state).await?;
            } else {
                // No checkpoint, reset and follow failure policy
                self.reset_node(node, Policy::Retry).await?;
            }
        }
    }
}
```

- [ ] **Step 3: Run full workspace cargo check**

```bash
cargo check --workspace
```

- [ ] **Step 4: Run tests**

```bash
cargo test --workspace
```

- [ ] **Step 5: Commit**

```bash
git commit -m "feat: integrate Checkpointer, VFS, and ContextManager into AgentRuntime"
```

---

### Task 10: Write integration tests

**Files:**
- Create: `crates/checkpointer/tests/integration.rs`
- Create: `crates/context-store/tests/vfs_integration.rs`
- Create: `crates/agent-runtime/tests/context_integration.rs`

- [ ] **Step 1: Write Checkpointer integration test**

```rust
#[tokio::test]
async fn test_checkpointer_save_and_load() {
    let pool = sqlx::PgPool::connect(&std::env::var("DATABASE_URL").unwrap())
        .await
        .unwrap();
    let redis = redis::Client::open(std::env::var("REDIS_URL").unwrap())
        .unwrap()
        .get_connection_manager()
        .await
        .unwrap();
    
    let checkpointer = HybridCheckpointer::new(pool, redis);
    
    let state = CheckpointState {
        messages: vec![
            Message {
                role: "user".into(),
                content: "Hello".into(),
            }
        ],
        tool_call_count: 5,
        intermediate_results: vec![],
        custom_state: None,
    };
    
    let run_id = Uuid::new_v4();
    let node_id = Uuid::new_v4();
    
    let id = checkpointer.save(run_id, node_id, state.clone()).await.unwrap();
    
    let loaded = checkpointer.load(id).await.unwrap();
    
    assert_eq!(loaded.messages.len(), state.messages.len());
    assert_eq!(loaded.tool_call_count, state.tool_call_count);
}
```

- [ ] **Step 2: Run integration tests**

```bash
cargo test --workspace --test integration
```

- [ ] **Step 3: Commit**

```bash
git add crates/*/tests/*.rs
git commit -m "test: add integration tests for Checkpointer, VFS, and ContextManager"
```

---

## Summary

| Phase | Tasks | Duration |
|-------|-------|----------|
| Phase 1: Checkpointer | Tasks 1-4 | Week 1 |
| Phase 2: VFS | Tasks 5-6 | Week 2 |
| Phase 3: Context Manager | Tasks 7-8 | Week 3 |
| Phase 4: Integration | Tasks 9-10 | Week 4 |

**Total Estimated Time:** 4 weeks

---

**Plan complete and saved to** `docs/superpowers/plans/2025-04-05-torque-core-enhancements-plan.md`

**Two execution options:**

**1. Subagent-Driven (recommended)** - Dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

**Which approach?**
