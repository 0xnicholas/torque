# Torque Architecture Optimization Design

> Date: 2026-04-16  
> Scope: `agent-runtime-service` internal refactoring + `torque-kernel` integration + `checkpointer` bridge  
> Goal: Make the kernel actually run, establish clean api/service/repository layers, and wire event persistence and checkpoints.

---

## 1. Overview and Core Principles

### 1.1 Problem Statement

The current codebase has six structural issues:

1. **Kernel is bypassed** — `torque-kernel` defines a full state machine, execution engine, and recovery model, but `agent-runtime-service` constructs kernel objects and discards them; `AgentRunner` calls the LLM client directly.
2. **HTTP layer contains all business logic** — Handlers like `messages::chat` are 150+ lines, mixing auth, DB gating, tool registration, orchestration, and SSE assembly.
3. **ToolRegistry is rebuilt on every request** — No shared lifecycle; builtin tools are re-registered inside every chat handler.
4. **Checkpointer is isolated** — `crates/checkpointer` has a trait and hybrid impl, but `agent-runtime-service` never uses it; `torque-kernel` defines its own `Checkpoint`.
5. **Semantic gap between MVP and v1** — MVP uses `Session` as the core model; Kernel and the v1 spec use `AgentInstance`. There is no migration strategy.
6. **No event persistence** — `ExecutionEngine` emits `ExecutionEvent` vectors in memory, but there is no table or repository to persist them.

### 1.2 Target Outcome

- Every execution flow goes through `InMemoryKernelRuntime`.
- HTTP handlers are thin adapters (<30 lines each).
- Business logic lives in a `service/` layer.
- DB access is isolated behind `repository/` traits.
- Events are written to `v1_events` on every step.
- Checkpoints are persisted via `checkpointer::Checkpointer`.
- MVP `/sessions` API continues to work during migration.

### 1.3 Core Principles

1. **HTTP handlers are adapters only** — serialization, auth, routing, and SSE wrapping. No direct DB or LLM access.
2. **Business logic belongs in services** — `SessionService`, `AgentInstanceService`, etc. are the primary testable units.
3. **Kernel runtime is the single source of execution truth** — instance state, task state, approvals, and delegations must advance through `KernelRuntime`.
4. **Repository pattern isolates persistence** — sqlx calls exist only in `repository/` implementations.
5. **Events are first-class persisted citizens** — every `ExecutionEngine::step` writes to `v1_events`.
6. **MVP APIs remain functional during migration** — existing `/sessions` and `/chat` keep working while their internals are switched to the new runtime.

---

## 2. Service Internal Layering

The `agent-runtime-service/src` tree is reorganized into five layers with strict downward dependencies:

```
api/           → HTTP adapters; may call service and repository
service/       → Business orchestration; may call repository and kernel-bridge
kernel-bridge/ → Runtime bridge between kernel and Postgres
repository/    → Persistence abstraction; may call db/ and models/
infra/         → LLM client, ToolRegistry, stream utilities
```

### 2.1 Layer Responsibilities

#### `api/`
- `mod.rs` — router assembly
- `sessions.rs`, `messages.rs`, `memory.rs` — thinned handlers that delegate to services
- `middleware.rs`, `metrics.rs` — unchanged
- `v1/` — v1 Platform API routes (parallel to MVP, populated incrementally)

#### `service/`
- `mod.rs` — `ServiceContainer` (DI container)
- `session.rs` — `SessionService`: session lifecycle, chat orchestration, status gating
- `agent_instance.rs` — `AgentInstanceService`: v1 instance management
- `memory.rs` — `MemoryService`: nomination, approval, search
- `tool.rs` — `ToolService`: singleton `ToolRegistry` lifecycle

#### `kernel-bridge/`
- `runtime.rs` — `KernelRuntimeHandle`: wraps `InMemoryKernelRuntime` + Postgres persistence
- `mapping.rs` — `session_to_execution_request` and other model mappings
- `events.rs` — `EventRecorder`: converts `ExecutionEvent` to DB `Event`
- `checkpointer.rs` — `PostgresCheckpointer`: implements `checkpointer::Checkpointer`

#### `repository/`
- `mod.rs` — trait definitions and `RepositoryContainer`
- `session.rs`, `message.rs`, `memory.rs`, `event.rs`, `checkpoint.rs` — async traits + Postgres implementations

#### `infra/`
- `llm.rs` — LLM client wrappers
- `tool_registry.rs` — moved from `tools/registry.rs`
- `stream.rs` — SSE channel utilities

### 2.2 Dependency Rules

```rust
// api may:
use crate::service::{SessionService, ServiceContainer};
use crate::repository::{SessionRepository, MessageRepository};

// service may:
use crate::kernel_bridge::{KernelRuntimeHandle, session_to_execution_request};
use crate::repository::{SessionRepository, MessageRepository};
use crate::infra::{ToolRegistry, LlmClient};

// kernel-bridge may:
use crate::repository::EventRepository;
use torque_kernel::{KernelRuntime, ExecutionEngine};
use checkpointer::{Checkpointer, CheckpointMeta};

// repository may:
use crate::db::Database;
use crate::models::{Session, Message};

// Forbidden:
// api calling db::xxx directly
// service writing raw sqlx queries
// kernel-bridge emitting HTTP responses
```

### 2.3 Fate of `AgentRunner`

`AgentRunner::run` (229 lines) is split across:
- `SessionService::chat()` — high-level orchestration
- `KernelRuntimeHandle::execute_chat()` — kernel state machine advancement
- `infra/tool_registry.rs` — tool execution infrastructure

---

## 3. Kernel Bridge and Runtime Integration

### 3.1 Current Flow (Broken)

```rust
let kernel_turn = build_kernel_turn(session, user_message)?;
// kernel_turn is only used for tracing
let history = db::messages::get_recent(...).await?;
llm.chat_streaming(...).await?  // bypasses kernel entirely
```

### 3.2 Target Flow

All execution goes through `KernelRuntimeHandle`, which wraps `InMemoryKernelRuntime` and persists state changes after each step.

### 3.3 `KernelRuntimeHandle`

```rust
pub struct KernelRuntimeHandle {
    runtime: InMemoryKernelRuntime,
    event_repo: Arc<dyn EventRepository>,
    checkpoint_repo: Arc<dyn CheckpointRepository>,
    checkpointer: Arc<dyn Checkpointer>,
}

impl KernelRuntimeHandle {
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
}
```

### 3.4 Mapping Layer

`mapping.rs` is refactored to return an `ExecutionRequest` instead of constructing and discarding a full `KernelTurn`:

```rust
pub fn session_to_execution_request(
    session: &Session,
    user_message: &str,
) -> Result<ExecutionRequest, KernelError> {
    let agent_def = AgentDefinition::new(
        &session.agent_definition_id.to_string(),
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

### 3.5 State Machine Advancement

When `AgentInstanceState` is `Ready` or `Running`, the bridge calls the LLM and translates the response into a `StepDecision`:

```rust
async fn resolve_decision_to_step(
    &self,
    state: AgentInstanceState,
    llm: Arc<dyn LlmClient>,
    tools: Arc<ToolRegistry>,
    messages: Vec<LlmMessage>,
) -> Result<StepDecision, KernelBridgeError> {
    match state {
        AgentInstanceState::Ready | AgentInstanceState::Running => {
            llm_to_step_decision(llm, tools, messages).await
        }
        _ => Ok(StepDecision::Continue),
    }
}
```

When the kernel returns `AwaitTool`, the bridge executes the tool and resumes with `ResumeSignal::ToolCompleted`. The same pattern applies to `AwaitApproval` and `AwaitDelegation`.

### 3.6 Hydration Strategy (Short-Term)

`InMemoryKernelRuntime` is in-memory, but instances must survive across requests. The short-term strategy:

1. On each chat request, load the instance’s simplified state from `SessionRepository::get_kernel_state`.
2. Reconstruct `AgentInstance` and inject it into `InMemoryRuntimeStore`.
3. The long-term goal is `PostgresRuntimeStore` implementing `RuntimeStore`; this is documented as a future task.

```rust
async fn hydrate_runtime(
    &mut self,
    instance_id: AgentInstanceId,
    session_repo: &dyn SessionRepository,
) -> Result<(), KernelBridgeError> {
    let state = session_repo.get_kernel_state(instance_id.as_uuid()).await?;
    // reconstruct AgentInstance into runtime store
    Ok(())
}
```

---

## 4. Repository and Service Boundaries

### 4.1 Repository Traits

Each domain gets an async trait. Existing `db/` modules become the private implementation of `PostgresXxxRepository`.

```rust
#[async_trait]
pub trait SessionRepository: Send + Sync {
    async fn create(&self, api_key: &str, project_scope: &str) -> anyhow::Result<Session>;
    async fn list(&self, limit: i64) -> anyhow::Result<Vec<Session>>;
    async fn get_by_id(&self, id: Uuid) -> anyhow::Result<Option<Session>>;
    async fn update_status(&self, id: Uuid, status: SessionStatus, error_msg: Option<&str>) -> anyhow::Result<bool>;
    async fn try_mark_running(&self, id: Uuid) -> anyhow::Result<bool>;
    async fn get_kernel_state(&self, id: Uuid) -> anyhow::Result<Option<SessionKernelState>>;
}

pub struct SessionKernelState {
    pub agent_definition_id: Uuid,
    pub status: String,
    pub active_task_id: Option<Uuid>,
    pub checkpoint_id: Option<Uuid>,
}
```

### 4.2 `ServiceContainer`

```rust
pub struct ServiceContainer {
    pub session: Arc<SessionService>,
    pub memory: Arc<MemoryService>,
    pub tool: Arc<ToolService>,
    pub agent_instance: Arc<AgentInstanceService>,
}

impl ServiceContainer {
    pub async fn new(
        repos: RepositoryContainer,
        kernel: KernelRuntimeHandle,
        llm: Arc<dyn LlmClient>,
    ) -> Self {
        let tool = Arc::new(ToolService::new().await);
        let memory = Arc::new(MemoryService::new(repos.memory.clone()));
        let session = Arc::new(SessionService::new(
            repos.session.clone(),
            repos.message.clone(),
            kernel,
            llm,
            tool.clone(),
            memory.clone(),
        ));
        let agent_instance = Arc::new(AgentInstanceService::new(
            repos.agent_definition.clone(),
            kernel,
        ));

        Self { session, memory, tool, agent_instance }
    }
}
```

### 4.3 `SessionService`

```rust
pub struct SessionService {
    session_repo: Arc<dyn SessionRepository>,
    message_repo: Arc<dyn MessageRepository>,
    kernel: Mutex<KernelRuntimeHandle>,
    llm: Arc<dyn LlmClient>,
    tools: Arc<ToolService>,
    memory: Arc<MemoryService>,
}

impl SessionService {
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

        let request = crate::kernel_bridge::session_to_execution_request(&session, &message)?;

        let mut kernel = self.kernel.lock().await;
        let result = kernel.execute_chat(
            request,
            self.llm.clone(),
            self.tools.registry().clone(),
            event_sink.clone(),
        ).await;

        match result {
            Ok(exec) => {
                let msg = Message::assistant(session_id, exec.summary.unwrap_or_default(), None, None);
                self.message_repo.create(&msg).await?;
                self.session_repo.update_status(session_id, SessionStatus::Completed, None).await?;
            }
            Err(e) => {
                self.session_repo.update_status(session_id, SessionStatus::Error, Some(&e.to_string())).await?;
                let _ = event_sink.send(StreamEvent::Error {
                    code: "AGENT_ERROR".to_string(),
                    message: e.to_string(),
                }).await;
            }
        }

        Ok(())
    }
}
```

### 4.4 Thinned Handler

```rust
pub async fn chat(
    State(services): State<Arc<ServiceContainer>>,
    Path(session_id): Path<Uuid>,
    Extension(api_key): Extension<String>,
    Json(req): Json<ChatRequest>,
) -> Result<Response, StatusCode> {
    let (tx, rx) = mpsc::channel::<StreamEvent>(100);

    let session_svc = services.session.clone();
    tokio::spawn(async move {
        let _ = session_svc.chat(session_id, &api_key, req.message, tx).await;
    });

    let stream = ReceiverStream::new(rx)
        .map(|e| Ok::<_, Infallible>(e.to_sse()));

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/event-stream")
        .body(Body::from_stream(stream))
        .unwrap())
}
```

### 4.5 Testing Benefits

- `SessionRepository` can be mocked for service tests without a database.
- `LlmClient` can be mocked to test chat orchestration.
- Handlers only need route/serialization tests.

---

## 5. Checkpointer and Event Persistence Integration

### 5.1 PostgresCheckpointer

Implements `checkpointer::Checkpointer` and maps `torque-kernel::Checkpoint` to the trait’s types.

```rust
pub struct PostgresCheckpointer {
    db: crate::db::Database,
}

#[async_trait::async_trait]
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
        .bind(serde_json::to_value(&state)?)
        .execute(self.db.pool())
        .await
        .map_err(|e| checkpointer::CheckpointerError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn load(&self, id: CheckpointId) -> checkpointer::Result<Option<CheckpointState>> {
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

### 5.2 Event Table and Repository

```sql
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

#[async_trait]
pub trait EventRepository: Send + Sync {
    async fn create(&self, event: Event) -> anyhow::Result<()>;
    async fn create_batch(&self, events: Vec<Event>) -> anyhow::Result<()>;
    async fn list_by_resource(
        &self,
        resource_type: &str,
        resource_id: Uuid,
        limit: i64,
    ) -> anyhow::Result<Vec<Event>>;
    async fn list_by_types(
        &self,
        resource_type: &str,
        resource_id: Uuid,
        event_types: &[String],
        limit: i64,
    ) -> anyhow::Result<Vec<Event>>;
}
```

### 5.3 EventRecorder Mapping

```rust
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
```

### 5.4 Recovery Flow

When an instance is loaded for a new request:

1. Read the latest checkpoint ID from `CheckpointRepository`.
2. Load the checkpoint state via `PostgresCheckpointer::load`.
3. Read tail events from `EventRepository::list_by_resource`.
4. Reconstruct `AgentInstance` in `InMemoryRuntimeStore`.

```rust
async fn restore_instance(
    &mut self,
    instance_id: AgentInstanceId,
) -> Result<(), KernelBridgeError> {
    let cp_id = self.checkpoint_repo
        .latest_checkpoint_id(instance_id.as_uuid())
        .await?
        .ok_or_else(|| KernelBridgeError::NoCheckpoint(instance_id))?;

    let cp_state = self.checkpointer
        .load(cp_id.into())
        .await?
        .ok_or_else(|| KernelBridgeError::CheckpointNotFound(cp_id))?;

    let _tail_events = self.event_repo
        .list_by_resource("agent_instance", instance_id.as_uuid(), 1000)
        .await?;

    self.runtime = self.runtime.with_restored_instance(instance_id, cp_state)?;
    Ok(())
}
```

### 5.5 Dual-Write During Migration

MVP `sessions` status and v1 events are both written during the transition:
- `SessionService::chat` updates `sessions.status` (MVP compatibility).
- `KernelRuntimeHandle` writes `v1_events` (v1 readiness).
- After v1 fully replaces MVP, the `sessions` table can be deprecated.

---

## 6. Migration Path and File Structure

### 6.1 Final File Structure

```
crates/agent-runtime-service/src/
├── main.rs
├── app.rs
├── lib.rs
├── api/
│   ├── mod.rs
│   ├── middleware.rs
│   ├── metrics.rs
│   ├── sessions.rs
│   ├── messages.rs
│   ├── memory.rs
│   └── v1/
│       ├── mod.rs
│       ├── agent_definitions.rs
│       ├── agent_instances.rs
│       ├── runs.rs
│       └── ...
├── service/
│   ├── mod.rs
│   ├── session.rs
│   ├── agent_instance.rs
│   ├── memory.rs
│   └── tool.rs
├── repository/
│   ├── mod.rs
│   ├── session.rs
│   ├── message.rs
│   ├── memory.rs
│   ├── event.rs
│   └── checkpoint.rs
├── kernel_bridge/
│   ├── mod.rs
│   ├── runtime.rs
│   ├── mapping.rs
│   ├── events.rs
│   └── checkpointer.rs
├── infra/
│   ├── mod.rs
│   ├── llm.rs
│   ├── tool_registry.rs
│   └── stream.rs
├── db/
│   ├── mod.rs
│   ├── sessions.rs      # migrates into repository/session.rs, then removed
│   ├── messages.rs
│   ├── memory_entries.rs
│   ├── memory_candidates.rs
│   └── ...
├── models/
│   ├── mod.rs
│   ├── session.rs
│   ├── message.rs
│   ├── memory.rs
│   └── v1/
│       ├── mod.rs
│       ├── common.rs
│       ├── event.rs
│       └── ...
├── agent/
│   ├── mod.rs
│   ├── context.rs
│   └── stream.rs
├── tools/
│   ├── mod.rs
│   ├── builtin.rs
│   └── registry.rs      # logic moves to infra/tool_registry.rs
├── metrics.rs
└── v1_guards.rs
```

### 6.2 Migration Tasks

| Task | Description | Verification |
|---|---|---|
| **T1** | Create `repository/` and migrate `sessions` + `messages` | Handlers no longer call `db::sessions::` directly |
| **T2** | Create `service/` and extract `SessionService` | `messages::chat` delegates to `session_svc.chat()` |
| **T3** | Create `kernel-bridge/`, refactor `mapping.rs`, implement `KernelRuntimeHandle` | `AgentRunner` removed; all execution uses `KernelRuntimeHandle` |
| **T4** | Implement `EventRepository` + `EventRecorder`, dual-write events | `v1_events` has data; `GET /v1/events` works |
| **T5** | Implement `PostgresCheckpointer`, bridge `checkpointer` crate | `checkpointer` is no longer unused |
| **T6** | Move `tools/registry.rs` to `infra/tool_registry.rs`, `ToolService` singleton | `ToolRegistry` is no longer rebuilt per request |

### 6.3 Key Breaking Changes

- `AgentRunner` is deleted; its logic is distributed across `SessionService`, `KernelRuntimeHandle`, and `infra/tool_registry.rs`.
- `kernel/mapping.rs` becomes `kernel-bridge/mapping.rs` and only produces `ExecutionRequest`.
- `db/*` modules are transitional; they are absorbed into `repository/` implementations.
- `v1_events` table serves both MVP and v1 to avoid duplicate schemas.

---

## 7. Success Criteria

1. `cargo check -p agent-runtime-service` passes after each task.
2. MVP `/sessions` and `/chat` endpoints continue to function.
3. `v1_events` is populated on every chat request.
4. `checkpointer` crate is referenced in `agent-runtime-service` source.
5. Handlers are under 30 lines; services are independently unit-testable.
6. `torque-kernel` objects are actually advanced by runtime code.
