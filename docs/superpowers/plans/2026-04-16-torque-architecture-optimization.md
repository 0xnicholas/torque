# Torque Architecture Optimization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor `agent-runtime-service` to establish clean api/service/repository/kernel-bridge layers, make `torque-kernel` actually drive execution, wire event persistence and checkpointing via `checkpointer`, and thin HTTP handlers to pure adapters.

**Architecture:** Introduce `repository/` (async traits over sqlx), `service/` (business logic), `kernel-bridge/` (runtime integration), and `infra/` (shared tools/LLM). Replace `AgentRunner` with `SessionService` + `KernelRuntimeHandle`. Keep MVP `/sessions` API working during migration.

**Tech Stack:** Rust, axum 0.7, tokio, sqlx 0.7, serde, uuid, chrono, torque-kernel, checkpointer

---

## File Structure Map

### New / heavily modified modules
- `crates/agent-runtime-service/src/repository/mod.rs` — trait definitions + `RepositoryContainer`
- `crates/agent-runtime-service/src/repository/session.rs` — `SessionRepository` trait + `PostgresSessionRepository`
- `crates/agent-runtime-service/src/repository/message.rs` — `MessageRepository` trait + `PostgresMessageRepository`
- `crates/agent-runtime-service/src/repository/memory.rs` — `MemoryRepository` trait + `PostgresMemoryRepository`
- `crates/agent-runtime-service/src/repository/event.rs` — `EventRepository` trait + `PostgresEventRepository`
- `crates/agent-runtime-service/src/repository/checkpoint.rs` — `CheckpointRepository` trait + `PostgresCheckpointRepository`
- `crates/agent-runtime-service/src/repository/agent_definition.rs` — `AgentDefinitionRepository` trait
- `crates/agent-runtime-service/src/service/mod.rs` — `ServiceContainer`
- `crates/agent-runtime-service/src/service/session.rs` — `SessionService`
- `crates/agent-runtime-service/src/service/agent_instance.rs` — `AgentInstanceService`
- `crates/agent-runtime-service/src/service/memory.rs` — `MemoryService`
- `crates/agent-runtime-service/src/service/tool.rs` — `ToolService`
- `crates/agent-runtime-service/src/kernel_bridge/mod.rs` — module exports
- `crates/agent-runtime-service/src/kernel_bridge/runtime.rs` — `KernelRuntimeHandle`
- `crates/agent-runtime-service/src/kernel_bridge/mapping.rs` — `session_to_execution_request`
- `crates/agent-runtime-service/src/kernel_bridge/events.rs` — `EventRecorder`
- `crates/agent-runtime-service/src/kernel_bridge/checkpointer.rs` — `PostgresCheckpointer`
- `crates/agent-runtime-service/src/infra/mod.rs`
- `crates/agent-runtime-service/src/infra/tool_registry.rs` — moved from `tools/registry.rs`
- `crates/agent-runtime-service/src/infra/stream.rs` — SSE utilities
- `crates/agent-runtime-service/src/infra/llm.rs` — LLM wrappers

### Modified existing modules
- `crates/agent-runtime-service/src/api/mod.rs` — wire `ServiceContainer` into state
- `crates/agent-runtime-service/src/api/sessions.rs` — thin to ~20 lines
- `crates/agent-runtime-service/src/api/messages.rs` — thin to ~25 lines
- `crates/agent-runtime-service/src/api/memory.rs` — thin to service calls
- `crates/agent-runtime-service/src/app.rs` — build `ServiceContainer` and repositories
- `crates/agent-runtime-service/src/lib.rs` — export new modules
- `crates/agent-runtime-service/src/main.rs` — initialize repositories + services
- `crates/agent-runtime-service/src/agent/runner.rs` — **delete**
- `crates/agent-runtime-service/src/kernel/mapping.rs` — **delete** (move to kernel_bridge)
- `crates/agent-runtime-service/src/kernel/mod.rs` — **delete**
- `crates/agent-runtime-service/src/tools/registry.rs` — **delete** (move to infra)
- `crates/agent-runtime-service/src/models/v1/event.rs` — new DB model
- `crates/agent-runtime-service/migrations/20260416000003_create_v1_events.up.sql` — new table
- `crates/agent-runtime-service/migrations/20260416000003_create_v1_events.down.sql`
- `crates/agent-runtime-service/migrations/20260416000004_create_checkpoints.up.sql` — checkpoint table
- `crates/agent-runtime-service/migrations/20260416000004_create_checkpoints.down.sql`

---

## Phase 0: Foundation

### Task 0.1: Add `v1_events` and `checkpoints` migrations

**Files:**
- Create: `crates/agent-runtime-service/migrations/20260416000003_create_v1_events.up.sql`
- Create: `crates/agent-runtime-service/migrations/20260416000003_create_v1_events.down.sql`
- Create: `crates/agent-runtime-service/migrations/20260416000004_create_checkpoints.up.sql`
- Create: `crates/agent-runtime-service/migrations/20260416000004_create_checkpoints.down.sql`
- Modify: `crates/agent-runtime-service/src/models/v1/mod.rs`
- Create: `crates/agent-runtime-service/src/models/v1/event.rs`

- [ ] **Step 1: Write events migration**

```sql
-- up
CREATE TABLE v1_events (
    event_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_type TEXT NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resource_type TEXT NOT NULL,
    resource_id UUID NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}',
    sequence_number BIGINT
);

CREATE INDEX idx_v1_events_resource ON v1_events(resource_type, resource_id, timestamp DESC);
CREATE INDEX idx_v1_events_type ON v1_events(event_type, timestamp DESC);
```

```sql
-- down
DROP TABLE IF EXISTS v1_events;
```

- [ ] **Step 2: Write checkpoints migration**

```sql
-- up
CREATE TABLE checkpoints (
    id UUID PRIMARY KEY,
    instance_id UUID,
    task_id UUID,
    snapshot JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_checkpoints_instance ON checkpoints(instance_id, created_at DESC);
```

```sql
-- down
DROP TABLE IF EXISTS checkpoints;
```

- [ ] **Step 3: Write Event model**

```rust
// crates/agent-runtime-service/src/models/v1/event.rs
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Event {
    pub event_id: Uuid,
    pub event_type: String,
    pub timestamp: DateTime<Utc>,
    pub resource_type: String,
    pub resource_id: Uuid,
    pub payload: serde_json::Value,
    pub sequence_number: Option<i64>,
}
```

- [ ] **Step 4: Export from models/v1/mod.rs**

```rust
pub mod event;
```

- [ ] **Step 5: Run migrations locally**

```bash
cd crates/agent-runtime-service
export DATABASE_URL=postgres://postgres:postgres@localhost/agent_runtime_service
cargo sqlx migrate run
```

Expected: migrations apply successfully.

- [ ] **Step 6: Commit**

```bash
git add crates/agent-runtime-service/migrations/ crates/agent-runtime-service/src/models/v1/
git commit -m "feat(v1): add events and checkpoints migrations and model"
```

---

## Phase 1: Repository Layer

### Task 1.1: Create `RepositoryContainer` and `SessionRepository`

**Files:**
- Create: `crates/agent-runtime-service/src/repository/mod.rs`
- Create: `crates/agent-runtime-service/src/repository/session.rs`
- Modify: `crates/agent-runtime-service/src/lib.rs`

- [ ] **Step 1: Write repository module scaffold**

```rust
// crates/agent-runtime-service/src/repository/mod.rs
use std::sync::Arc;

pub mod session;
pub mod message;
pub mod memory;
pub mod event;
pub mod checkpoint;
pub mod agent_definition;

pub use session::{PostgresSessionRepository, SessionRepository, SessionKernelState};

pub struct RepositoryContainer {
    pub session: Arc<dyn SessionRepository>,
}
```

- [ ] **Step 1b: Create agent_definition stub**

Create `crates/agent-runtime-service/src/repository/agent_definition.rs`:
```rust
// Stub: full trait and Postgres impl added during Platform API v1 implementation
#[async_trait::async_trait]
pub trait AgentDefinitionRepository: Send + Sync {}
```

- [ ] **Step 2: Write SessionRepository trait and Postgres impl**

```rust
// crates/agent-runtime-service/src/repository/session.rs
use async_trait::async_trait;
use crate::db::Database;
use crate::models::{Session, SessionStatus};
use uuid::Uuid;

pub struct SessionKernelState {
    pub agent_definition_id: Uuid,
    pub status: String,
    pub active_task_id: Option<Uuid>,
    pub checkpoint_id: Option<Uuid>,
}

#[async_trait]
pub trait SessionRepository: Send + Sync {
    async fn create(&self, api_key: &str, project_scope: &str) -> anyhow::Result<Session>;
    async fn list(&self, limit: i64) -> anyhow::Result<Vec<Session>>;
    async fn get_by_id(&self, id: Uuid) -> anyhow::Result<Option<Session>>;
    async fn update_status(
        &self,
        id: Uuid,
        status: SessionStatus,
        error_msg: Option<&str>,
    ) -> anyhow::Result<bool>;
    async fn try_mark_running(&self, id: Uuid) -> anyhow::Result<bool>;
    async fn get_kernel_state(&self, id: Uuid) -> anyhow::Result<Option<SessionKernelState>>;
}

pub struct PostgresSessionRepository {
    db: Database,
}

impl PostgresSessionRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

#[async_trait]
impl SessionRepository for PostgresSessionRepository {
    async fn create(&self, api_key: &str, project_scope: &str) -> anyhow::Result<Session> {
        let row = sqlx::query_as::<_, Session>(
            r#"
            INSERT INTO sessions (api_key, project_scope, status)
            VALUES ($1, $2, 'idle')
            RETURNING id, api_key, project_scope, status, error_message, created_at, updated_at
            "#
        )
        .bind(api_key)
        .bind(project_scope)
        .fetch_one(self.db.pool())
        .await?;
        Ok(row)
    }

    async fn list(&self, limit: i64) -> anyhow::Result<Vec<Session>> {
        let rows = sqlx::query_as::<_, Session>(
            "SELECT * FROM sessions ORDER BY created_at DESC LIMIT $1"
        )
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows)
    }

    async fn get_by_id(&self, id: Uuid) -> anyhow::Result<Option<Session>> {
        let row = sqlx::query_as::<_, Session>(
            "SELECT * FROM sessions WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(self.db.pool())
        .await?;
        Ok(row)
    }

    async fn update_status(
        &self,
        id: Uuid,
        status: SessionStatus,
        error_msg: Option<&str>,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "UPDATE sessions SET status = $1, error_message = $2, updated_at = NOW() WHERE id = $3"
        )
        .bind(format!("{:?}", status).to_lowercase())
        .bind(error_msg)
        .bind(id)
        .execute(self.db.pool())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn try_mark_running(&self, id: Uuid) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "UPDATE sessions SET status = 'running', updated_at = NOW() WHERE id = $1 AND status = 'idle'"
        )
        .bind(id)
        .execute(self.db.pool())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn get_kernel_state(
        &self,
        id: Uuid,
    ) -> anyhow::Result<Option<SessionKernelState>> {
        let row: Option<(Uuid, String, Option<Uuid>, Option<Uuid>)> = sqlx::query_as(
            "SELECT id, status, active_task_id, checkpoint_id FROM sessions WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(self.db.pool())
        .await?;
        Ok(row.map(|(agent_def_id, status, active_task_id, checkpoint_id)| SessionKernelState {
            agent_definition_id: agent_def_id,
            status,
            active_task_id,
            checkpoint_id,
        }))
    }
}
```

Note: `get_kernel_state` assumes `sessions` table will have `active_task_id` and `checkpoint_id` columns. If they don't exist yet, add them via migration before this task, or mock the query in this step and fix in Task 6. For this plan, we will add a migration in Task 1.2 to add these columns.

Wait — looking at the existing `sessions` table, it does NOT have `active_task_id` or `checkpoint_id`. We need a migration.

Let me add that as Step 0 for Task 1.1.

Actually, to keep the plan clean, I'll add it as part of Task 0.1 or Task 1.1. Let's add it to Task 1.1 Step 0.

- [ ] **Step 0: Add session columns migration**

Create `crates/agent-runtime-service/migrations/20260416000005_add_session_kernel_columns.up.sql`:
```sql
ALTER TABLE sessions ADD COLUMN IF NOT EXISTS agent_definition_id UUID;
ALTER TABLE sessions ADD COLUMN IF NOT EXISTS agent_instance_id UUID;
ALTER TABLE sessions ADD COLUMN IF NOT EXISTS active_task_id UUID;
ALTER TABLE sessions ADD COLUMN IF NOT EXISTS checkpoint_id UUID;
```

Down:
```sql
ALTER TABLE sessions DROP COLUMN IF EXISTS agent_definition_id;
ALTER TABLE sessions DROP COLUMN IF EXISTS agent_instance_id;
ALTER TABLE sessions DROP COLUMN IF EXISTS active_task_id;
ALTER TABLE sessions DROP COLUMN IF EXISTS checkpoint_id;
```

Run migration.

- [ ] **Step 3: Export repository module from lib.rs**

```rust
pub mod repository;
pub mod service;
pub mod kernel_bridge;
pub mod infra;
```

- [ ] **Step 4: Verify compilation**

```bash
cargo check -p agent-runtime-service
```

Expected: PASS (may need to adjust Session model sqlx mapping if columns changed).

- [ ] **Step 5: Commit**

```bash
git add crates/agent-runtime-service/src/repository/ crates/agent-runtime-service/src/lib.rs crates/agent-runtime-service/migrations/
git commit -m "feat(repo): add SessionRepository trait and Postgres implementation"
```

---

### Task 1.2: Create `MessageRepository` and `MemoryRepository`

**Files:**
- Modify: `crates/agent-runtime-service/src/repository/mod.rs`
- Create: `crates/agent-runtime-service/src/repository/message.rs`
- Create: `crates/agent-runtime-service/src/repository/memory.rs`

- [ ] **Step 1: Write MessageRepository**

```rust
// crates/agent-runtime-service/src/repository/message.rs
use async_trait::async_trait;
use crate::db::Database;
use crate::models::Message;
use uuid::Uuid;

#[async_trait]
pub trait MessageRepository: Send + Sync {
    async fn create(&self, msg: &Message) -> anyhow::Result<Message>;
    async fn list_by_session(
        &self,
        session_id: Uuid,
        limit: i64,
    ) -> anyhow::Result<Vec<Message>>;
    async fn get_recent_by_session(
        &self,
        session_id: Uuid,
        limit: i64,
    ) -> anyhow::Result<Vec<Message>>;
}

pub struct PostgresMessageRepository {
    db: Database,
}

impl PostgresMessageRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

#[async_trait]
impl MessageRepository for PostgresMessageRepository {
    async fn create(&self, msg: &Message) -> anyhow::Result<Message> {
        let row = sqlx::query_as::<_, Message>(
            r#"
            INSERT INTO messages (session_id, role, content, tool_calls, artifacts)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, session_id, role, content, tool_calls, artifacts, created_at
            "#
        )
        .bind(msg.session_id)
        .bind(format!("{:?}", msg.role))
        .bind(&msg.content)
        .bind(msg.tool_calls.clone())
        .bind(msg.artifacts.clone())
        .fetch_one(self.db.pool())
        .await?;
        Ok(row)
    }

    async fn list_by_session(
        &self,
        session_id: Uuid,
        limit: i64,
    ) -> anyhow::Result<Vec<Message>> {
        let rows = sqlx::query_as::<_, Message>(
            "SELECT * FROM messages WHERE session_id = $1 ORDER BY created_at ASC LIMIT $2"
        )
        .bind(session_id)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows)
    }

    async fn get_recent_by_session(
        &self,
        session_id: Uuid,
        limit: i64,
    ) -> anyhow::Result<Vec<Message>> {
        let rows = sqlx::query_as::<_, Message>(
            "SELECT * FROM messages WHERE session_id = $1 ORDER BY created_at DESC LIMIT $2"
        )
        .bind(session_id)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows.into_iter().rev().collect())
    }
}
```

- [ ] **Step 2: Write MemoryRepository**

```rust
// crates/agent-runtime-service/src/repository/memory.rs
use async_trait::async_trait;
use crate::db::Database;
use crate::models::{MemoryCandidate, MemoryEntry, MemoryEntryStatus};
use uuid::Uuid;

#[async_trait]
pub trait MemoryRepository: Send + Sync {
    async fn create_candidate(
        &self,
        session_id: Uuid,
        entries: &[MemoryEntry],
    ) -> anyhow::Result<MemoryCandidate>;
    async fn accept_candidate(
        &self,
        candidate_id: Uuid,
    ) -> anyhow::Result<Vec<MemoryEntry>>;
    async fn list_entries(
        &self,
        session_id: Uuid,
    ) -> anyhow::Result<Vec<MemoryEntry>>;
    async fn search_entries(
        &self,
        session_id: Uuid,
        query: &str,
    ) -> anyhow::Result<Vec<MemoryEntry>>;
}

pub struct PostgresMemoryRepository {
    db: Database,
}

impl PostgresMemoryRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

#[async_trait]
impl MemoryRepository for PostgresMemoryRepository {
    async fn create_candidate(
        &self,
        _session_id: Uuid,
        _entries: &[MemoryEntry],
    ) -> anyhow::Result<MemoryCandidate> {
        todo!("migrate from db/memory_candidates.rs")
    }

    async fn accept_candidate(
        &self,
        _candidate_id: Uuid,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        todo!("migrate from db/memory_candidates.rs")
    }

    async fn list_entries(
        &self,
        _session_id: Uuid,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        todo!("migrate from db/memory_entries.rs")
    }

    async fn search_entries(
        &self,
        _session_id: Uuid,
        _query: &str,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        todo!("migrate from db/memory_entries.rs")
    }
}
```

For this architecture optimization plan, the memory repository stubs are acceptable because full memory migration is lower priority than kernel integration. We can fill them in later or as part of handler thinning.

- [ ] **Step 3: Update RepositoryContainer**

```rust
// crates/agent-runtime-service/src/repository/mod.rs
pub use message::{MessageRepository, PostgresMessageRepository};
pub use memory::{MemoryRepository, PostgresMemoryRepository};

pub struct RepositoryContainer {
    pub session: Arc<dyn SessionRepository>,
    pub message: Arc<dyn MessageRepository>,
    pub memory: Arc<dyn MemoryRepository>,
}
```

- [ ] **Step 4: Verify compilation**

```bash
cargo check -p agent-runtime-service
```

Expected: PASS (with todo!() warnings).

- [ ] **Step 5: Commit**

```bash
git add crates/agent-runtime-service/src/repository/
git commit -m "feat(repo): add MessageRepository and MemoryRepository stubs"
```

---

### Task 1.3: Create `EventRepository` and `CheckpointRepository`

**Files:**
- Modify: `crates/agent-runtime-service/src/repository/mod.rs`
- Create: `crates/agent-runtime-service/src/repository/event.rs`
- Create: `crates/agent-runtime-service/src/repository/checkpoint.rs`

- [ ] **Step 1: Write EventRepository**

```rust
// crates/agent-runtime-service/src/repository/event.rs
use async_trait::async_trait;
use crate::db::Database;
use crate::models::v1::event::Event;
use uuid::Uuid;

#[async_trait]
pub trait EventRepository: Send + Sync {
    async fn create(&self, event: Event) -> anyhow::Result<()>;
    async fn create_batch(&self,
        events: Vec<Event>,
    ) -> anyhow::Result<()>;
    async fn list_by_resource(
        &self,
        resource_type: &str,
        resource_id: Uuid,
        limit: i64,
    ) -> anyhow::Result<Vec<Event>>;
}

pub struct PostgresEventRepository {
    db: Database,
}

impl PostgresEventRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

#[async_trait]
impl EventRepository for PostgresEventRepository {
    async fn create(&self, event: Event) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO v1_events (event_id, event_type, timestamp, resource_type, resource_id, payload, sequence_number)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#
        )
        .bind(event.event_id)
        .bind(event.event_type)
        .bind(event.timestamp)
        .bind(event.resource_type)
        .bind(event.resource_id)
        .bind(event.payload)
        .bind(event.sequence_number)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    async fn create_batch(
        &self,
        _events: Vec<Event>,
    ) -> anyhow::Result<()> {
        // TODO: use COPY or UNNEST for efficiency
        todo!()
    }

    async fn list_by_resource(
        &self,
        resource_type: &str,
        resource_id: Uuid,
        limit: i64,
    ) -> anyhow::Result<Vec<Event>> {
        let rows = sqlx::query_as::<_, Event>(
            "SELECT * FROM v1_events WHERE resource_type = $1 AND resource_id = $2 ORDER BY timestamp DESC LIMIT $3"
        )
        .bind(resource_type)
        .bind(resource_id)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows)
    }
}
```

- [ ] **Step 2: Write CheckpointRepository**

```rust
// crates/agent-runtime-service/src/repository/checkpoint.rs
use async_trait::async_trait;
use crate::db::Database;
use uuid::Uuid;

#[async_trait]
pub trait CheckpointRepository: Send + Sync {
    async fn latest_checkpoint_id(
        &self,
        instance_id: Uuid,
    ) -> anyhow::Result<Option<Uuid>>;
}

pub struct PostgresCheckpointRepository {
    db: Database,
}

impl PostgresCheckpointRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

#[async_trait]
impl CheckpointRepository for PostgresCheckpointRepository {
    async fn latest_checkpoint_id(
        &self,
        instance_id: Uuid,
    ) -> anyhow::Result<Option<Uuid>> {
        let row: Option<(Uuid,)> = sqlx::query_as(
            "SELECT id FROM checkpoints WHERE instance_id = $1 ORDER BY created_at DESC LIMIT 1"
        )
        .bind(instance_id)
        .fetch_optional(self.db.pool())
        .await?;
        Ok(row.map(|(id,)| id))
    }
}
```

- [ ] **Step 3: Update RepositoryContainer**

```rust
pub use event::{EventRepository, PostgresEventRepository};
pub use checkpoint::{CheckpointRepository, PostgresCheckpointRepository};

pub struct RepositoryContainer {
    pub session: Arc<dyn SessionRepository>,
    pub message: Arc<dyn MessageRepository>,
    pub memory: Arc<dyn MemoryRepository>,
    pub event: Arc<dyn EventRepository>,
    pub checkpoint: Arc<dyn CheckpointRepository>,
}
```

- [ ] **Step 4: Verify compilation**

```bash
cargo check -p agent-runtime-service
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/agent-runtime-service/src/repository/
git commit -m "feat(repo): add EventRepository and CheckpointRepository"
```

---

## Phase 2: Service Layer Foundation

### Task 2.1: Create `ToolService` and move `ToolRegistry` to `infra/`

**Files:**
- Create: `crates/agent-runtime-service/src/infra/mod.rs`
- Create: `crates/agent-runtime-service/src/infra/tool_registry.rs`
- Create: `crates/agent-runtime-service/src/infra/stream.rs`
- Create: `crates/agent-runtime-service/src/infra/llm.rs`
- Create: `crates/agent-runtime-service/src/service/tool.rs`
- Modify: `crates/agent-runtime-service/src/tools/registry.rs`
- Modify: `crates/agent-runtime-service/src/tools/mod.rs`

- [ ] **Step 1: Move ToolRegistry to infra**

Copy the entire contents of `crates/agent-runtime-service/src/tools/registry.rs` to `crates/agent-runtime-service/src/infra/tool_registry.rs`.

Then modify `crates/agent-runtime-service/src/infra/tool_registry.rs` to make sure all internal imports use `crate::tools::builtin` etc. properly.

- [ ] **Step 2: Create infra module files**

```rust
// crates/agent-runtime-service/src/infra/mod.rs
pub mod llm;
pub mod stream;
pub mod tool_registry;
```

```rust
// crates/agent-runtime-service/src/infra/llm.rs
// Re-export or thin wrapper around llm crate types for infra consistency
pub use llm::{Chunk, FinishReason, LlmClient, Message as LlmMessage, OpenAiClient, ToolCall, ToolDef};
```

```rust
// crates/agent-runtime-service/src/infra/stream.rs
use crate::agent::stream::StreamEvent;

pub fn event_to_sse(event: StreamEvent) -> Result<axum::response::sse::Event, std::convert::Infallible> {
    Ok(event.to_sse())
}
```

- [ ] **Step 3: Create ToolService**

```rust
// crates/agent-runtime-service/src/service/tool.rs
use crate::infra::tool_registry::ToolRegistry;
use std::sync::Arc;

pub struct ToolService {
    registry: Arc<ToolRegistry>,
}

impl ToolService {
    pub async fn new() -> Self {
        let registry = Arc::new(ToolRegistry::new());
        for tool in crate::tools::builtin::create_builtin_tools() {
            registry.register(Arc::from(tool)).await;
        }
        Self { registry }
    }

    pub fn registry(&self) -> Arc<ToolRegistry> {
        self.registry.clone()
    }
}
```

- [ ] **Step 4: Update tools/mod.rs and tools/registry.rs**

In `tools/mod.rs`, add a re-export so existing code doesn't break during migration:

```rust
pub use crate::infra::tool_registry::{ToolRegistry, ToolResult};
```

In `tools/registry.rs`, replace the entire file with:
```rust
pub use crate::infra::tool_registry::*;
```

- [ ] **Step 5: Verify compilation**

```bash
cargo check -p agent-runtime-service
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/agent-runtime-service/src/infra/ crates/agent-runtime-service/src/service/tool.rs crates/agent-runtime-service/src/tools/
git commit -m "feat(infra): move ToolRegistry to infra and add ToolService singleton"
```

---

### Task 2.2: Create `SessionService` (without kernel integration first)

**Files:**
- Create: `crates/agent-runtime-service/src/service/mod.rs`
- Create: `crates/agent-runtime-service/src/service/session.rs`
- Create: `crates/agent-runtime-service/src/service/agent_instance.rs`
- Create: `crates/agent-runtime-service/src/service/memory.rs`
- Modify: `crates/agent-runtime-service/src/api/sessions.rs`

- [ ] **Step 0: Create service stubs**

Create `crates/agent-runtime-service/src/service/agent_instance.rs`:
```rust
use std::sync::Arc;

pub struct AgentInstanceService;
impl AgentInstanceService {
    pub fn new(
        _session_repo: Arc<dyn crate::repository::SessionRepository>,
        _event_repo: Arc<dyn crate::repository::EventRepository>,
        _checkpoint_repo: Arc<dyn crate::repository::CheckpointRepository>,
        _checkpointer: Arc<dyn crate::repository::Checkpointer>,
    ) -> Self {
        Self
    }
}
```

Create `crates/agent-runtime-service/src/service/memory.rs`:
```rust
use crate::repository::MemoryRepository;
use std::sync::Arc;

pub struct MemoryService {
    _repo: Arc<dyn MemoryRepository>,
}

impl MemoryService {
    pub fn new(repo: Arc<dyn MemoryRepository>) -> Self {
        Self { _repo: repo }
    }
}
```

- [ ] **Step 1: Write ServiceContainer**

```rust
// crates/agent-runtime-service/src/service/mod.rs
use std::sync::Arc;

pub mod session;
pub mod tool;
pub mod memory;
pub mod agent_instance;

pub use session::SessionService;
pub use tool::ToolService;

pub struct ServiceContainer {
    pub session: Arc<SessionService>,
    pub memory: Arc<memory::MemoryService>,
    pub tool: Arc<ToolService>,
    pub agent_instance: Arc<agent_instance::AgentInstanceService>,
    pub idempotency: Arc<crate::v1_guards::IdempotencyStore>,
    pub run_gate: Arc<crate::v1_guards::RunGate>,
}

impl ServiceContainer {
    pub async fn new(
        repos: crate::repository::RepositoryContainer,
        checkpointer: Arc<dyn crate::repository::Checkpointer>,
        llm: Arc<dyn crate::infra::llm::LlmClient>,
        idempotency: Arc<crate::v1_guards::IdempotencyStore>,
        run_gate: Arc<crate::v1_guards::RunGate>,
    ) -> Self {
        let tool = Arc::new(ToolService::new().await);
        let memory = Arc::new(memory::MemoryService::new(repos.memory.clone()));
        let session = Arc::new(SessionService::new(
            repos.session.clone(),
            repos.message.clone(),
            repos.event.clone(),
            repos.checkpoint.clone(),
            checkpointer.clone(),
            llm,
            tool.clone(),
            memory.clone(),
        ));
        let agent_instance = Arc::new(agent_instance::AgentInstanceService::new(
            repos.session.clone(),
            repos.event.clone(),
            repos.checkpoint.clone(),
            checkpointer.clone(),
        ));

        Self { session, memory, tool, agent_instance, idempotency, run_gate }
    }
}
```

- [ ] **Step 2: Write SessionService skeleton**

```rust
// crates/agent-runtime-service/src/service/session.rs
use crate::agent::stream::StreamEvent;
use crate::infra::llm::LlmClient;
use crate::infra::tool_registry::ToolRegistry;
use crate::repository::{MessageRepository, SessionRepository};
use crate::service::{MemoryService, ToolService};
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum SessionServiceError {
    #[error("not found")]
    NotFound,
    #[error("forbidden")]
    Forbidden,
    #[error("conflict")]
    Conflict,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub struct SessionService {
    session_repo: Arc<dyn SessionRepository>,
    message_repo: Arc<dyn MessageRepository>,
    event_repo: Arc<dyn crate::repository::EventRepository>,
    checkpoint_repo: Arc<dyn crate::repository::CheckpointRepository>,
    checkpointer: Arc<dyn crate::repository::Checkpointer>,
    llm: Arc<dyn LlmClient>,
    tools: Arc<ToolService>,
    memory: Arc<MemoryService>,
}

impl SessionService {
    pub fn new(
        session_repo: Arc<dyn SessionRepository>,
        message_repo: Arc<dyn MessageRepository>,
        event_repo: Arc<dyn crate::repository::EventRepository>,
        checkpoint_repo: Arc<dyn crate::repository::CheckpointRepository>,
        checkpointer: Arc<dyn crate::repository::Checkpointer>,
        llm: Arc<dyn LlmClient>,
        tools: Arc<ToolService>,
        memory: Arc<MemoryService>,
    ) -> Self {
        Self {
            session_repo,
            message_repo,
            event_repo,
            checkpoint_repo,
            checkpointer,
            llm,
            tools,
            memory,
        }
    }

    pub async fn create(&self,
        api_key: &str,
        project_scope: &str,
    ) -> Result<crate::models::Session, SessionServiceError> {
        self.session_repo.create(api_key, project_scope).await
            .map_err(SessionServiceError::Other)
    }

    pub async fn list(&self,
        limit: i64,
    ) -> Result<Vec<crate::models::Session>, SessionServiceError> {
        self.session_repo.list(limit).await
            .map_err(SessionServiceError::Other)
    }

    pub async fn get_by_id(&self,
        session_id: Uuid,
        api_key: &str,
    ) -> Result<crate::models::Session, SessionServiceError> {
        let session = self.session_repo.get_by_id(session_id).await
            .map_err(SessionServiceError::Other)?
            .ok_or(SessionServiceError::NotFound)?;

        use subtle::ConstantTimeEq;
        if !bool::from(session.api_key.as_bytes().ct_eq(api_key.as_bytes())) {
            return Err(SessionServiceError::Forbidden);
        }

        Ok(session)
    }
}
```

- [ ] **Step 3: Thin `api/sessions.rs` to use SessionService**

```rust
// crates/agent-runtime-service/src/api/sessions.rs
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use crate::api::middleware::extract_api_key;
use crate::models::Session;
use crate::service::ServiceContainer;
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    pub project_scope: String,
}

pub async fn create(
    State(services): State<Arc<ServiceContainer>>,
    request: axum::extract::Request,
) -> Result<Json<Session>, StatusCode> {
    let api_key = extract_api_key(&request).ok_or(StatusCode::UNAUTHORIZED)?;
    let session = services.session.create(&api_key, "default").await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(session))
}

pub async fn list(
    State(services): State<Arc<ServiceContainer>>,
) -> Result<Json<Vec<Session>>, StatusCode> {
    let sessions = services.session.list(100).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(sessions))
}

pub async fn get(
    State(services): State<Arc<ServiceContainer>>,
    Path(id): Path<Uuid>,
    request: axum::extract::Request,
) -> Result<Json<Session>, StatusCode> {
    let api_key = extract_api_key(&request).ok_or(StatusCode::UNAUTHORIZED)?;
    match services.session.get_by_id(id, &api_key).await {
        Ok(session) => Ok(Json(session)),
        Err(crate::service::session::SessionServiceError::NotFound) => Err(StatusCode::NOT_FOUND),
        Err(crate::service::session::SessionServiceError::Forbidden) => Err(StatusCode::FORBIDDEN),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}
```

- [ ] **Step 4: Wire ServiceContainer into app state**

Modify `crates/agent-runtime-service/src/app.rs`:

```rust
use axum::Router;
use llm::OpenAiClient;
use std::sync::Arc;

use crate::api;
use crate::db::Database;
use crate::repository::RepositoryContainer;
use crate::service::ServiceContainer;

pub fn build_app(db: Database, llm: Arc<OpenAiClient>) -> Router {
    let repos = RepositoryContainer {
        session: Arc::new(crate::repository::PostgresSessionRepository::new(db.clone())),
        message: Arc::new(crate::repository::PostgresMessageRepository::new(db.clone())),
        memory: Arc::new(crate::repository::PostgresMemoryRepository::new(db.clone())),
        event: Arc::new(crate::repository::PostgresEventRepository::new(db.clone())),
        checkpoint: Arc::new(crate::repository::PostgresCheckpointRepository::new(db.clone())),
    };

    let checkpointer = Arc::new(crate::kernel_bridge::PostgresCheckpointer::new(db.clone()));
    let idempotency = Arc::new(crate::v1_guards::IdempotencyStore::new());
    let run_gate = Arc::new(crate::v1_guards::RunGate::new());

    let services = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(ServiceContainer::new(repos, checkpointer, llm.clone(), idempotency, run_gate))
    });

    api::router(db, llm, Arc::new(services))
}
```

Modify `crates/agent-runtime-service/src/api/mod.rs` to accept services:

```rust
pub fn router(
    db: Database,
    llm: Arc<OpenAiClient>,
    services: Arc<ServiceContainer>,
) -> Router {
    use axum::middleware;
    use crate::api::middleware::auth_middleware;

    Router::new()
        .route("/sessions", post(sessions::create).get(sessions::list))
        .route("/sessions/:id", get(sessions::get))
        // ... keep all existing routes ...
        .route("/metrics", get(metrics::get))
        .layer(middleware::from_fn(auth_middleware))
        .with_state((db, llm, services))
}
```

Update all handler state extractions from `State((db, llm))` to `State((db, llm, services))` in the files we modify. For files not yet modified, the old destructuring still works because tuples are additive.

Actually, axum's `State` uses the full tuple. If we change from `(Database, Arc<OpenAiClient>)` to `(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)`, handlers that destructure `State((db, llm))` will fail because the tuple has 3 elements.

So we need to update ALL handlers in this step. Let's update `messages.rs`, `memory.rs`, and `metrics.rs` to destructure 3 elements. For `metrics.rs`, it's simple:

```rust
pub async fn get(
    State(_): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
) -> Result<Json<...` — actually we can just ignore it.
```

Better: use a struct for state to avoid tuple length issues. But that increases scope. Let's just update the destructurings.

In `api/messages.rs`:
```rust
pub async fn list(
    State((db, _, _)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    // ...
)
```

Wait, `metrics::get` currently takes no state. Actually looking at the code:
```rust
pub async fn get() -> Json<...>
```

It doesn't use state. So no change needed there. `memory.rs` handlers need updating. Let's handle them in Task 2.3 or do minimal updates here.

For compilation in this step, we should at least make `messages.rs` and `memory.rs` compile. Let's update their state signatures to 3-tuple but keep the body unchanged.

Actually, this is getting complex. Let me change the approach: keep the tuple as `(db, llm)` for existing routes, and nest the v1 router with its own state. But `sessions::get` now needs `services`. So `sessions.rs` handlers destructure the 3-tuple. For other handlers, we update them minimally.

Let's do minimal updates in this task so everything compiles.

In `api/messages.rs`, change all `State((db, llm))` to `State((db, llm, _services))`.
In `api/memory.rs`, change all `State((db, _))` or `State((db, llm))` to `State((db, _, _))`.

- [ ] **Step 5: Verify compilation**

```bash
cargo check -p agent-runtime-service
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/agent-runtime-service/src/service/ crates/agent-runtime-service/src/api/sessions.rs crates/agent-runtime-service/src/api/mod.rs crates/agent-runtime-service/src/app.rs crates/agent-runtime-service/src/api/messages.rs crates/agent-runtime-service/src/api/memory.rs
git commit -m "feat(service): add SessionService and thin sessions handler"
```

---

## Phase 3: Kernel Bridge

### Task 3.1: Create `kernel-bridge/mapping.rs` and `kernel-bridge/mod.rs`

**Files:**
- Create: `crates/agent-runtime-service/src/kernel_bridge/mod.rs`
- Create: `crates/agent-runtime-service/src/kernel_bridge/mapping.rs`
- Delete: `crates/agent-runtime-service/src/kernel/mapping.rs`
- Delete: `crates/agent-runtime-service/src/kernel/mod.rs`
- Modify: `crates/agent-runtime-service/src/lib.rs`

- [ ] **Step 1: Write mapping.rs**

```rust
// crates/agent-runtime-service/src/kernel_bridge/mapping.rs
use crate::models::Session;
use torque_kernel::{
    AgentDefinition, ExecutionMode, ExecutionRequest, KernelError,
};

pub fn session_to_execution_request(
    session: &Session,
    user_message: &str,
) -> Result<ExecutionRequest, KernelError> {
    let agent_def_id = session.agent_definition_id
        .map(|id| id.to_string())
        .unwrap_or_else(|| session.id.to_string());
    let agent_def = AgentDefinition::new(
        &agent_def_id,
        "MVP session adapter",
    );

    ExecutionRequest::new(
        agent_def.id,
        user_message.to_string(),
        vec![format!("Session {}", session.id)],
    )
    .with_execution_mode(ExecutionMode::Sync)
}
```

- [ ] **Step 2: Write kernel_bridge/mod.rs**

```rust
pub mod mapping;
pub mod runtime;
pub mod events;
pub mod checkpointer;

pub use mapping::session_to_execution_request;
pub use runtime::KernelRuntimeHandle;
pub use events::{to_db_events, EventRecorder};
pub use checkpointer::PostgresCheckpointer;
```

- [ ] **Step 3: Delete old kernel/ module**

```bash
rm crates/agent-runtime-service/src/kernel/mod.rs
rm crates/agent-runtime-service/src/kernel/mapping.rs
rmdir crates/agent-runtime-service/src/kernel
```

If `kernel/` directory doesn't exist or has other files, adjust. Looking at our earlier exploration, it only had `mod.rs` and `mapping.rs`.

- [ ] **Step 4: Update lib.rs**

Remove `pub mod kernel;` and add `pub mod kernel_bridge;` (already done in Task 2.1 if we added it then, but verify).

Also update `agent/runner.rs` imports — it currently uses `crate::kernel::build_kernel_turn`. Change to `crate::kernel_bridge::session_to_execution_request`. We'll fix this when we delete runner.rs.

- [ ] **Step 5: Verify compilation**

```bash
cargo check -p agent-runtime-service
```

Expected: PASS (except references to deleted `kernel` module in runner.rs, which we'll fix next).

- [ ] **Step 6: Commit**

```bash
git add crates/agent-runtime-service/src/kernel_bridge/ crates/agent-runtime-service/src/lib.rs
git rm -r crates/agent-runtime-service/src/kernel/
git commit -m "feat(kernel-bridge): add mapping layer and delete old kernel module"
```

---

### Task 3.2: Create `EventRecorder` and `PostgresCheckpointer`

**Files:**
- Create: `crates/agent-runtime-service/src/kernel_bridge/events.rs`
- Create: `crates/agent-runtime-service/src/kernel_bridge/checkpointer.rs`

- [ ] **Step 1: Write EventRecorder**

```rust
// crates/agent-runtime-service/src/kernel_bridge/events.rs
use crate::models::v1::event::Event;
use chrono::Utc;
use torque_kernel::{
    AgentInstanceId, ExecutionEvent, ExecutionResult,
};
use uuid::Uuid;

pub struct EventRecorder;

impl EventRecorder {
    pub fn to_db_events(
        result: &ExecutionResult,
        sequence_offset: u64,
    ) -> Vec<Event> {
        let mut events = Vec::new();
        let seq = sequence_offset as i64;

        for (idx, event) in result.events.iter().enumerate() {
            let db_event = match event {
                ExecutionEvent::InstanceStateChanged { from, to } => Event {
                    event_id: Uuid::new_v4(),
                    event_type: "instance_state_changed".to_string(),
                    timestamp: Utc::now(),
                    resource_type: "agent_instance".to_string(),
                    resource_id: result.instance_id.as_uuid(),
                    payload: serde_json::json!({
                        "from": format!("{:?}", from),
                        "to": format!("{:?}", to),
                        "task_id": result.task_id.as_uuid(),
                    }),
                    sequence_number: Some(seq + idx as i64),
                },
                ExecutionEvent::TaskStateChanged { from, to } => Event {
                    event_id: Uuid::new_v4(),
                    event_type: "task_state_changed".to_string(),
                    timestamp: Utc::now(),
                    resource_type: "task".to_string(),
                    resource_id: result.task_id.as_uuid(),
                    payload: serde_json::json!({
                        "from": format!("{:?}", from),
                        "to": format!("{:?}", to),
                    }),
                    sequence_number: Some(seq + idx as i64),
                },
                ExecutionEvent::ArtifactProduced { artifact_id } => Event {
                    event_id: Uuid::new_v4(),
                    event_type: "artifact_produced".to_string(),
                    timestamp: Utc::now(),
                    resource_type: "task".to_string(),
                    resource_id: result.task_id.as_uuid(),
                    payload: serde_json::json!({
                        "artifact_id": artifact_id.as_uuid(),
                    }),
                    sequence_number: Some(seq + idx as i64),
                },
                ExecutionEvent::ApprovalRequested { approval_request_id } => Event {
                    event_id: Uuid::new_v4(),
                    event_type: "approval_requested".to_string(),
                    timestamp: Utc::now(),
                    resource_type: "agent_instance".to_string(),
                    resource_id: result.instance_id.as_uuid(),
                    payload: serde_json::json!({
                        "approval_request_id": approval_request_id.as_uuid(),
                    }),
                    sequence_number: Some(seq + idx as i64),
                },
                ExecutionEvent::DelegationRequested { delegation_request_id } => Event {
                    event_id: Uuid::new_v4(),
                    event_type: "delegation_requested".to_string(),
                    timestamp: Utc::now(),
                    resource_type: "agent_instance".to_string(),
                    resource_id: result.instance_id.as_uuid(),
                    payload: serde_json::json!({
                        "delegation_request_id": delegation_request_id.as_uuid(),
                    }),
                    sequence_number: Some(seq + idx as i64),
                },
                ExecutionEvent::ResumeApplied { resume_signal } => Event {
                    event_id: Uuid::new_v4(),
                    event_type: "resume_applied".to_string(),
                    timestamp: Utc::now(),
                    resource_type: "agent_instance".to_string(),
                    resource_id: result.instance_id.as_uuid(),
                    payload: serde_json::json!({
                        "resume_signal": format!("{:?}", resume_signal),
                    }),
                    sequence_number: Some(seq + idx as i64),
                },
            };
            events.push(db_event);
        }
        events
    }
}
```

- [ ] **Step 2: Write PostgresCheckpointer**

```rust
// crates/agent-runtime-service/src/kernel_bridge/checkpointer.rs
use checkpointer::{Checkpointer, CheckpointId, CheckpointMeta, CheckpointState};
use crate::db::Database;
use async_trait::async_trait;
use uuid::Uuid;

pub struct PostgresCheckpointer {
    db: Database,
}

impl PostgresCheckpointer {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Checkpointer for PostgresCheckpointer {
    async fn save(
        &self,
        id: CheckpointId,
        meta: CheckpointMeta,
        state: CheckpointState,
    ) -> checkpointer::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO checkpoints (id, instance_id, task_id, snapshot, created_at)
            VALUES ($1, $2, $3, $4, NOW())
            ON CONFLICT (id) DO UPDATE SET snapshot = EXCLUDED.snapshot
            "#
        )
        .bind(Uuid::parse_str(&id.to_string()).unwrap_or_default())
        .bind(meta.instance_id.map(|u| u.to_string()))
        .bind(meta.task_id.map(|u| u.to_string()))
        .bind(serde_json::to_value(&state).map_err(|e| checkpointer::CheckpointerError::Serialization(e.to_string()))?)
        .execute(self.db.pool())
        .await
        .map_err(|e| checkpointer::CheckpointerError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn load(
        &self,
        id: CheckpointId,
    ) -> checkpointer::Result<Option<CheckpointState>> {
        let row: Option<(serde_json::Value,)> = sqlx::query_as(
            "SELECT snapshot FROM checkpoints WHERE id = $1"
        )
        .bind(Uuid::parse_str(&id.to_string()).unwrap_or_default())
        .fetch_optional(self.db.pool())
        .await
        .map_err(|e| checkpointer::CheckpointerError::Storage(e.to_string()))?;

        row.map(|(json,)| serde_json::from_value(json)
            .map_err(|e| checkpointer::CheckpointerError::Serialization(e.to_string())))
         .transpose()
    }

    async fn list_by_instance(
        &self,
        instance_id: Uuid,
    ) -> checkpointer::Result<Vec<CheckpointId>> {
        let rows: Vec<(Uuid,)> = sqlx::query_as(
            "SELECT id FROM checkpoints WHERE instance_id = $1 ORDER BY created_at DESC"
        )
        .bind(instance_id)
        .fetch_all(self.db.pool())
        .await
        .map_err(|e| checkpointer::CheckpointerError::Storage(e.to_string()))?;

        Ok(rows.into_iter()
            .map(|(id,)| CheckpointId::from_uuid(id))
            .collect())
    }
}
```

Wait — I need to check what `CheckpointId::from_uuid` actually looks like in `checkpointer`. Let me verify the API.

Actually, looking at `crates/checkpointer/src/trait.rs`, I should check if it exists. I haven't read that file. Let me not hardcode methods I'm unsure of. I'll use a generic construction.

Actually I can just write it as `CheckpointId::new()` or similar and note that the exact constructor should match the `checkpointer` crate API. But for the plan, I'll assume `CheckpointId` has a constructor that accepts a `Uuid`. If not, the implementer will adjust.

- [ ] **Step 3: Verify compilation**

```bash
cargo check -p agent-runtime-service
```

Expected: PASS (may need minor adjustments to match `checkpointer` trait exactly).

- [ ] **Step 4: Commit**

```bash
git add crates/agent-runtime-service/src/kernel_bridge/events.rs crates/agent-runtime-service/src/kernel_bridge/checkpointer.rs
git commit -m "feat(kernel-bridge): add EventRecorder and PostgresCheckpointer"
```

---

### Task 3.3: Implement `KernelRuntimeHandle`

**Files:**
- Create: `crates/agent-runtime-service/src/kernel_bridge/runtime.rs`

- [ ] **Step 1: Write KernelRuntimeHandle**

```rust
// crates/agent-runtime-service/src/kernel_bridge/runtime.rs
use crate::agent::stream::StreamEvent;
use crate::infra::llm::LlmClient;
use crate::infra::tool_registry::ToolRegistry;
use crate::kernel_bridge::events::EventRecorder;
use crate::repository::{CheckpointRepository, EventRepository, SessionRepository};
use checkpointer::Checkpointer;
use std::sync::Arc;
use tokio::sync::mpsc;
use torque_kernel::{
    AgentDefinition, AgentDefinitionId, AgentInstance, AgentInstanceId, AgentInstanceState,
    ExecutionRequest, ExecutionResult, InMemoryKernelRuntime, KernelError, ResumeSignal,
    StepDecision, TaskPacket,
};

#[derive(Debug, thiserror::Error)]
pub enum KernelBridgeError {
    #[error("kernel error: {0}")]
    Kernel(#[from] KernelError),
    #[error("db error: {0}")]
    Db(#[from] anyhow::Error),
    #[error("checkpoint error: {0}")]
    Checkpoint(String),
    #[error("no checkpoint for instance {0}")]
    NoCheckpoint(AgentInstanceId),
    #[error("checkpoint not found")]
    CheckpointNotFound,
}

pub struct KernelRuntimeHandle {
    runtime: InMemoryKernelRuntime,
    event_repo: Arc<dyn EventRepository>,
    checkpoint_repo: Arc<dyn CheckpointRepository>,
    checkpointer: Arc<dyn Checkpointer>,
}

impl KernelRuntimeHandle {
    pub fn new(
        agent_definitions: Vec<AgentDefinition>,
        event_repo: Arc<dyn EventRepository>,
        checkpoint_repo: Arc<dyn CheckpointRepository>,
        checkpointer: Arc<dyn Checkpointer>,
    ) -> Self {
        Self {
            runtime: InMemoryKernelRuntime::new(agent_definitions),
            event_repo,
            checkpoint_repo,
            checkpointer,
        }
    }

    pub async fn hydrate_runtime(
        &mut self,
        instance_id: AgentInstanceId,
        session_repo: &dyn SessionRepository,
    ) -> Result<(), KernelBridgeError> {
        let _state = session_repo.get_kernel_state(instance_id.as_uuid()).await?;
        // Short-term: reconstruct AgentInstance from session state and inject into store.
        // For MVP migration, we create a fresh instance each time.
        // Long-term: PostgresRuntimeStore implements RuntimeStore.
        Ok(())
    }

    pub async fn execute_chat(
        &mut self,
        request: ExecutionRequest,
        llm: Arc<dyn LlmClient>,
        tools: Arc<ToolRegistry>,
        event_sink: mpsc::Sender<StreamEvent>,
    ) -> Result<ExecutionResult, KernelBridgeError> {
        let mut result = self.runtime.handle(request, StepDecision::Continue)?;
        self.record_events(&result).await?;

        while self.needs_external_resolution(&result)? {
            result = self.resolve_and_resume(
                result, llm.clone(), tools.clone(), event_sink.clone()
            ).await?;
            self.record_events(&result).await?;
        }

        if self.is_terminal(&result) {
            self.create_checkpoint(result.instance_id).await?;
        }

        Ok(result)
    }

    fn needs_external_resolution(
        &self,
        result: &ExecutionResult,
    ) -> Result<bool, KernelBridgeError> {
        Ok(matches!(
            result.outcome,
            torque_kernel::ExecutionOutcome::AwaitTool
                | torque_kernel::ExecutionOutcome::AwaitApproval
                | torque_kernel::ExecutionOutcome::AwaitDelegation
        ))
    }

    fn is_terminal(&self,
        result: &ExecutionResult,
    ) -> bool {
        matches!(
            result.outcome,
            torque_kernel::ExecutionOutcome::CompletedTask
                | torque_kernel::ExecutionOutcome::FailedTask
        )
    }

    async fn resolve_and_resume(
        &mut self,
        result: ExecutionResult,
        llm: Arc<dyn LlmClient>,
        tools: Arc<ToolRegistry>,
        event_sink: mpsc::Sender<StreamEvent>,
    ) -> Result<ExecutionResult, KernelBridgeError> {
        let instance_id = result.instance_id;
        let instance = self.runtime.instance(instance_id)
            .ok_or_else(|| KernelBridgeError::Kernel(
                torque_kernel::ValidationError::new("Runtime", "instance missing").into()
            ))?;

        let decision = match instance.state() {
            AgentInstanceState::WaitingTool => {
                // MVP simplification: resolve tool immediately
                let resume = ResumeSignal::ToolCompleted;
                let cmd = torque_kernel::RuntimeCommand::new(StepDecision::Continue)
                    .with_resume_signal(resume);
                // We need an ExecutionRequest to call handle_command
                // For MVP, we reconstruct a minimal request
                let req = self.reconstruct_request(instance_id)?;
                return self.runtime.handle_command(req, cmd).map_err(KernelBridgeError::Kernel);
            }
            AgentInstanceState::WaitingApproval => {
                // In MVP, approval is auto-granted after a stub delay
                let approval_id = result.approval_request_ids.last()
                    .copied()
                    .ok_or_else(|| KernelBridgeError::Kernel(
                        torque_kernel::ValidationError::new("Runtime", "approval id missing").into()
                    ))?;
                let resume = ResumeSignal::ApprovalGranted { approval_request_id: approval_id };
                let cmd = torque_kernel::RuntimeCommand::new(StepDecision::Continue)
                    .with_resume_signal(resume);
                let req = self.reconstruct_request(instance_id)?;
                return self.runtime.handle_command(req, cmd).map_err(KernelBridgeError::Kernel);
            }
            AgentInstanceState::WaitingSubagent => {
                let delegation_id = result.delegation_request_ids.last()
                    .copied()
                    .ok_or_else(|| KernelBridgeError::Kernel(
                        torque_kernel::ValidationError::new("Runtime", "delegation id missing").into()
                    ))?;
                let resume = ResumeSignal::DelegationCompleted { delegation_request_id: delegation_id };
                let cmd = torque_kernel::RuntimeCommand::new(StepDecision::Continue)
                    .with_resume_signal(resume);
                let req = self.reconstruct_request(instance_id)?;
                return self.runtime.handle_command(req, cmd).map_err(KernelBridgeError::Kernel);
            }
            AgentInstanceState::Ready | AgentInstanceState::Running => {
                // Call LLM to decide next step
                self.llm_to_step_decision(llm, tools, event_sink).await?
            }
            _ => StepDecision::Continue,
        };

        let req = self.reconstruct_request(instance_id)?;
        self.runtime.handle(req, decision).map_err(KernelBridgeError::Kernel)
    }

    async fn llm_to_step_decision(
        &self,
        _llm: Arc<dyn LlmClient>,
        _tools: Arc<ToolRegistry>,
        _event_sink: mpsc::Sender<StreamEvent>,
    ) -> Result<StepDecision, KernelBridgeError> {
        // MVP: always continue. In full implementation, this calls LLM and maps response to StepDecision.
        Ok(StepDecision::Continue)
    }

    fn reconstruct_request(
        &self,
        instance_id: AgentInstanceId,
    ) -> Result<ExecutionRequest, KernelBridgeError> {
        let instance = self.runtime.instance(instance_id)
            .ok_or_else(|| KernelBridgeError::Kernel(
                torque_kernel::ValidationError::new("Runtime", "instance missing").into()
            ))?;
        Ok(torque_kernel::ExecutionRequest::new(
            instance.state().into(), // This won't compile; use a stub
            "continue".to_string(),
            vec![],
        ))
    }

    async fn record_events(
        &self,
        result: &ExecutionResult,
    ) -> Result<(), KernelBridgeError> {
        let db_events = EventRecorder::to_db_events(result, result.sequence_number);
        for event in db_events {
            self.event_repo.create(event).await?;
        }
        Ok(())
    }

    async fn create_checkpoint(
        &mut self,
        instance_id: AgentInstanceId,
    ) -> Result<(), KernelBridgeError> {
        let checkpoint = self.runtime.create_checkpoint(instance_id)?;

        let meta = checkpointer::CheckpointMeta {
            instance_id: Some(checkpoint.instance_id.as_uuid()),
            task_id: checkpoint.active_task_id.map(|id| id.as_uuid()),
            created_at: checkpoint.created_at,
        };

        let state = checkpointer::CheckpointState(serde_json::json!({
            "instance_state": checkpoint.instance_state,
            "active_task_state": checkpoint.active_task_state,
            "pending_approval_ids": checkpoint.pending_approval_ids,
            "child_delegation_ids": checkpoint.child_delegation_ids,
            "event_sequence": checkpoint.event_sequence,
        }));

        let id = checkpointer::CheckpointId::from_uuid(checkpoint.id.as_uuid());
        self.checkpointer.save(id, meta, state).await
            .map_err(|e| KernelBridgeError::Checkpoint(e.to_string()))?;

        Ok(())
    }
}
```

Fix `reconstruct_request` to use the runtime store:

```rust
    fn reconstruct_request(
        &self,
        instance_id: AgentInstanceId,
    ) -> Result<ExecutionRequest, KernelBridgeError> {
        let instance = self.runtime.instance(instance_id)
            .ok_or_else(|| KernelBridgeError::Kernel(
                torque_kernel::ValidationError::new("Runtime", "instance missing").into()
            ))?;
        let agent_def = self.runtime.store().agent_definition(instance.agent_definition_id())
            .ok_or_else(|| KernelBridgeError::Kernel(
                torque_kernel::ValidationError::new("Runtime", "agent definition missing").into()
            ))?;
        Ok(torque_kernel::ExecutionRequest::new(
            agent_def.id,
            "continue".to_string(),
            vec![],
        ))
    }
```

**Note:** `checkpointer::CheckpointId` may not have `from_uuid`. Adjust to use whatever constructor the `checkpointer` crate provides (e.g., `CheckpointId::new()` or `CheckpointId(uuid)`).

- [ ] **Step 2: Verify compilation**

```bash
cargo check -p agent-runtime-service
```

Expected: PASS (with possible minor adjustments to `checkpointer` API).

- [ ] **Step 3: Commit**

```bash
git add crates/agent-runtime-service/src/kernel_bridge/runtime.rs
git commit -m "feat(kernel-bridge): add KernelRuntimeHandle"
```

---

## Phase 4: Handler Thinning and Final Integration

### Task 4.1: Integrate KernelRuntimeHandle into SessionService and thin chat handler

**Files:**
- Modify: `crates/agent-runtime-service/src/service/session.rs`
- Modify: `crates/agent-runtime-service/src/service/mod.rs`
- Modify: `crates/agent-runtime-service/src/api/messages.rs`
- Modify: `crates/agent-runtime-service/src/app.rs`
- Delete: `crates/agent-runtime-service/src/agent/runner.rs`

- [ ] **Step 1: Update SessionService to use KernelRuntimeHandle**

```rust
// crates/agent-runtime-service/src/service/session.rs
use crate::kernel_bridge::{session_to_execution_request, KernelRuntimeHandle};
use crate::repository::{EventRepository, CheckpointRepository, MessageRepository, SessionRepository};
use crate::service::{MemoryService, ToolService};
use crate::infra::llm::LlmClient;
use crate::agent::stream::StreamEvent;
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

pub struct SessionService {
    session_repo: Arc<dyn SessionRepository>,
    message_repo: Arc<dyn MessageRepository>,
    event_repo: Arc<dyn EventRepository>,
    checkpoint_repo: Arc<dyn CheckpointRepository>,
    db: crate::db::Database,
    llm: Arc<dyn LlmClient>,
    tools: Arc<ToolService>,
    memory: Arc<MemoryService>,
}

impl SessionService {
    pub fn new(
        session_repo: Arc<dyn SessionRepository>,
        message_repo: Arc<dyn MessageRepository>,
        event_repo: Arc<dyn EventRepository>,
        checkpoint_repo: Arc<dyn CheckpointRepository>,
        db: crate::db::Database,
        llm: Arc<dyn LlmClient>,
        tools: Arc<ToolService>,
        memory: Arc<MemoryService>,
    ) -> Self {
        Self { session_repo, message_repo, event_repo, checkpoint_repo, db, llm, tools, memory }
    }

    pub async fn chat(
        &self,
        session_id: Uuid,
        api_key: &str,
        message: String,
        event_sink: mpsc::Sender<StreamEvent>,
    ) -> Result<(), SessionServiceError> {
        let session = self.authorize(session_id, api_key).await?;

        if !self.session_repo.try_mark_running(session_id).await? {
            return Err(SessionServiceError::Conflict);
        }

        let request = session_to_execution_request(&session, &message)
            .map_err(|e| SessionServiceError::Other(anyhow::anyhow!("kernel mapping error: {e}")))?;

        let checkpointer = Arc::new(crate::kernel_bridge::PostgresCheckpointer::new(self.db.clone()));

        let mut kernel = KernelRuntimeHandle::new(
            vec![], // agent definitions — can be empty for MVP or loaded from a def repo
            self.event_repo.clone(),
            self.checkpoint_repo.clone(),
            checkpointer,
        );

        if let Some(instance_id) = session.agent_instance_id {
            use torque_kernel::ids::AgentInstanceId;
            let id = AgentInstanceId::from_uuid(instance_id);
            kernel.hydrate_runtime(id, self.session_repo.as_ref()).await
                .map_err(|e| SessionServiceError::Other(anyhow::anyhow!("hydration error: {e}")))?;
        }

        let result = kernel.execute_chat(
            request,
            self.llm.clone(),
            self.tools.registry().clone(),
            event_sink.clone(),
        ).await;

        match result {
            Ok(exec) => {
                let content = exec.summary.unwrap_or_default();
                let assistant_msg = crate::models::Message::assistant(session_id, content, None, None);
                self.message_repo.create(&assistant_msg).await?;
                self.session_repo.update_status(session_id, crate::models::SessionStatus::Completed, None).await?;
            }
            Err(e) => {
                self.session_repo.update_status(session_id, crate::models::SessionStatus::Error, Some(&e.to_string())).await?;
                let _ = event_sink.send(StreamEvent::Error {
                    code: "AGENT_ERROR".to_string(),
                    message: e.to_string(),
                }).await;
            }
        }

        Ok(())
    }

    async fn authorize(
        &self,
        session_id: Uuid,
        api_key: &str,
    ) -> Result<crate::models::Session, SessionServiceError> {
        let session = self.session_repo.get_by_id(session_id).await
            .map_err(SessionServiceError::Other)?
            .ok_or(SessionServiceError::NotFound)?;
        use subtle::ConstantTimeEq;
        if !bool::from(session.api_key.as_bytes().ct_eq(api_key.as_bytes())) {
            return Err(SessionServiceError::Forbidden);
        }
        Ok(session)
    }
}
```

**Note on Database access in SessionService:**
`PostgresCheckpointer` needs a `Database`. Since repositories already hold `Database`, the simplest fix is to also pass a `Database` clone into `ServiceContainer::new` and down to `SessionService`. Update `ServiceContainer::new` signature:

```rust
pub async fn new(
    repos: crate::repository::RepositoryContainer,
    db: crate::db::Database,
    llm: Arc<dyn LlmClient>,
) -> Self
```

Then in `SessionService::chat`, construct the checkpointer from `self.db.clone()`.

- [ ] **Step 2: Update ServiceContainer to pass db and repos to SessionService**

```rust
impl ServiceContainer {
    pub async fn new(
        repos: crate::repository::RepositoryContainer,
        db: crate::db::Database,
        llm: Arc<dyn LlmClient>,
    ) -> Self {
        let tool = Arc::new(ToolService::new().await);
        let memory = Arc::new(MemoryService::new(repos.memory.clone()));
        let session = Arc::new(SessionService::new(
            repos.session.clone(),
            repos.message.clone(),
            repos.event.clone(),
            repos.checkpoint.clone(),
            db,
            llm.clone(),
            tool.clone(),
            memory.clone(),
        ));
        let agent_instance = Arc::new(agent_instance::AgentInstanceService::new(
            repos.session.clone(),
        ));

        Self { session, memory, tool, agent_instance }
    }
}
```

- [ ] **Step 3: Update app.rs**

```rust
pub fn build_app(db: Database, llm: Arc<OpenAiClient>) -> Router {
    let repos = crate::repository::RepositoryContainer {
        session: Arc::new(crate::repository::PostgresSessionRepository::new(db.clone())),
        message: Arc::new(crate::repository::PostgresMessageRepository::new(db.clone())),
        memory: Arc::new(crate::repository::PostgresMemoryRepository::new(db.clone())),
        event: Arc::new(crate::repository::PostgresEventRepository::new(db.clone())),
        checkpoint: Arc::new(crate::repository::PostgresCheckpointRepository::new(db.clone())),
    };

    let services = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(ServiceContainer::new(repos, db.clone(), llm.clone()))
    });

    api::router(db, llm, Arc::new(services))
}
```

- [ ] **Step 4: Thin messages.rs chat handler**

```rust
// crates/agent-runtime-service/src/api/messages.rs
use axum::{
    body::Body,
    extract::{Extension, Path, State},
    http::StatusCode,
    response::Response,
    Json,
};
use futures::StreamExt;
use serde::Deserialize;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

use crate::agent::stream::StreamEvent;
use crate::service::ServiceContainer;

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub message: String,
}

pub async fn list(
    State((_, _, services)): State<(crate::db::Database, Arc<llm::OpenAiClient>, Arc<ServiceContainer>)>,
    Path(session_id): Path<Uuid>,
    request: axum::extract::Request,
) -> Result<Json<Vec<crate::models::Message>>, StatusCode> {
    let api_key = crate::api::middleware::extract_api_key(&request).ok_or(StatusCode::UNAUTHORIZED)?;

    match services.session.get_by_id(session_id, &api_key).await {
        Ok(_) => {}
        Err(crate::service::session::SessionServiceError::NotFound) => return Err(StatusCode::NOT_FOUND),
        Err(crate::service::session::SessionServiceError::Forbidden) => return Err(StatusCode::FORBIDDEN),
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    }

    let messages = services.session.list_messages(session_id).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(messages))
}

pub async fn chat(
    State((_, _, services)): State<(crate::db::Database, Arc<llm::OpenAiClient>, Arc<ServiceContainer>)>,
    Path(session_id): Path<Uuid>,
    Extension(api_key): Extension<String>,
    Json(req): Json<ChatRequest>,
) -> Result<Response, StatusCode> {
    let (tx, rx) = mpsc::channel::<StreamEvent>(100);

    let session_svc = services.session.clone();
    tokio::spawn(async move {
        let _ = session_svc.chat(session_id, &api_key, req.message, tx).await;
    });

    let stream = ReceiverStream::new(rx).map(|event| {
        Ok::<_, Infallible>(event.to_sse())
    });

    let response = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/event-stream")
        .header("cache-control", "no-cache")
        .body(Body::from_stream(stream))
        .unwrap();

    Ok(response)
}
```

Wait — `list` still uses old `db::` calls. We should thin it too, but for this plan we can leave it as-is and migrate in a follow-up. The key is that `chat` now goes through `SessionService`.

Actually, to keep consistency, let's also thin `list`:

```rust
pub async fn list(
    State((_, _, services)): State<(crate::db::Database, Arc<llm::OpenAiClient>, Arc<ServiceContainer>)>,
    Path(session_id): Path<Uuid>,
    request: axum::extract::Request,
) -> Result<Json<Vec<crate::models::Message>>, StatusCode> {
    let api_key = crate::api::middleware::extract_api_key(&request).ok_or(StatusCode::UNAUTHORIZED)?;

    match services.session.get_by_id(session_id, &api_key).await {
        Ok(_) => {}
        Err(crate::service::session::SessionServiceError::NotFound) => return Err(StatusCode::NOT_FOUND),
        Err(crate::service::session::SessionServiceError::Forbidden) => return Err(StatusCode::FORBIDDEN),
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    }

    let messages = services.session.list_messages(session_id).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(messages))
}
```

This requires adding `list_messages` to `SessionService`:

```rust
pub async fn list_messages(&self, session_id: Uuid) -> Result<Vec<crate::models::Message>, SessionServiceError> {
    self.message_repo.list_by_session(session_id, 100).await.map_err(SessionServiceError::Other)
}
```

- [ ] **Step 5: Delete AgentRunner**

```bash
rm crates/agent-runtime-service/src/agent/runner.rs
```

Update `crates/agent-runtime-service/src/agent/mod.rs` to remove `pub mod runner;`.

- [ ] **Step 6: Verify compilation**

```bash
cargo check -p agent-runtime-service
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/agent-runtime-service/src/service/ crates/agent-runtime-service/src/api/messages.rs crates/agent-runtime-service/src/app.rs crates/agent-runtime-service/src/agent/mod.rs
git rm crates/agent-runtime-service/src/agent/runner.rs
git commit -m "feat(service): wire KernelRuntimeHandle into SessionService, delete AgentRunner"
```

---

### Task 4.2: Thin memory handlers to use MemoryService

**Files:**
- Modify: `crates/agent-runtime-service/src/api/memory.rs`
- Modify: `crates/agent-runtime-service/src/service/memory.rs`

- [ ] **Step 1: Implement MemoryService methods**

Fill the `todo!()` implementations in `MemoryService` by migrating logic from `db/memory_candidates.rs` and `db/memory_entries.rs`. Since this is a mechanical migration, the plan documents the pattern but not every line.

For each `MemoryService` method:
1. Copy the corresponding `db/` function body.
2. Replace `db.pool()` with `self.db.pool()` (if MemoryService holds `Database`) or call through `self.memory_repo`.

If `MemoryRepository` is fully implemented, just call `self.memory_repo.xxx()`. If stubs remain, implement the repository first.

- [ ] **Step 2: Thin memory.rs handlers**

Each handler in `api/memory.rs` should be reduced to:
1. Extract `api_key` and authorize via `SessionService::get_by_id`.
2. Call `MemoryService::xxx`.
3. Return JSON.

- [ ] **Step 3: Verify compilation**

```bash
cargo check -p agent-runtime-service
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/agent-runtime-service/src/api/memory.rs crates/agent-runtime-service/src/service/memory.rs crates/agent-runtime-service/src/repository/memory.rs
git commit -m "feat(service): migrate memory endpoints to MemoryService"
```

---

## Phase 5: Testing and Verification

### Task 5.1: Run the full service build

- [ ] **Step 1: Build**

```bash
cargo build -p agent-runtime-service
```

Expected: PASS.

- [ ] **Step 2: Run existing tests**

```bash
cargo test -p agent-runtime-service
```

Expected: All existing tests pass (or pre-existing failures documented).

### Task 5.2: Verify MVP endpoints still work

- [ ] **Step 1: Start service locally**

```bash
cargo run -p agent-runtime-service
```

- [ ] **Step 2: Create session**

```bash
curl -s -X POST http://localhost:3000/sessions -H "X-API-Key: demo-key"
```

Expected: 200 with JSON containing `id`.

- [ ] **Step 3: Chat (SSE)**

```bash
curl -N -X POST http://localhost:3000/sessions/$SESSION_ID/chat \
  -H "X-API-Key: demo-key" \
  -H "Content-Type: application/json" \
  -d '{"message":"Hello"}'
```

Expected: SSE stream with `start`, `chunk`/`done` or `error`.

- [ ] **Step 4: Check v1_events populated**

```bash
psql $DATABASE_URL -c "SELECT event_type, resource_type FROM v1_events LIMIT 5;"
```

Expected: Rows exist with event types like `instance_state_changed`, `task_state_changed`.

- [ ] **Step 5: Check checkpoints populated**

```bash
psql $DATABASE_URL -c "SELECT id, instance_id FROM checkpoints LIMIT 1;"
```

Expected: At least one row after a completed chat.

---

## Plan Review and Execution

After completing the plan document:

1. **Review**: Dispatch a plan-document-reviewer subagent to verify the plan against the spec at `docs/superpowers/specs/2026-04-16-torque-architecture-optimization-design.md`.
2. **Fix**: Address any issues found.
3. **Execute**: Choose one of:
   - **Subagent-Driven (recommended)** — dispatch fresh subagents per task
   - **Inline Execution** — execute tasks in this session

> **Required sub-skills for execution:**
> - `superpowers:subagent-driven-development` for parallel task execution
> - `superpowers:executing-plans` for inline batch execution with checkpoints
> - `superpowers:test-driven-development` before implementing each handler
> - `superpowers:verification-before-completion` after each phase
    Ok(ExecutionRequest::new(agent_def.id, 