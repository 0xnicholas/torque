# Torque Platform API v1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the Torque Platform API v1 REST service with SSE streaming runs, async team tasks, and full CRUD for all v1 resources, while keeping the existing MVP API functional.

**Architecture:** Extend `crates/agent-runtime-service` with a parallel `/v1/` router hierarchy, built on top of the `repository/` + `service/` + `kernel-bridge/` layers established by the architecture optimization. HTTP handlers are thin adapters (<30 lines) that delegate to `service/` only. DB access is isolated behind `repository/` async traits. Keep MVP routes intact during migration. Use tokio streams for SSE. Use sqlx for all persistence.

**Tech Stack:** Rust, axum 0.7, tokio, sqlx, serde, uuid, chrono, tracing, tower-http

---

## File Structure Map

### New API route modules
- `crates/agent-runtime-service/src/api/v1/mod.rs` — v1 router assembly
- `crates/agent-runtime-service/src/api/v1/agent_definitions.rs` — `POST|GET /v1/agent-definitions`
- `crates/agent-runtime-service/src/api/v1/agent_instances.rs` — `POST|GET /v1/agent-instances`, `GET|DELETE /v1/agent-instances/{id}`, `POST /v1/agent-instances/{id}/cancel`, `POST /v1/agent-instances/{id}/resume`, `POST /v1/agent-instances/{id}/time-travel`, `GET /v1/agent-instances/{id}/delegations`, `GET /v1/agent-instances/{id}/artifacts`, `GET /v1/agent-instances/{id}/events`, `GET /v1/agent-instances/{id}/checkpoints`
- `crates/agent-runtime-service/src/api/v1/runs.rs` — `POST /v1/agent-instances/{id}/runs` (SSE)
- `crates/agent-runtime-service/src/api/v1/tasks.rs` — `GET /v1/tasks`, `GET /v1/tasks/{id}`, `POST /v1/tasks/{id}/cancel`, `GET /v1/tasks/{id}/events`, `GET /v1/tasks/{id}/approvals`, `GET /v1/tasks/{id}/delegations`
- `crates/agent-runtime-service/src/api/v1/artifacts.rs` — `POST|GET /v1/artifacts`, `GET|DELETE /v1/artifacts/{id}`, `content`, `publish`
- `crates/agent-runtime-service/src/api/v1/memory.rs` — `POST|GET /v1/memory-write-candidates`, `GET /v1/memory-write-candidates/{id}`, `approve|reject`, `GET /v1/memory-entries`, `GET /v1/memory-entries/{id}`, `search`
- `crates/agent-runtime-service/src/api/v1/capabilities.rs` — `POST|GET /v1/capability-profiles`, `GET|DELETE /v1/capability-profiles/{id}`, `bindings`, `resolve`, `POST|GET /v1/capability-registry-bindings`, `GET|DELETE /v1/capability-registry-bindings/{id}`
- `crates/agent-runtime-service/src/api/v1/teams.rs` — `POST|GET /v1/team-definitions`, `GET|DELETE /v1/team-definitions/{id}`, `POST|GET /v1/team-instances`, `GET|DELETE /v1/team-instances/{id}`, `GET /v1/team-instances/{id}/tasks`, `GET /v1/team-instances/{id}/members`, `GET /v1/team-instances/{id}/shared-state`, `GET /v1/team-instances/{id}/artifacts`, `GET /v1/team-instances/{id}/events`, `POST /v1/team-instances/{id}/tasks` (team task creation)
- `crates/agent-runtime-service/src/api/v1/delegations.rs` — `POST|GET /v1/delegations`, `GET /v1/delegations/{id}`, `accept|reject`
- `crates/agent-runtime-service/src/api/v1/approvals.rs` — `GET /v1/approvals`, `GET /v1/approvals/{id}`, `resolve`
- `crates/agent-runtime-service/src/api/v1/checkpoints.rs` — `GET /v1/checkpoints`, `GET /v1/checkpoints/{id}`, `restore`
- `crates/agent-runtime-service/src/api/v1/events.rs` — `GET /v1/events`, `GET /v1/agent-instances/{id}/events`, `GET /v1/team-instances/{id}/events`

### Shared models
- `crates/agent-runtime-service/src/models/v1/mod.rs`
- `crates/agent-runtime-service/src/models/v1/common.rs` — Error, Pagination, ListQuery
- `crates/agent-runtime-service/src/models/v1/agent_definition.rs`
- `crates/agent-runtime-service/src/models/v1/agent_instance.rs`
- `crates/agent-runtime-service/src/models/v1/run.rs`
- `crates/agent-runtime-service/src/models/v1/task.rs`
- `crates/agent-runtime-service/src/models/v1/artifact.rs`
- `crates/agent-runtime-service/src/models/v1/memory.rs`
- `crates/agent-runtime-service/src/models/v1/capability.rs`
- `crates/agent-runtime-service/src/models/v1/team.rs`
- `crates/agent-runtime-service/src/models/v1/delegation.rs`
- `crates/agent-runtime-service/src/models/v1/approval.rs`
- `crates/agent-runtime-service/src/models/v1/checkpoint.rs`
- `crates/agent-runtime-service/src/models/v1/event.rs`

### Repository modules (v1 extensions)
Extends the `repository/` layer established by the architecture optimization.
- `crates/agent-runtime-service/src/repository/agent_definition.rs` — `AgentDefinitionRepository` trait + `PostgresAgentDefinitionRepository`
- `crates/agent-runtime-service/src/repository/agent_instance.rs` — `AgentInstanceRepository` trait + impl
- `crates/agent-runtime-service/src/repository/task.rs` — `TaskRepository` trait + impl
- `crates/agent-runtime-service/src/repository/artifact.rs` — `ArtifactRepository` trait + impl
- `crates/agent-runtime-service/src/repository/memory.rs` — filled `MemoryRepository` + `MemoryWriteCandidateRepository`
- `crates/agent-runtime-service/src/repository/capability.rs` — `CapabilityProfileRepository` + `CapabilityRegistryBindingRepository`
- `crates/agent-runtime-service/src/repository/team.rs` — `TeamDefinitionRepository` + `TeamInstanceRepository`
- `crates/agent-runtime-service/src/repository/delegation.rs` — `DelegationRepository`
- `crates/agent-runtime-service/src/repository/approval.rs` — `ApprovalRepository`

### Service modules (v1)
- `crates/agent-runtime-service/src/service/agent_definition.rs` — `AgentDefinitionService`
- `crates/agent-runtime-service/src/service/agent_instance.rs` — `AgentInstanceService`
- `crates/agent-runtime-service/src/service/task.rs` — `TaskService`
- `crates/agent-runtime-service/src/service/artifact.rs` — `ArtifactService`
- `crates/agent-runtime-service/src/service/memory.rs` — filled `MemoryService`
- `crates/agent-runtime-service/src/service/capability.rs` — `CapabilityService`
- `crates/agent-runtime-service/src/service/team.rs` — `TeamService`
- `crates/agent-runtime-service/src/service/delegation.rs` — `DelegationService`
- `crates/agent-runtime-service/src/service/approval.rs` — `ApprovalService`
- `crates/agent-runtime-service/src/service/mod.rs` — extended `ServiceContainer` with v1 services

### Tests
- `crates/agent-runtime-service/tests/v1_agent_definitions.rs`
- `crates/agent-runtime-service/tests/v1_agent_instances.rs`
- `crates/agent-runtime-service/tests/v1_runs.rs`
- `crates/agent-runtime-service/tests/v1_teams.rs`
- `crates/agent-runtime-service/tests/v1_end_to_end.rs`

### Migrations
- `crates/agent-runtime-service/migrations/20260416000001_create_v1_agent_definitions.up.sql`
- `crates/agent-runtime-service/migrations/20260416000001_create_v1_agent_definitions.down.sql`
- (similar for instances, tasks, artifacts, memory, capabilities, teams, delegations, approvals, checkpoints, events)

### OpenAPI
- `docs/openapi/torque-v1.yaml`

---

## Phase 0: v1 Foundation

### Task 0.1: Create v1 common request/response models

**Files:**
- Create: `crates/agent-runtime-service/src/models/v1/common.rs`
- Modify: `crates/agent-runtime-service/src/models/mod.rs`

- [ ] **Step 1: Write the model file**

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<HashMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Pagination {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev_cursor: Option<String>,
    pub has_more: bool,
}

#[derive(Debug, Serialize)]
pub struct ListResponse<T> {
    pub data: Vec<T>,
    pub pagination: Pagination,
}

#[derive(Debug, Deserialize, Default)]
pub struct ListQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    pub cursor: Option<String>,
    pub sort: Option<String>,
    pub filter_status: Option<String>,
    pub filter_created_after: Option<DateTime<Utc>>,
    pub filter_created_before: Option<DateTime<Utc>>,
    // resource-specific filters passed as extra HashMap or explicit fields per endpoint
}

fn default_limit() -> i64 { 20 }

#[derive(Debug, Deserialize, Default)]
pub struct EventListQuery {
    #[serde(flatten)]
    pub base: ListQuery,
    pub resource_type: Option<String>,
    pub resource_id: Option<String>,
    pub before_event_id: Option<String>,
    pub after_event_id: Option<String>,
    pub event_types: Option<Vec<String>>,
}
```

- [ ] **Step 2: Export from models/mod.rs**

Add to `crates/agent-runtime-service/src/models/mod.rs`:
```rust
pub mod v1;
```

- [ ] **Step 3: Create models/v1/mod.rs**

```rust
pub mod common;
pub mod agent_definition;
// others to come
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p agent-runtime-service`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/agent-runtime-service/src/models/
git commit -m "feat(v1): add common request/response models"
```

---

### Task 0.2: Add v1 router scaffold

**Files:**
- Create: `crates/agent-runtime-service/src/api/v1/mod.rs`
- Modify: `crates/agent-runtime-service/src/api/mod.rs`

- [ ] **Step 1: Write v1 router module**

```rust
use axum::{Router, routing::{get, post, delete}};
use crate::db::Database;
use llm::OpenAiClient;
use std::sync::Arc;

pub mod agent_definitions;
pub mod agent_instances;
pub mod runs;
pub mod tasks;
pub mod artifacts;
pub mod memory;
pub mod capabilities;
pub mod teams;
pub mod delegations;
pub mod approvals;
pub mod checkpoints;
pub mod events;

use crate::service::ServiceContainer;

pub fn router(services: Arc<ServiceContainer>) -> Router {
    Router::new()
        .route("/v1/agent-definitions", post(agent_definitions::create).get(agent_definitions::list))
        .route("/v1/agent-definitions/:id", get(agent_definitions::get).delete(agent_definitions::delete))
        // more routes added in later tasks
        .with_state(services)
}
```

- [ ] **Step 2: Mount v1 router in main api router**

Modify `crates/agent-runtime-service/src/api/mod.rs`:
```rust
pub mod v1;
```

Change the router function signature and mount v1 with its own state:
```rust
pub fn router(
    db: Database,
    llm: Arc<OpenAiClient>,
    services: Arc<ServiceContainer>,
) -> Router {
    let v1_router = v1::router(services);

    Router::new()
        // existing MVP routes...
        .route("/sessions", post(sessions::create).get(sessions::list))
        // ... keep all existing routes ...
        .route("/metrics", get(metrics::get))
        .nest("/", v1_router)
        .layer(middleware::from_fn(auth_middleware))
        .with_state((db, llm, services))
}
```

- [ ] **Step 3: Add placeholder handler**

Create `crates/agent-runtime-service/src/api/v1/agent_definitions.rs`:
```rust
use axum::{extract::State, http::StatusCode, Json};
use crate::service::ServiceContainer;
use std::sync::Arc;

pub async fn create(
    State(_services): State<Arc<ServiceContainer>>,
) -> StatusCode {
    StatusCode::NOT_IMPLEMENTED
}

pub async fn list(
    State(_services): State<Arc<ServiceContainer>>,
) -> StatusCode {
    StatusCode::NOT_IMPLEMENTED
}

pub async fn get(
    State(_services): State<Arc<ServiceContainer>>,
) -> StatusCode {
    StatusCode::NOT_IMPLEMENTED
}

pub async fn delete(
    State(_services): State<Arc<ServiceContainer>>,
) -> StatusCode {
    StatusCode::NOT_IMPLEMENTED
}
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p agent-runtime-service`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/agent-runtime-service/src/api/
git commit -m "feat(v1): add v1 router scaffold"
```

---

### Task 0.3: Add lifecycle guards, idempotency, and run-gate infrastructure

**Files:**
- Create: `crates/agent-runtime-service/src/v1_guards.rs`
- Modify: `crates/agent-runtime-service/src/lib.rs`

- [ ] **Step 1: Write guard module**

```rust
use crate::models::v1::agent_instance::AgentInstanceStatus;
use axum::http::StatusCode;

pub fn allow_run_for_status(status: &AgentInstanceStatus) -> Result<(), StatusCode> {
    match status {
        AgentInstanceStatus::Ready => Ok(()),
        AgentInstanceStatus::Running
        | AgentInstanceStatus::WaitingTool
        | AgentInstanceStatus::WaitingSubagent
        | AgentInstanceStatus::WaitingApproval => Err(StatusCode::CONFLICT),
        _ => Err(StatusCode::CONFLICT),
    }
}

pub fn allow_delete_instance(status: &AgentInstanceStatus) -> Result<(), StatusCode> {
    match status {
        AgentInstanceStatus::Created
        | AgentInstanceStatus::Ready
        | AgentInstanceStatus::Completed
        | AgentInstanceStatus::Failed
        | AgentInstanceStatus::Cancelled => Ok(()),
        _ => Err(StatusCode::CONFLICT),
    }
}
```

- [ ] **Step 2: Add idempotency store**

```rust
use std::collections::HashMap;
use std::sync::Mutex;
use chrono::{DateTime, Utc};

pub struct IdempotencyStore {
    entries: Mutex<HashMap<String, IdempotencyEntry>>,
}

pub struct IdempotencyEntry {
    pub created_at: DateTime<Utc>,
    pub response_json: String,
    pub status_code: u16,
}

impl IdempotencyStore {
    pub fn new() -> Self {
        Self { entries: Mutex::new(HashMap::new()) }
    }
    pub fn get(&self, key: &str) -> Option<IdempotencyEntry> {
        self.entries.lock().unwrap().get(key).cloned()
    }
    pub fn insert(&self, key: String, entry: IdempotencyEntry) {
        self.entries.lock().unwrap().insert(key, entry);
    }
}
```

- [ ] **Step 3: Add run gate**

```rust
use std::collections::HashSet;
use std::sync::Mutex;
use uuid::Uuid;

pub struct RunGate {
    active: Mutex<HashSet<Uuid>>,
}

impl RunGate {
    pub fn new() -> Self {
        Self { active: Mutex::new(HashSet::new()) }
    }
    pub fn try_acquire(&self, id: Uuid) -> bool {
        self.active.lock().unwrap().insert(id)
    }
    pub fn release(&self, id: Uuid) {
        self.active.lock().unwrap().remove(&id);
    }
}
```

- [ ] **Step 4: Export from lib.rs**

Add to `crates/agent-runtime-service/src/lib.rs`:
```rust
pub mod v1_guards;
```

**Note:** `IdempotencyStore` and `RunGate` are wired into `ServiceContainer` during app initialization (see Architecture Optimization Plan). v1 handlers access them via `services.idempotency` and `services.run_gate`.

- [ ] **Step 5: Commit**

```bash
git add crates/agent-runtime-service/src/v1_guards.rs crates/agent-runtime-service/src/lib.rs
git commit -m "feat(v1): add lifecycle guards, idempotency store, and run gate"
```

---

## Phase 1: Agent Definitions

### Task 1.1: Add AgentDefinition model and migration

**Files:**
- Create: `crates/agent-runtime-service/src/models/v1/agent_definition.rs`
- Create: `crates/agent-runtime-service/migrations/20260416000001_create_v1_agent_definitions.up.sql`
- Create: `crates/agent-runtime-service/migrations/20260416000001_create_v1_agent_definitions.down.sql`

- [ ] **Step 1: Write the model**

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Serialize, FromRow)]
pub struct AgentDefinition {
    pub id: Uuid,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    pub tool_policy: serde_json::Value,
    pub memory_policy: serde_json::Value,
    pub delegation_policy: serde_json::Value,
    pub limits: serde_json::Value,
    pub default_model_policy: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct AgentDefinitionCreate {
    pub name: String,
    pub description: Option<String>,
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub tool_policy: serde_json::Value,
    #[serde(default)]
    pub memory_policy: serde_json::Value,
    #[serde(default)]
    pub delegation_policy: serde_json::Value,
    #[serde(default)]
    pub limits: serde_json::Value,
    #[serde(default)]
    pub default_model_policy: serde_json::Value,
}
```

- [ ] **Step 2: Write migration**

```sql
CREATE TABLE v1_agent_definitions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    description TEXT,
    system_prompt TEXT,
    tool_policy JSONB NOT NULL DEFAULT '{}',
    memory_policy JSONB NOT NULL DEFAULT '{}',
    delegation_policy JSONB NOT NULL DEFAULT '{}',
    limits JSONB NOT NULL DEFAULT '{}',
    default_model_policy JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_v1_agent_definitions_name ON v1_agent_definitions(name);
```

Down migration:
```sql
DROP TABLE IF EXISTS v1_agent_definitions;
```

- [ ] **Step 3: Run migration locally**

```bash
cd crates/agent-runtime-service
export DATABASE_URL=postgres://postgres:postgres@localhost/agent_runtime_service
cargo sqlx migrate run
```

- [ ] **Step 4: Commit**

```bash
git add crates/agent-runtime-service/src/models/v1/agent_definition.rs crates/agent-runtime-service/migrations/
git commit -m "feat(v1): add AgentDefinition model and migration"
```

---

### Task 1.2: Implement AgentDefinitionRepository and AgentDefinitionService

**Files:**
- Modify: `crates/agent-runtime-service/src/repository/agent_definition.rs`
- Create: `crates/agent-runtime-service/src/service/agent_definition.rs`
- Modify: `crates/agent-runtime-service/src/service/mod.rs`

- [ ] **Step 1: Implement AgentDefinitionRepository trait and Postgres impl**

Modify `crates/agent-runtime-service/src/repository/agent_definition.rs`:
```rust
use async_trait::async_trait;
use crate::db::Database;
use crate::models::v1::agent_definition::{AgentDefinition, AgentDefinitionCreate};
use uuid::Uuid;

#[async_trait]
pub trait AgentDefinitionRepository: Send + Sync {
    async fn create(&self, req: &AgentDefinitionCreate) -> anyhow::Result<AgentDefinition>;
    async fn list(&self, limit: i64, cursor: Option<Uuid>, sort: Option<&str>) -> anyhow::Result<Vec<AgentDefinition>>;
    async fn get(&self, id: Uuid) -> anyhow::Result<Option<AgentDefinition>>;
    async fn delete(&self, id: Uuid) -> anyhow::Result<bool>;
}

pub struct PostgresAgentDefinitionRepository {
    db: Database,
}

impl PostgresAgentDefinitionRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

#[async_trait]
impl AgentDefinitionRepository for PostgresAgentDefinitionRepository {
    async fn create(&self, req: &AgentDefinitionCreate) -> anyhow::Result<AgentDefinition> {
        let row = sqlx::query_as::<_, AgentDefinition>(
            r#"
            INSERT INTO v1_agent_definitions (name, description, system_prompt, tool_policy, memory_policy, delegation_policy, limits, default_model_policy)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING *
            "#
        )
        .bind(&req.name)
        .bind(&req.description)
        .bind(&req.system_prompt)
        .bind(&req.tool_policy)
        .bind(&req.memory_policy)
        .bind(&req.delegation_policy)
        .bind(&req.limits)
        .bind(&req.default_model_policy)
        .fetch_one(self.db.pool())
        .await?;
        Ok(row)
    }

    async fn list(&self, limit: i64, cursor: Option<Uuid>, sort: Option<&str>) -> anyhow::Result<Vec<AgentDefinition>> {
        let order = match sort {
            Some("-created_at") => "created_at DESC, id DESC",
            Some("created_at") => "created_at ASC, id ASC",
            _ => "id ASC",
        };
        let rows = if let Some(after) = cursor {
            sqlx::query_as::<_, AgentDefinition>(
                &format!("SELECT * FROM v1_agent_definitions WHERE id > $1 ORDER BY {} LIMIT $2", order)
            )
            .bind(after)
            .bind(limit)
            .fetch_all(self.db.pool())
            .await?
        } else {
            sqlx::query_as::<_, AgentDefinition>(
                &format!("SELECT * FROM v1_agent_definitions ORDER BY {} LIMIT $1", order)
            )
            .bind(limit)
            .fetch_all(self.db.pool())
            .await?
        };
        Ok(rows)
    }

    async fn get(&self, id: Uuid) -> anyhow::Result<Option<AgentDefinition>> {
        let row = sqlx::query_as::<_, AgentDefinition>(
            "SELECT * FROM v1_agent_definitions WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(self.db.pool())
        .await?;
        Ok(row)
    }

    async fn delete(&self, id: Uuid) -> anyhow::Result<bool> {
        let result = sqlx::query("DELETE FROM v1_agent_definitions WHERE id = $1")
            .bind(id)
            .execute(self.db.pool())
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
```

- [ ] **Step 2: Implement AgentDefinitionService**

Create `crates/agent-runtime-service/src/service/agent_definition.rs`:
```rust
use crate::models::v1::agent_definition::{AgentDefinition, AgentDefinitionCreate};
use crate::repository::AgentDefinitionRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct AgentDefinitionService {
    repo: Arc<dyn AgentDefinitionRepository>,
}

impl AgentDefinitionService {
    pub fn new(repo: Arc<dyn AgentDefinitionRepository>) -> Self {
        Self { repo }
    }

    pub async fn create(&self, req: AgentDefinitionCreate) -> anyhow::Result<AgentDefinition> {
        self.repo.create(&req).await
    }

    pub async fn list(&self, limit: i64, cursor: Option<Uuid>, sort: Option<&str>) -> anyhow::Result<Vec<AgentDefinition>> {
        self.repo.list(limit, cursor, sort).await
    }

    pub async fn get(&self, id: Uuid) -> anyhow::Result<Option<AgentDefinition>> {
        self.repo.get(id).await
    }

    pub async fn delete(&self, id: Uuid) -> anyhow::Result<bool> {
        self.repo.delete(id).await
    }
}
```

- [ ] **Step 3: Wire into RepositoryContainer and ServiceContainer**

Modify `crates/agent-runtime-service/src/repository/mod.rs`:
```rust
pub mod agent_definition;
pub use agent_definition::{AgentDefinitionRepository, PostgresAgentDefinitionRepository};
```

Update `RepositoryContainer` to include:
```rust
pub agent_definition: Arc<dyn AgentDefinitionRepository>,
```

And in `app.rs`, when constructing `RepositoryContainer`, add:
```rust
agent_definition: Arc::new(crate::repository::PostgresAgentDefinitionRepository::new(db.clone())),
```

Modify `crates/agent-runtime-service/src/service/mod.rs`:
```rust
pub mod agent_definition;
pub use agent_definition::AgentDefinitionService;
```

Update `ServiceContainer` to include:
```rust
pub agent_definition: Arc<AgentDefinitionService>,
```

And in `ServiceContainer::new`, add before `Self { ... }`:
```rust
let agent_definition = Arc::new(AgentDefinitionService::new(repos.agent_definition.clone()));
```

Update the `Self { ... }` return to include `agent_definition,`.

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p agent-runtime-service`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/agent-runtime-service/src/repository/agent_definition.rs crates/agent-runtime-service/src/service/
git commit -m "feat(v1): add AgentDefinitionRepository and AgentDefinitionService"
```

---

### Task 1.3: Implement AgentDefinition HTTP handlers

**Files:**
- Modify: `crates/agent-runtime-service/src/api/v1/agent_definitions.rs`
- Modify: `crates/agent-runtime-service/src/api/v1/mod.rs`

- [ ] **Step 1: Implement handlers**

```rust
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use crate::models::v1::agent_definition::{AgentDefinition, AgentDefinitionCreate};
use crate::models::v1::common::{ErrorBody, ListQuery, ListResponse, Pagination};
use crate::service::ServiceContainer;
use std::sync::Arc;
use uuid::Uuid;

pub async fn create(
    State(services): State<Arc<ServiceContainer>>,
    Json(req): Json<AgentDefinitionCreate>,
) -> Result<(StatusCode, Json<AgentDefinition>), (StatusCode, Json<ErrorBody>)> {
    let def = services.agent_definition.create(req).await
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody {
                code: "DB_ERROR".into(),
                message: e.to_string(),
                details: None,
                request_id: None,
            })
        ))?;
    Ok((StatusCode::CREATED, Json(def)))
}

pub async fn list(
    State(services): State<Arc<ServiceContainer>>,
    Query(q): Query<ListQuery>,
) -> Result<Json<ListResponse<AgentDefinition>>, (StatusCode, Json<ErrorBody>)> {
    let limit = q.limit.clamp(1, 100);
    let cursor = q.cursor.and_then(|s| Uuid::parse_str(&s).ok());
    let mut rows = services.agent_definition.list(limit + 1, cursor, q.sort.as_deref()).await
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody { code: "DB_ERROR".into(), message: e.to_string(), details: None, request_id: None })
        ))?;
    let has_more = rows.len() > limit as usize;
    if has_more { rows.pop(); }
    let next_cursor = rows.last().map(|r| r.id.to_string());
    Ok(Json(ListResponse {
        data: rows,
        pagination: Pagination { next_cursor, prev_cursor: q.cursor, has_more },
    }))
}

pub async fn get(
    State(services): State<Arc<ServiceContainer>>,
    Path(id): Path<Uuid>,
) -> Result<Json<AgentDefinition>, StatusCode> {
    match services.agent_definition.get(id).await {
        Ok(Some(def)) => Ok(Json(def)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn delete(
    State(services): State<Arc<ServiceContainer>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    match services.agent_definition.delete(id).await {
        Ok(true) => Ok(StatusCode::NO_CONTENT),
        Ok(false) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}
```

- [ ] **Step 2: Ensure routes are wired**

In `src/api/v1/mod.rs`, verify the routes exist:
```rust
.route("/v1/agent-definitions", post(agent_definitions::create).get(agent_definitions::list))
.route("/v1/agent-definitions/:id", get(agent_definitions::get).delete(agent_definitions::delete))
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p agent-runtime-service`
Expected: PASS

- [ ] **Step 4: Write integration test**

Create `crates/agent-runtime-service/tests/v1_agent_definitions.rs`:
```rust
use agent_runtime_service::models::v1::agent_definition::AgentDefinition;

#[tokio::test]
async fn test_create_and_get_agent_definition() {
    // Use test helpers to spin up service with test DB
    let client = test_helpers::v1_client().await;

    let res = client
        .post("/v1/agent-definitions")
        .json(&serde_json::json!({ "name": "test-agent" }))
        .send()
        .await;
    assert_eq!(res.status(), 201);

    let body: AgentDefinition = res.json().await;
    assert_eq!(body.name, "test-agent");

    let get_res = client.get(&format!("/v1/agent-definitions/{}", body.id)).send().await;
    assert_eq!(get_res.status(), 200);
}
```

- [ ] **Step 5: Commit**

```bash
git add crates/agent-runtime-service/src/api/v1/agent_definitions.rs crates/agent-runtime-service/tests/v1_agent_definitions.rs
git commit -m "feat(v1): implement AgentDefinition handlers and tests"
```

---

## Phase 2: Agent Instances and Runs (SSE)

### Task 2.1: Add AgentInstance model, migration, and DB layer

**Files:**
- Create: `crates/agent-runtime-service/src/models/v1/agent_instance.rs`
- Create: `crates/agent-runtime-service/migrations/20260416000002_create_v1_agent_instances.up.sql`
- Create: `crates/agent-runtime-service/src/repository/agent_instance.rs`
- Create: `crates/agent-runtime-service/src/service/agent_instance.rs`

- [ ] **Step 1: Write model**

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, sqlx::Type, Serialize, Deserialize)]
#[sqlx(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AgentInstanceStatus {
    Created,
    Hydrating,
    Ready,
    Running,
    WaitingTool,
    WaitingSubagent,
    WaitingApproval,
    Suspended,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Serialize, FromRow)]
pub struct AgentInstance {
    pub id: Uuid,
    pub agent_definition_id: Uuid,
    pub status: AgentInstanceStatus,
    pub external_context_refs: serde_json::Value,
    pub current_task_id: Option<Uuid>,
    pub checkpoint_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct AgentInstanceCreate {
    pub agent_definition_id: Uuid,
    #[serde(default)]
    pub external_context_refs: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct TimeTravelRequest {
    pub checkpoint_id: Uuid,
    pub branch_name: Option<String>,
}
```

- [ ] **Step 2: Write migration**

```sql
CREATE TABLE v1_agent_instances (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_definition_id UUID NOT NULL REFERENCES v1_agent_definitions(id),
    status TEXT NOT NULL DEFAULT 'CREATED',
    external_context_refs JSONB NOT NULL DEFAULT '[]',
    current_task_id UUID,
    checkpoint_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

- [ ] **Step 3: Write DB layer**

Functions: `create`, `list`, `get`, `delete`, `update_status`.

- [ ] **Step 4: Commit**

```bash
git add crates/agent-runtime-service/src/models/v1/agent_instance.rs crates/agent-runtime-service/src/repository/agent_instance.rs crates/agent-runtime-service/src/service/agent_instance.rs crates/agent-runtime-service/migrations/
git commit -m "feat(v1): add AgentInstance model, migration, and DB layer"
```

---

### Task 2.2: Implement AgentInstance HTTP handlers

**Files:**
- Create: `crates/agent-runtime-service/src/api/v1/agent_instances.rs`
- Modify: `crates/agent-runtime-service/src/api/v1/mod.rs`

- [ ] **Step 1: Implement CRUD + lifecycle handlers**

`create`, `list`, `get`, `delete`, `cancel`, `resume`, `time_travel`.

- [ ] **Step 2: Implement sub-resource handlers**

`list_delegations`, `list_artifacts`, `list_events`, `list_checkpoints`.

- [ ] **Step 3: Add routes**

```rust
.route("/v1/agent-instances", post(agent_instances::create).get(agent_instances::list))
.route("/v1/agent-instances/:id", get(agent_instances::get).delete(agent_instances::delete))
.route("/v1/agent-instances/:id/cancel", post(agent_instances::cancel))
.route("/v1/agent-instances/:id/resume", post(agent_instances::resume))
.route("/v1/agent-instances/:id/time-travel", post(agent_instances::time_travel))
.route("/v1/agent-instances/:id/delegations", get(agent_instances::list_delegations))
.route("/v1/agent-instances/:id/artifacts", get(agent_instances::list_artifacts))
.route("/v1/agent-instances/:id/events", get(agent_instances::list_events))
.route("/v1/agent-instances/:id/checkpoints", get(agent_instances::list_checkpoints))
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p agent-runtime-service`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/agent-runtime-service/src/api/v1/agent_instances.rs crates/agent-runtime-service/src/api/v1/mod.rs
git commit -m "feat(v1): implement AgentInstance handlers"
```

---

### Task 2.3: Add Run model and SSE streaming handler

**Files:**
- Create: `crates/agent-runtime-service/src/models/v1/run.rs`
- Create: `crates/agent-runtime-service/src/api/v1/runs.rs`
- Modify: `crates/agent-runtime-service/src/api/v1/mod.rs`

- [ ] **Step 1: Write RunRequest model**

```rust
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct RunRequest {
    pub goal: String,
    pub instructions: Option<String>,
    #[serde(default)]
    pub input_artifacts: Vec<Uuid>,
    #[serde(default)]
    pub external_context_refs: Vec<serde_json::Value>,
    #[serde(default)]
    pub constraints: serde_json::Value,
    #[serde(default)]
    pub execution_mode: String,
    #[serde(default)]
    pub expected_outputs: Vec<String>,
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RunEvent {
    pub event: String,
    pub data: serde_json::Value,
}
```

- [ ] **Step 2: Implement SSE handler**

Use `axum::response::sse::{Event, Sse}` and `tokio_stream::Stream`:

```rust
use axum::{
    extract::{Path, State},
    response::sse::{Event, Sse},
    Json,
};
use crate::models::v1::run::{RunEvent, RunRequest};
use crate::service::ServiceContainer;
use std::sync::Arc;
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

pub async fn run(
    State(_services): State<Arc<ServiceContainer>>,
    Path(id): Path<Uuid>,
    Json(_req): Json<RunRequest>,
) -> Sse<ReceiverStream<Result<Event, axum::Error>>> {
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, axum::Error>>(32);

    tokio::spawn(async move {
        let start = RunEvent {
            event: "run.started".into(),
            data: serde_json::json!({"run_id": Uuid::new_v4(), "instance_id": id, "status": "RUNNING"}),
        };
        let _ = tx.send(Ok(Event::default().event("run.started").json_data(start).unwrap())).await;

        // TODO: wire into actual agent runner
        // Stream chunks from llm/chat completion

        let done = RunEvent {
            event: "run.completed".into(),
            data: serde_json::json!({"run_id": Uuid::new_v4(), "instance_id": id, "status": "COMPLETED"}),
        };
        let _ = tx.send(Ok(Event::default().event("run.completed").json_data(done).unwrap())).await;
    });

    Sse::new(ReceiverStream::new(rx))
}
```

- [ ] **Step 3: Add route**

```rust
.route("/v1/agent-instances/:id/runs", post(runs::run))
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p agent-runtime-service`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/agent-runtime-service/src/api/v1/runs.rs crates/agent-runtime-service/src/models/v1/run.rs
git commit -m "feat(v1): add Run model and SSE handler scaffold"
```

---

## Phase 3: Tasks, Artifacts, and Events

### Task 3.1: Task model, migration, DB, and handlers

**Files:**
- Create: `crates/agent-runtime-service/src/models/v1/task.rs`
- Create: migration
- Create: `crates/agent-runtime-service/src/repository/task.rs`
- Create: `crates/agent-runtime-service/src/service/task.rs`
- Modify: `crates/agent-runtime-service/src/api/v1/tasks.rs`

- [ ] **Step 1: Write model**

```rust
#[derive(Debug, sqlx::Type, Serialize, Deserialize)]
#[sqlx(rename_all = "snake_case")]
pub enum TaskType {
    AgentTask,
    TeamTask,
}

#[derive(Debug, Serialize, FromRow)]
pub struct Task {
    pub id: Uuid,
    pub task_type: TaskType,
    pub parent_task_id: Option<Uuid>,
    pub agent_instance_id: Option<Uuid>,
    pub team_instance_id: Option<Uuid>,
    pub status: String,
    pub goal: String,
    pub instructions: Option<String>,
    pub input_artifacts: serde_json::Value,
    pub produced_artifacts: serde_json::Value,
    pub delegation_ids: serde_json::Value,
    pub approval_ids: serde_json::Value,
    pub checkpoint_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

- [ ] **Step 2: Write migration**

```sql
CREATE TABLE v1_tasks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    task_type TEXT NOT NULL,
    parent_task_id UUID,
    agent_instance_id UUID,
    team_instance_id UUID,
    status TEXT NOT NULL,
    goal TEXT NOT NULL,
    instructions TEXT,
    input_artifacts JSONB NOT NULL DEFAULT '[]',
    produced_artifacts JSONB NOT NULL DEFAULT '[]',
    delegation_ids JSONB NOT NULL DEFAULT '[]',
    approval_ids JSONB NOT NULL DEFAULT '[]',
    checkpoint_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

- [ ] **Step 3: Implement handlers**

`GET /v1/tasks`, `GET /v1/tasks/:id`, `POST /v1/tasks/:id/cancel`, `GET /v1/tasks/:id/events`, `GET /v1/tasks/:id/approvals`, `GET /v1/tasks/:id/delegations`.

- [ ] **Step 4: Add routes**

```rust
.route("/v1/tasks", get(tasks::list))
.route("/v1/tasks/:id", get(tasks::get))
.route("/v1/tasks/:id/cancel", post(tasks::cancel))
.route("/v1/tasks/:id/events", get(tasks::list_events))
.route("/v1/tasks/:id/approvals", get(tasks::list_approvals))
.route("/v1/tasks/:id/delegations", get(tasks::list_delegations))
```

- [ ] **Step 5: Commit**

```bash
git add crates/agent-runtime-service/src/models/v1/task.rs crates/agent-runtime-service/src/repository/task.rs crates/agent-runtime-service/src/service/task.rs crates/agent-runtime-service/src/api/v1/tasks.rs
git commit -m "feat(v1): implement Task resource"
```

---

### Task 3.2: Artifact model, migration, DB, and handlers

**Files:**
- Create: `crates/agent-runtime-service/src/models/v1/artifact.rs`
- Create: migration
- Create: `crates/agent-runtime-service/src/repository/artifact.rs`
- Create: `crates/agent-runtime-service/src/service/artifact.rs`
- Modify: `crates/agent-runtime-service/src/api/v1/artifacts.rs`

- [ ] **Step 1: Write model**

```rust
#[derive(Debug, sqlx::Type, Serialize, Deserialize)]
#[sqlx(rename_all = "snake_case")]
pub enum ArtifactScope {
    Private,
    TeamShared,
    ExternalPublished,
}

#[derive(Debug, Serialize, FromRow)]
pub struct Artifact {
    pub id: Uuid,
    pub kind: String,
    pub scope: ArtifactScope,
    pub source_instance_id: Option<Uuid>,
    pub published_to_team_instance_id: Option<Uuid>,
    pub mime_type: String,
    pub size_bytes: i64,
    pub summary: Option<String>,
    pub content: serde_json::Value,
    pub created_at: DateTime<Utc>,
}
```

- [ ] **Step 2: Write migration**

```sql
CREATE TABLE v1_artifacts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    kind TEXT NOT NULL,
    scope TEXT NOT NULL DEFAULT 'private',
    source_instance_id UUID,
    published_to_team_instance_id UUID,
    mime_type TEXT NOT NULL,
    size_bytes BIGINT NOT NULL DEFAULT 0,
    summary TEXT,
    content JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

- [ ] **Step 3: Implement handlers**

`POST|GET /v1/artifacts`, `GET /v1/artifacts/:id`, `GET /v1/artifacts/:id/content`, `POST /v1/artifacts/:id/publish`, `DELETE /v1/artifacts/:id`.

- [ ] **Step 4: Commit**

```bash
git add crates/agent-runtime-service/src/models/v1/artifact.rs crates/agent-runtime-service/src/repository/artifact.rs crates/agent-runtime-service/src/service/artifact.rs crates/agent-runtime-service/src/api/v1/artifacts.rs
git commit -m "feat(v1): implement Artifact resource"
```

---

### Task 3.3: Event read-only endpoint

**Files:**
- Create: `crates/agent-runtime-service/src/models/v1/event.rs`
- Create: `crates/agent-runtime-service/src/repository/event.rs` (already exists from architecture optimization — extend with `list_by_types` if missing)
- Modify: `crates/agent-runtime-service/src/api/v1/events.rs`

- [ ] **Step 1: Write model**

```rust
#[derive(Debug, Serialize, FromRow)]
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

- [ ] **Step 2: Verify events table exists**

The `v1_events` table and `Event` model were created by the Architecture Optimization Plan (migration `20260416000003_create_v1_events`). Ensure the model above aligns with that schema.

- [ ] **Step 3: Implement handler**

`GET /v1/events`, `GET /v1/agent-instances/:id/events`, `GET /v1/team-instances/:id/events`. All use `EventListQuery` with `event_types` filter.

- [ ] **Step 4: Add routes**

```rust
.route("/v1/events", get(events::list))
// agent_instances.rs:
.route("/v1/agent-instances/:id/events", get(agent_instances::list_events))
// teams.rs:
.route("/v1/team-instances/:id/events", get(teams::list_events))
```

- [ ] **Step 5: Commit**

```bash
git add crates/agent-runtime-service/src/models/v1/event.rs crates/agent-runtime-service/src/repository/event.rs crates/agent-runtime-service/src/api/v1/events.rs
git commit -m "feat(v1): implement Event read-only endpoint"
```

---

## Phase 4: Memory and Capability

### Task 4.1: MemoryWriteCandidate and MemoryEntry

**Files:**
- Create: `crates/agent-runtime-service/src/models/v1/memory.rs`
- Create: migrations
- Modify: `crates/agent-runtime-service/src/repository/memory.rs`
- Modify: `crates/agent-runtime-service/src/service/memory.rs`
- Modify: `crates/agent-runtime-service/src/api/v1/memory.rs`

- [ ] **Step 1: Write models**

```rust
#[derive(Debug, sqlx::Type, Serialize, Deserialize)]
#[sqlx(rename_all = "snake_case")]
pub enum MemoryCategory {
    AgentProfileMemory,
    UserPreferenceMemory,
    TaskOrDomainMemory,
    ExternalContextMemory,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MemoryContent {
    pub category: MemoryCategory,
    pub key: String,
    pub value: serde_json::Value,
}

#[derive(Debug, sqlx::Type, Serialize, Deserialize)]
#[sqlx(rename_all = "UPPERCASE")]
pub enum MemoryWriteCandidateStatus {
    Pending,
    Approved,
    Rejected,
}

#[derive(Debug, Serialize, FromRow)]
pub struct MemoryWriteCandidate {
    pub id: Uuid,
    pub agent_instance_id: Uuid,
    pub team_instance_id: Option<Uuid>,
    pub content: serde_json::Value, // MemoryContent as JSON
    pub reasoning: Option<String>,
    pub status: MemoryWriteCandidateStatus,
    pub memory_entry_id: Option<Uuid>,
    pub reviewed_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub reviewed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct MemoryWriteCandidateCreate {
    pub agent_instance_id: Uuid,
    pub content: MemoryContent,
    pub reasoning: Option<String>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct MemoryEntry {
    pub id: Uuid,
    pub agent_instance_id: Option<Uuid>,
    pub team_instance_id: Option<Uuid>,
    pub category: MemoryCategory,
    pub key: String,
    pub value: serde_json::Value,
    pub source_candidate_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

- [ ] **Step 2: Write migrations**

```sql
CREATE TABLE v1_memory_write_candidates (...);
CREATE TABLE v1_memory_entries (...);
```

- [ ] **Step 3: Implement handlers**

`POST|GET /v1/memory-write-candidates`, `POST /v1/memory-write-candidates/:id/approve`, `POST /v1/memory-write-candidates/:id/reject`, `GET /v1/memory-entries`, `GET /v1/memory-entries/search`.

- [ ] **Step 4: Commit**

```bash
git add crates/agent-runtime-service/src/models/v1/memory.rs crates/agent-runtime-service/src/repository/memory.rs crates/agent-runtime-service/src/service/memory.rs crates/agent-runtime-service/src/api/v1/memory.rs
git commit -m "feat(v1): implement Memory resources"
```

---

### Task 4.2: CapabilityProfile and CapabilityRegistryBinding

**Files:**
- Create: `crates/agent-runtime-service/src/models/v1/capability.rs`
- Create: migrations
- Create: `crates/agent-runtime-service/src/repository/capability.rs`
- Create: `crates/agent-runtime-service/src/service/capability.rs`
- Modify: `crates/agent-runtime-service/src/api/v1/capabilities.rs`

- [ ] **Step 1: Write models**

```rust
#[derive(Debug, sqlx::Type, Serialize, Deserialize)]
#[sqlx(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, sqlx::Type, Serialize, Deserialize)]
#[sqlx(rename_all = "snake_case")]
pub enum QualityTier {
    Experimental,
    Beta,
    Production,
}

#[derive(Debug, Serialize, FromRow)]
pub struct CapabilityProfile {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub input_contract: Option<serde_json::Value>,
    pub output_contract: Option<serde_json::Value>,
    pub risk_level: RiskLevel,
    pub default_agent_definition_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CapabilityProfileCreate {
    pub name: String,
    pub description: Option<String>,
    pub input_contract: Option<serde_json::Value>,
    pub output_contract: Option<serde_json::Value>,
    #[serde(default)]
    pub risk_level: RiskLevel,
    pub default_agent_definition_id: Option<Uuid>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct CapabilityRegistryBinding {
    pub id: Uuid,
    pub capability_profile_id: Uuid,
    pub agent_definition_id: Uuid,
    pub compatibility_score: Option<f64>,
    pub quality_tier: QualityTier,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CapabilityRegistryBindingCreate {
    pub capability_profile_id: Uuid,
    pub agent_definition_id: Uuid,
    pub compatibility_score: Option<f64>,
    #[serde(default)]
    pub quality_tier: QualityTier,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct CapabilityResolveRequest {
    pub team_instance_id: Option<Uuid>,
    pub team_task_id: Option<Uuid>,
    pub selector_id: Option<String>,
    pub constraints: Option<serde_json::Value>,
}
```

- [ ] **Step 2: Write migrations**

```sql
CREATE TABLE v1_capability_profiles (...);
CREATE TABLE v1_capability_registry_bindings (...);
```

- [ ] **Step 3: Implement handlers**

`POST|GET /v1/capability-profiles`, `POST|GET /v1/capability-registry-bindings`, `POST /v1/capability-profiles/:id/resolve`.

- [ ] **Step 4: Commit**

```bash
git add crates/agent-runtime-service/src/models/v1/capability.rs crates/agent-runtime-service/src/repository/capability.rs crates/agent-runtime-service/src/service/capability.rs crates/agent-runtime-service/src/api/v1/capabilities.rs
git commit -m "feat(v1): implement Capability resources"
```

---

## Phase 5: Teams, Delegations, Approvals, Checkpoints

### Task 5.1: TeamDefinition and TeamInstance

**Files:**
- Create: `crates/agent-runtime-service/src/models/v1/team.rs`
- Create: migrations
- Create: `crates/agent-runtime-service/src/repository/team.rs`
- Create: `crates/agent-runtime-service/src/service/team.rs`
- Modify: `crates/agent-runtime-service/src/api/v1/teams.rs`

- [ ] **Step 1: Write models**

```rust
#[derive(Debug, Serialize, FromRow)]
pub struct TeamDefinition {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub supervisor_agent_definition_id: Uuid,
    pub sub_agents: serde_json::Value,
    pub policy: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct TeamInstance {
    pub id: Uuid,
    pub team_definition_id: Uuid,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct TeamTaskCreate {
    pub goal: String,
    pub instructions: Option<String>,
    pub idempotency_key: String,
    #[serde(default)]
    pub input_artifacts: Vec<Uuid>,
    #[serde(default)]
    pub parent_task_id: Option<Uuid>,
}
```

- [ ] **Step 2: Write migrations**

```sql
CREATE TABLE v1_team_definitions (...);
CREATE TABLE v1_team_instances (...);
```

- [ ] **Step 3: Implement handlers**

`POST|GET /v1/team-definitions`, `GET|DELETE /v1/team-definitions/:id`, `POST|GET /v1/team-instances`, `GET|DELETE /v1/team-instances/:id`, `GET /v1/team-instances/:id/tasks`, `GET /v1/team-instances/:id/members`, `GET /v1/team-instances/:id/shared-state`, `GET /v1/team-instances/:id/artifacts`, `GET /v1/team-instances/:id/events`, `POST /v1/team-instances/:id/tasks` (creates a TeamTask, returns 202).

- [ ] **Step 4: Commit**

```bash
git add crates/agent-runtime-service/src/models/v1/team.rs crates/agent-runtime-service/src/repository/team.rs crates/agent-runtime-service/src/service/team.rs crates/agent-runtime-service/src/api/v1/teams.rs
git commit -m "feat(v1): implement Team resources"
```

---

### Task 5.2: Delegation and Approval

**Files:**
- Create: `crates/agent-runtime-service/src/models/v1/delegation.rs`
- Create: `crates/agent-runtime-service/src/models/v1/approval.rs`
- Create: migrations
- Create: `crates/agent-runtime-service/src/repository/delegation.rs`
- Create: `crates/agent-runtime-service/src/repository/approval.rs`
- Create: `crates/agent-runtime-service/src/service/delegation.rs`
- Create: `crates/agent-runtime-service/src/service/approval.rs`
- Modify: `crates/agent-runtime-service/src/api/v1/delegations.rs`
- Modify: `crates/agent-runtime-service/src/api/v1/approvals.rs`

- [ ] **Step 1: Write models and migrations**

```rust
#[derive(Debug, Serialize, FromRow)]
pub struct Delegation {
    pub id: Uuid,
    pub task_id: Uuid,
    pub parent_agent_instance_id: Uuid,
    pub child_agent_definition_selector: serde_json::Value,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct Approval {
    pub id: Uuid,
    pub task_id: Uuid,
    pub approval_type: String,
    pub status: String,
    pub requested_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}
```

- [ ] **Step 2: Implement handlers**

Delegations: `POST|GET /v1/delegations`, `POST /v1/delegations/:id/accept`, `POST /v1/delegations/:id/reject`.
Approvals: `GET /v1/approvals`, `POST /v1/approvals/:id/resolve`.

- [ ] **Step 3: Commit**

```bash
git add crates/agent-runtime-service/src/models/v1/delegation.rs crates/agent-runtime-service/src/models/v1/approval.rs crates/agent-runtime-service/src/repository/delegation.rs crates/agent-runtime-service/src/repository/approval.rs crates/agent-runtime-service/src/service/delegation.rs crates/agent-runtime-service/src/service/approval.rs crates/agent-runtime-service/src/api/v1/delegations.rs crates/agent-runtime-service/src/api/v1/approvals.rs
git commit -m "feat(v1): implement Delegation and Approval resources"
```

---

### Task 5.3: Checkpoint

**Files:**
- Create: `crates/agent-runtime-service/src/models/v1/checkpoint.rs`
- Create: migration
- Modify: `crates/agent-runtime-service/src/repository/checkpoint.rs` (already exists from architecture optimization)
- Modify: `crates/agent-runtime-service/src/api/v1/checkpoints.rs`

- [ ] **Step 1: Write model and migration**

```rust
#[derive(Debug, Serialize, FromRow)]
pub struct Checkpoint {
    pub id: Uuid,
    pub agent_instance_id: Uuid,
    pub task_id: Option<Uuid>,
    pub snapshot: serde_json::Value,
    pub created_at: DateTime<Utc>,
}
```

- [ ] **Step 2: Implement handlers**

`GET /v1/agent-instances/:id/checkpoints`, `GET /v1/checkpoints/:id`, `POST /v1/checkpoints/:id/restore`.

- [ ] **Step 3: Commit**

```bash
git add crates/agent-runtime-service/src/models/v1/checkpoint.rs crates/agent-runtime-service/src/repository/checkpoint.rs crates/agent-runtime-service/src/api/v1/checkpoints.rs
git commit -m "feat(v1): implement Checkpoint resource"
```

---

## Phase 6: OpenAPI, Integration Tests, and Migration

### Task 6.1: Write OpenAPI 3.1 YAML

**Files:**
- Create: `docs/openapi/torque-v1.yaml`

- [ ] **Step 1: Write minimal OpenAPI skeleton**

Use the structure from the spec document and fill in at least:
- `info`, `servers`, `security`
- All path definitions with method, summary, requestBody, responses
- All component schemas derived from the Rust models

- [ ] **Step 2: Validate YAML**

Run: `docker run --rm -v $(pwd)/docs/openapi:/spec redocly/cli lint torque-v1.yaml` (or equivalent swagger-editor check)
Expected: No structural errors

- [ ] **Step 3: Commit**

```bash
git add docs/openapi/torque-v1.yaml
git commit -m "docs: add OpenAPI 3.1 spec for v1 Platform API"
```

---

### Task 6.2: Integration tests for v1 end-to-end flows

**Files:**
- Create: `crates/agent-runtime-service/tests/v1_end_to_end.rs`

- [ ] **Step 1: Write test helpers**

```rust
pub mod test_helpers {
    use agent_runtime_service::app::build_app;
    use agent_runtime_service::db::Database;
    use reqwest::Client;
    use sqlx::PgPool;
    use std::sync::Arc;

    pub async fn v1_client() -> Client {
        // spin up test DB, migrate, build app, return reqwest client pointed at test server
    }
}
```

- [ ] **Step 2: Write end-to-end tests**

Tests:
- Create agent definition → create instance → run (SSE) → get task → get events
- Create team definition → create team instance → create team task (202) → poll task → get artifacts
- Memory candidate → approve → search memory entries
- Capability profile → create binding → resolve

- [ ] **Step 3: Run tests**

Run: `cargo test -p agent-runtime-service --test v1_end_to_end`
Expected: PASS (or document expected failures for unimplemented phases)

- [ ] **Step 4: Commit**

```bash
git add crates/agent-runtime-service/tests/v1_end_to_end.rs
git commit -m "test(v1): add end-to-end integration tests"
```

---

### Task 6.3: MVP-to-v1 migration notes and cleanup

**Files:**
- Modify: `crates/agent-runtime-service/README.md`
- Modify: `docs/superpowers/specs/2026-04-16-torque-platform-api-design.md` (if needed)

- [ ] **Step 1: Document migration path**

In README, add a section:
```markdown
## API Versions

- `/` — Legacy MVP API (sessions, chat, memory). Deprecated.
- `/v1/` — New Platform API (agents, teams, tasks, artifacts, etc.).
```

- [ ] **Step 2: Commit**

```bash
git add crates/agent-runtime-service/README.md
git commit -m "docs: document v1 API and MVP deprecation"
```

---

## Plan Review and Execution

After completing the plan document:

1. **Review**: Dispatch a plan-document-reviewer subagent to verify the plan against the spec.
2. **Fix**: Address any issues found.
3. **Execute**: Choose one of:
   - **Subagent-Driven (recommended)** — dispatch fresh subagents per task
   - **Inline Execution** — execute tasks in this session

> **Required sub-skills for execution:**
> - `superpowers:subagent-driven-development` for parallel task execution
> - `superpowers:executing-plans` for inline batch execution with checkpoints
> - `superpowers:test-driven-development` before implementing each handler
> - `superpowers:verification-before-completion` after each phase
