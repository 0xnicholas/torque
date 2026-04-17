# Torque Kernel Execution Engine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the v1 AgentInstance execution engine so that `/v1/agent-instances/{id}/runs` streams real tool-augmented LLM execution, with proper Task lifecycle management and Event recording.

**Architecture:** Reuse the existing `SessionService::chat` execution loop (kernel bridge + LLM + tools + SSE), but refactor it into a generic `AgentInstanceService::execute` method that drives v1 runs. Add Task state machine transitions and rich event recording.

**Tech Stack:** Rust, axum, sqlx, async-trait, torque-kernel crate, llm crate, SSE streaming

---

## File Structure

| File | Responsibility |
|------|---------------|
| `src/models/v1/task.rs` | Extend Task model with status enum and transition rules |
| `src/repository/task.rs` | Add `update_status`, `create_task`, `get_with_instance` methods |
| `src/repository/agent_instance.rs` | Add `update_status`, `update_current_task` methods |
| `src/service/task.rs` | Add Task state machine and lifecycle helpers |
| `src/service/agent_instance.rs` | Add `execute` method wrapping kernel bridge |
| `src/service/run.rs` | New file: orchestrate run from request to SSE stream |
| `src/kernel_bridge/v1_mapping.rs` | New file: map v1 RunRequest to kernel ExecutionRequest |
| `src/api/v1/runs.rs` | Wire real execution into the SSE endpoint |
| `tests/v1_execution_tests.rs` | Integration tests for run execution |

---

## Prerequisites

Before starting, verify the codebase state:

```bash
cargo check -p agent-runtime-service
cargo test -p agent-runtime-service
```

Both should pass. The current `runs::run` handler returns a dummy SSE stream.

---

## Task 1: Extend Task Model with Status Enum

**Files:**
- Modify: `crates/agent-runtime-service/src/models/v1/task.rs`

**Context:** The current `Task` model stores `status` as a plain `String`. We need a typed enum with valid transitions.

- [ ] **Step 1: Add TaskStatus enum**

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, sqlx::Type, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[sqlx(rename_all = "snake_case")]
pub enum TaskStatus {
    Created,
    Queued,
    Running,
    WaitingTool,
    WaitingSubagent,
    WaitingApproval,
    Completed,
    Failed,
    Cancelled,
}

impl TaskStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled)
    }

    pub fn can_transition_to(&self, next: &TaskStatus) -> bool {
        match (self, next) {
            (TaskStatus::Created, TaskStatus::Queued) => true,
            (TaskStatus::Queued, TaskStatus::Running) => true,
            (TaskStatus::Running, TaskStatus::WaitingTool) => true,
            (TaskStatus::Running, TaskStatus::WaitingSubagent) => true,
            (TaskStatus::Running, TaskStatus::WaitingApproval) => true,
            (TaskStatus::Running, TaskStatus::Completed) => true,
            (TaskStatus::Running, TaskStatus::Failed) => true,
            (TaskStatus::WaitingTool, TaskStatus::Running) => true,
            (TaskStatus::WaitingTool, TaskStatus::Failed) => true,
            (TaskStatus::WaitingSubagent, TaskStatus::Running) => true,
            (TaskStatus::WaitingApproval, TaskStatus::Running) => true,
            (TaskStatus::WaitingApproval, TaskStatus::Failed) => true,
            (TaskStatus::Queued, TaskStatus::Cancelled) => true,
            (TaskStatus::Running, TaskStatus::Cancelled) => true,
            (s, t) if s == t => true, // same state is idempotent
            _ => false,
        }
    }
}
```

- [ ] **Step 2: Update Task struct to use TaskStatus**

```rust
#[derive(Debug, Serialize, FromRow)]
pub struct Task {
    pub id: Uuid,
    pub task_type: TaskType,
    pub parent_task_id: Option<Uuid>,
    pub agent_instance_id: Option<Uuid>,
    pub team_instance_id: Option<Uuid>,
    pub status: TaskStatus,
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

- [ ] **Step 3: Commit**

```bash
git add crates/agent-runtime-service/src/models/v1/task.rs
git commit -m "feat(task): add TaskStatus enum with valid transitions"
```

---

## Task 2: Extend Task Repository with State Management

**Files:**
- Modify: `crates/agent-runtime-service/src/repository/task.rs`

**Context:** Current TaskRepository only supports `list`, `get`, `cancel`. We need creation and state updates.

- [ ] **Step 1: Extend TaskRepository trait**

```rust
#[async_trait]
pub trait TaskRepository: Send + Sync {
    async fn create(
        &self,
        task_type: TaskType,
        goal: &str,
        instructions: Option<&str>,
        agent_instance_id: Option<Uuid>,
        input_artifacts: serde_json::Value,
    ) -> anyhow::Result<Task>;

    async fn list(&self, limit: i64) -> anyhow::Result<Vec<Task>>;
    async fn get(&self, id: Uuid) -> anyhow::Result<Option<Task>>;
    async fn update_status(&self, id: Uuid, status: TaskStatus) -> anyhow::Result<bool>;
    async fn cancel(&self, id: Uuid) -> anyhow::Result<bool>;
    async fn update_produced_artifacts(
        &self,
        id: Uuid,
        artifacts: serde_json::Value,
    ) -> anyhow::Result<bool>;
}
```

- [ ] **Step 2: Implement create method**

```rust
async fn create(
    &self,
    task_type: TaskType,
    goal: &str,
    instructions: Option<&str>,
    agent_instance_id: Option<Uuid>,
    input_artifacts: serde_json::Value,
) -> anyhow::Result<Task> {
    let row = sqlx::query_as::<_, Task>(
        "INSERT INTO v1_tasks (task_type, status, goal, instructions, agent_instance_id, input_artifacts) VALUES ($1, $2, $3, $4, $5, $6) RETURNING *"
    )
    .bind(task_type)
    .bind(TaskStatus::Created)
    .bind(goal)
    .bind(instructions)
    .bind(agent_instance_id)
    .bind(input_artifacts)
    .fetch_one(self.db.pool())
    .await?;
    Ok(row)
}
```

- [ ] **Step 3: Implement update_status**

```rust
async fn update_status(&self, id: Uuid, status: TaskStatus) -> anyhow::Result<bool> {
    let result = sqlx::query(
        "UPDATE v1_tasks SET status = $1, updated_at = NOW() WHERE id = $2"
    )
    .bind(status)
    .bind(id)
    .execute(self.db.pool())
    .await?;
    Ok(result.rows_affected() > 0)
}
```

- [ ] **Step 4: Implement update_produced_artifacts**

```rust
async fn update_produced_artifacts(
    &self,
    id: Uuid,
    artifacts: serde_json::Value,
) -> anyhow::Result<bool> {
    let result = sqlx::query(
        "UPDATE v1_tasks SET produced_artifacts = $1, updated_at = NOW() WHERE id = $2"
    )
    .bind(artifacts)
    .bind(id)
    .execute(self.db.pool())
    .await?;
    Ok(result.rows_affected() > 0)
}
```

- [ ] **Step 5: Update cancel to use TaskStatus**

```rust
async fn cancel(&self, id: Uuid) -> anyhow::Result<bool> {
    self.update_status(id, TaskStatus::Cancelled).await
}
```

- [ ] **Step 6: Run tests**

```bash
cargo check -p agent-runtime-service
cargo test -p agent-runtime-service
```

Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/agent-runtime-service/src/repository/task.rs
git commit -m "feat(task): add create, update_status, update_produced_artifacts to TaskRepository"
```

---

## Task 3: Extend AgentInstance Repository with Execution State

**Files:**
- Modify: `crates/agent-runtime-service/src/repository/agent_instance.rs`

**Context:** Need to track current task and status transitions during execution.

- [ ] **Step 1: Extend trait**

```rust
#[async_trait]
pub trait AgentInstanceRepository: Send + Sync {
    async fn create(&self, req: &AgentInstanceCreate) -> anyhow::Result<AgentInstance>;
    async fn list(&self, limit: i64) -> anyhow::Result<Vec<AgentInstance>>;
    async fn get(&self, id: Uuid) -> anyhow::Result<Option<AgentInstance>>;
    async fn delete(&self, id: Uuid) -> anyhow::Result<bool>;
    async fn update_status(&self, id: Uuid, status: AgentInstanceStatus) -> anyhow::Result<bool>;
    async fn update_current_task(&self, id: Uuid, task_id: Option<Uuid>) -> anyhow::Result<bool>;
}
```

- [ ] **Step 2: Implement update_current_task**

```rust
async fn update_current_task(&self, id: Uuid, task_id: Option<Uuid>) -> anyhow::Result<bool> {
    let result = sqlx::query(
        "UPDATE v1_agent_instances SET current_task_id = $1, updated_at = NOW() WHERE id = $2"
    )
    .bind(task_id)
    .bind(id)
    .execute(self.db.pool())
    .await?;
    Ok(result.rows_affected() > 0)
}
```

- [ ] **Step 3: Commit**

```bash
git add crates/agent-runtime-service/src/repository/agent_instance.rs
git commit -m "feat(agent-instance): add update_current_task to repository"
```

---

## Task 4: Create v1 Execution Mapping

**Files:**
- Create: `crates/agent-runtime-service/src/kernel_bridge/v1_mapping.rs`
- Modify: `crates/agent-runtime-service/src/kernel_bridge/mod.rs`

**Context:** Bridge v1 RunRequest to torque-kernel ExecutionRequest.

- [ ] **Step 1: Create v1_mapping.rs**

```rust
use crate::models::v1::agent_definition::AgentDefinition;
use crate::models::v1::run::RunRequest;
use torque_kernel::{ExecutionMode, ExecutionRequest};
use uuid::Uuid;

pub fn run_request_to_execution_request(
    agent_definition: &AgentDefinition,
    run_request: &RunRequest,
) -> ExecutionRequest {
    let mode = match run_request.execution_mode.as_str() {
        "async" => ExecutionMode::Async,
        _ => ExecutionMode::Sync,
    };

    let mut request = ExecutionRequest::new(
        agent_definition.id,
        run_request.goal.clone(),
        run_request.external_context_refs.iter().map(|v| v.to_string()).collect(),
    )
    .with_execution_mode(mode);

    if let Some(instructions) = &run_request.instructions {
        request = request.with_instructions(instructions.clone());
    }

    request
}
```

- [ ] **Step 2: Export from mod.rs**

Add to `src/kernel_bridge/mod.rs`:

```rust
pub mod v1_mapping;
pub use v1_mapping::run_request_to_execution_request;
```

- [ ] **Step 3: Commit**

```bash
git add crates/agent-runtime-service/src/kernel_bridge/v1_mapping.rs crates/agent-runtime-service/src/kernel_bridge/mod.rs
git commit -m "feat(kernel-bridge): add v1 RunRequest to ExecutionRequest mapping"
```

---

## Task 5: Create Run Service

**Files:**
- Create: `crates/agent-runtime-service/src/service/run.rs`

**Context:** Orchestrate a run: create Task, update AgentInstance status, execute, record events.

- [ ] **Step 1: Create the RunService**

```rust
use crate::agent::stream::StreamEvent;
use crate::infra::llm::LlmClient;
use crate::kernel_bridge::{run_request_to_execution_request, KernelRuntimeHandle};
use crate::models::v1::agent_instance::AgentInstanceStatus;
use crate::models::v1::run::RunRequest;
use crate::models::v1::task::{TaskStatus, TaskType};
use crate::repository::{
    AgentDefinitionRepository, AgentInstanceRepository, TaskRepository,
    EventRepository, CheckpointRepository,
};
use crate::service::{AgentInstanceService, ToolService};
use crate::tools::ToolRegistry;
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

pub struct RunService {
    agent_definition_repo: Arc<dyn AgentDefinitionRepository>,
    agent_instance_repo: Arc<dyn AgentInstanceRepository>,
    task_repo: Arc<dyn TaskRepository>,
    event_repo: Arc<dyn EventRepository>,
    checkpoint_repo: Arc<dyn CheckpointRepository>,
    checkpointer: Arc<dyn checkpointer::Checkpointer>,
    llm: Arc<dyn LlmClient>,
    tools: Arc<ToolService>,
}

impl RunService {
    pub fn new(
        agent_definition_repo: Arc<dyn AgentDefinitionRepository>,
        agent_instance_repo: Arc<dyn AgentInstanceRepository>,
        task_repo: Arc<dyn TaskRepository>,
        event_repo: Arc<dyn EventRepository>,
        checkpoint_repo: Arc<dyn CheckpointRepository>,
        checkpointer: Arc<dyn checkpointer::Checkpointer>,
        llm: Arc<dyn LlmClient>,
        tools: Arc<ToolService>,
    ) -> Self {
        Self {
            agent_definition_repo,
            agent_instance_repo,
            task_repo,
            event_repo,
            checkpoint_repo,
            checkpointer,
            llm,
            tools,
        }
    }

    pub async fn execute(
        &self,
        instance_id: Uuid,
        request: RunRequest,
        event_sink: mpsc::Sender<StreamEvent>,
    ) -> anyhow::Result<()> {
        // 1. Fetch instance and definition
        let instance = self.agent_instance_repo.get(instance_id).await?
            .ok_or_else(|| anyhow::anyhow!("Agent instance not found: {}", instance_id))?;

        let definition = self.agent_definition_repo.get(instance.agent_definition_id).await?
            .ok_or_else(|| anyhow::anyhow!("Agent definition not found: {}", instance.agent_definition_id))?;

        // 2. Update instance status to Running
        self.agent_instance_repo.update_status(instance_id, AgentInstanceStatus::Running).await?;

        // 3. Create task
        let task = self.task_repo.create(
            TaskType::AgentTask,
            &request.goal,
            request.instructions.as_deref(),
            Some(instance_id),
            serde_json::to_value(&request.input_artifacts)?,
        ).await?;

        // 4. Link task to instance
        self.agent_instance_repo.update_current_task(instance_id, Some(task.id)).await?;
        self.task_repo.update_status(task.id, TaskStatus::Running).await?;

        // 5. Build execution request
        let execution_request = run_request_to_execution_request(&definition, &request);

        // 6. Execute via kernel bridge
        let result = self.run_execution(
            instance_id,
            execution_request,
            event_sink.clone(),
        ).await;

        // 7. Update task status based on result
        let final_status = match &result {
            Ok(_) => TaskStatus::Completed,
            Err(_) => TaskStatus::Failed,
        };
        self.task_repo.update_status(task.id, final_status).await?;

        // 8. Update instance status
        self.agent_instance_repo.update_current_task(instance_id, None).await?;
        self.agent_instance_repo.update_status(
            instance_id,
            if result.is_ok() { AgentInstanceStatus::Ready } else { AgentInstanceStatus::Failed }
        ).await?;

        // 9. Send terminal event
        match result {
            Ok(content) => {
                let _ = event_sink.send(StreamEvent::Done {
                    message_id: task.id,
                    artifacts: None,
                }).await;
            }
            Err(e) => {
                let _ = event_sink.send(StreamEvent::Error {
                    code: "EXECUTION_ERROR".into(),
                    message: e.to_string(),
                }).await;
            }
        }

        result
    }

    async fn run_execution(
        &self,
        instance_id: Uuid,
        request: torque_kernel::ExecutionRequest,
        event_sink: mpsc::Sender<StreamEvent>,
    ) -> anyhow::Result<String> {
        let mut kernel = KernelRuntimeHandle::new(
            vec![],
            self.event_repo.clone(),
            self.checkpoint_repo.clone(),
            self.checkpointer.clone(),
        );

        // Use existing execute_chat logic but adapted for v1
        // This will be refactored in Task 7 to be shared
        kernel.execute_chat(
            request,
            self.llm.clone(),
            self.tools.registry.clone(),
            event_sink,
            vec![], // Start with empty messages for v1
        ).await
        .map(|result| result.summary.unwrap_or_default())
        .map_err(|e| anyhow::anyhow!("Kernel execution failed: {}", e))
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/agent-runtime-service/src/service/run.rs
git commit -m "feat(run): create RunService orchestrating task lifecycle and execution"
```

---

## Task 6: Wire RunService into ServiceContainer

**Files:**
- Modify: `crates/agent-runtime-service/src/service/mod.rs`

- [ ] **Step 1: Add run module to mod.rs**

```rust
pub mod run;
pub use run::RunService;
```

- [ ] **Step 2: Add run field to ServiceContainer**

```rust
pub struct ServiceContainer {
    // ... existing fields ...
    pub run: std::sync::Arc<RunService>,
}
```

- [ ] **Step 3: Construct RunService in ServiceContainer::new**

```rust
let run = std::sync::Arc::new(RunService::new(
    repos.agent_definition.clone(),
    repos.agent_instance.clone(),
    repos.task.clone(),
    repos.event.clone(),
    repos.checkpoint.clone(),
    checkpointer.clone(),
    llm.clone(),
    tool.clone(),
));

// ... in Self constructor, add: run
```

- [ ] **Step 4: Commit**

```bash
git add crates/agent-runtime-service/src/service/mod.rs
git commit -m "feat(service): wire RunService into ServiceContainer"
```

---

## Task 7: Implement Real v1 Runs Handler

**Files:**
- Modify: `crates/agent-runtime-service/src/api/v1/runs.rs`

**Context:** Replace dummy SSE with real execution.

- [ ] **Step 1: Rewrite runs.rs**

```rust
use axum::{
    extract::{Path, State},
    response::sse::{Event, Sse},
    Json,
};
use crate::agent::stream::StreamEvent;
use crate::db::Database;
use crate::models::v1::run::RunRequest;
use crate::service::ServiceContainer;
use llm::OpenAiClient;
use std::sync::Arc;
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

pub async fn run(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
    Json(req): Json<RunRequest>,
) -> Sse<ReceiverStream<Result<Event, axum::Error>>> {
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, axum::Error>>(32);
    let stream_tx = tx.clone();
    let run_service = services.run.clone();

    tokio::spawn(async move {
        let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<StreamEvent>(32);

        // Spawn execution
        let run_handle = tokio::spawn(async move {
            run_service.execute(id, req, event_tx).await
        });

        // Forward StreamEvents to SSE
        while let Some(event) = event_rx.recv().await {
            let sse_event = Event::default().event(event.event_name())
                .json_data(&event)
                .unwrap_or_else(|_| Event::default().event("error").data("serialization error"));
            
            if stream_tx.send(Ok(sse_event)).await.is_err() {
                break; // Client disconnected
            }
        }

        // Wait for completion
        let _ = run_handle.await;
    });

    Sse::new(ReceiverStream::new(rx))
}
```

- [ ] **Step 2: Add event_name helper to StreamEvent**

Modify `src/agent/stream.rs`:

```rust
impl StreamEvent {
    pub fn event_name(&self) -> &'static str {
        match self {
            StreamEvent::Start { .. } => "start",
            StreamEvent::Chunk { .. } => "chunk",
            StreamEvent::ToolCall { .. } => "tool_call",
            StreamEvent::ToolResult { .. } => "tool_result",
            StreamEvent::Done { .. } => "done",
            StreamEvent::Error { .. } => "error",
        }
    }
}
```

- [ ] **Step 3: Commit**

```bash
git add crates/agent-runtime-service/src/api/v1/runs.rs crates/agent-runtime-service/src/agent/stream.rs
git commit -m "feat(runs): implement real execution via RunService with SSE streaming"
```

---

## Task 8: Refactor KernelRuntimeHandle for Shared Use

**Files:**
- Modify: `crates/agent-runtime-service/src/kernel_bridge/runtime.rs`

**Context:** The current `execute_chat` is tightly coupled to Session. Refactor to accept AgentDefinition directly.

- [ ] **Step 1: Extract execute method that works with v1**

```rust
impl KernelRuntimeHandle {
    pub async fn execute_v1(
        &mut self,
        request: ExecutionRequest,
        llm: Arc<dyn LlmClient>,
        tools: Arc<ToolRegistry>,
        event_sink: mpsc::Sender<StreamEvent>,
    ) -> Result<ExecutionResult, KernelBridgeError> {
        let result = self.runtime.handle(request, StepDecision::Continue)?;
        self.record_events(&result).await?;

        let instance_id = result.instance_id;
        let _ = event_sink.send(StreamEvent::Start {
            session_id: instance_id.as_uuid(),
        }).await;

        let final_content = self
            .run_llm_conversation(llm, tools, event_sink.clone(), vec![])
            .await?;

        let complete_request = self.reconstruct_request(instance_id)?;
        let result = self
            .runtime
            .handle(complete_request, StepDecision::CompleteTask(final_content.clone()))?;
        self.record_events(&result).await?;

        self.create_checkpoint(instance_id).await?;

        let mut result = result;
        result.summary = Some(final_content);

        Ok(result)
    }
}
```

- [ ] **Step 2: Update RunService to use execute_v1**

In `src/service/run.rs`, change `run_execution` to call `execute_v1` instead of `execute_chat`.

- [ ] **Step 3: Commit**

```bash
git add crates/agent-runtime-service/src/kernel_bridge/runtime.rs crates/agent-runtime-service/src/service/run.rs
git commit -m "refactor(kernel): extract execute_v1 for shared use between session and v1"
```

---

## Task 9: Add Run Execution Integration Tests

**Files:**
- Create: `crates/agent-runtime-service/tests/v1_execution_tests.rs`

- [ ] **Step 1: Create test file**

```rust
mod common;

use common::setup_test_db_or_skip;
use serial_test::serial;
use agent_runtime_service::repository::{
    AgentDefinitionRepository, PostgresAgentDefinitionRepository,
    AgentInstanceRepository, PostgresAgentInstanceRepository,
    TaskRepository, PostgresTaskRepository,
};
use agent_runtime_service::models::v1::agent_definition::AgentDefinitionCreate;
use agent_runtime_service::models::v1::agent_instance::AgentInstanceCreate;
use agent_runtime_service::models::v1::run::RunRequest;
use agent_runtime_service::models::v1::task::TaskStatus;
use agent_runtime_service::service::RunService;
use std::sync::Arc;
use tokio::sync::mpsc;

#[tokio::test]
#[serial]
async fn test_run_creates_task_and_updates_status() {
    let Some(db) = setup_test_db_or_skip().await else {
        return;
    };

    let def_repo = Arc::new(PostgresAgentDefinitionRepository::new(db.clone()));
    let inst_repo = Arc::new(PostgresAgentInstanceRepository::new(db.clone()));
    let task_repo = Arc::new(PostgresTaskRepository::new(db.clone()));
    // ... other repos

    let definition = def_repo.create(&AgentDefinitionCreate {
        name: "Test Agent".into(),
        description: None,
        system_prompt: None,
        tool_policy: serde_json::json!({}),
        memory_policy: serde_json::json!({}),
        delegation_policy: serde_json::json!({}),
        limits: serde_json::json!({}),
        default_model_policy: serde_json::json!({}),
    }).await.expect("create definition");

    let instance = inst_repo.create(&AgentInstanceCreate {
        agent_definition_id: definition.id,
        external_context_refs: vec![],
    }).await.expect("create instance");

    let run_service = RunService::new(
        def_repo.clone(),
        inst_repo.clone(),
        task_repo.clone(),
        // ... event, checkpoint, checkpointer, llm, tools
    );

    let (tx, mut rx) = mpsc::channel(32);
    
    let run_request = RunRequest {
        goal: "Say hello".into(),
        instructions: None,
        input_artifacts: vec![],
        external_context_refs: vec![],
        constraints: serde_json::json!({}),
        execution_mode: "sync".into(),
        expected_outputs: vec![],
        idempotency_key: None,
    };

    // This test would need a mock LLM to run without external API
    // For now, verify task creation logic
    let tasks_before = task_repo.list(100).await.expect("list tasks");
    
    // Note: Full test requires mock LLM client
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/agent-runtime-service/tests/v1_execution_tests.rs
git commit -m "test(run): add v1 execution integration test scaffold"
```

---

## Task 10: Update OpenAPI Spec

**Files:**
- Modify: `docs/openapi/torque-v1.yaml`

- [ ] **Step 1: Update RunRequest schema to match actual fields**

```yaml
RunRequest:
  type: object
  properties:
    goal:
      type: string
    instructions:
      type: string
    input_artifacts:
      type: array
      items:
        type: string
        format: uuid
    external_context_refs:
      type: array
      items:
        type: object
    constraints:
      type: object
    execution_mode:
      type: string
      enum: [sync, async]
      default: sync
    expected_outputs:
      type: array
      items:
        type: string
    idempotency_key:
      type: string
  required: [goal]
```

- [ ] **Step 2: Add SSE event schemas**

```yaml
RunEvent:
  type: object
  properties:
    event:
      type: string
      enum: [start, chunk, tool_call, tool_result, done, error]
    data:
      type: object
```

- [ ] **Step 3: Commit**

```bash
git add docs/openapi/torque-v1.yaml
git commit -m "docs(openapi): update RunRequest and add RunEvent schemas"
```

---

## Task 11: Final Verification

- [ ] **Step 1: Run full test suite**

```bash
cargo test -p agent-runtime-service
```

Expected: All tests pass

- [ ] **Step 2: Run compilation check**

```bash
cargo check -p agent-runtime-service
```

Expected: No errors

- [ ] **Step 3: Commit final state**

```bash
git add -A
git commit -m "feat(kernel-execution): complete v1 AgentInstance execution engine

- Add TaskStatus enum with valid state transitions
- Extend TaskRepository with create, update_status, update_produced_artifacts
- Extend AgentInstanceRepository with update_current_task
- Create v1 RunRequest to kernel ExecutionRequest mapping
- Create RunService orchestrating full execution lifecycle
- Wire RunService into ServiceContainer
- Implement real v1 runs handler with SSE streaming
- Refactor KernelRuntimeHandle for shared session/v1 use
- Add v1 execution integration test scaffold
- Update OpenAPI spec with RunRequest and RunEvent schemas"
```

---

## Testing Strategy

### Unit Tests
- TaskStatus transition validation
- RunService state machine logic (with mocked repositories)

### Integration Tests
- Full run lifecycle: create instance → POST /runs → verify task created → verify instance status updated
- SSE event stream validation (start, chunk, done sequence)
- Error handling: instance not found, definition not found

### Manual Verification
```bash
cargo run -p agent-runtime-service

# Create agent definition
curl -X POST http://localhost:3000/v1/agent-definitions \
  -H "Content-Type: application/json" \
  -H "X-API-Key: demo-key" \
  -d '{"name": "hello-agent", "system_prompt": "You are a helpful assistant."}'

# Create instance (use returned definition id)
curl -X POST http://localhost:3000/v1/agent-instances \
  -H "Content-Type: application/json" \
  -H "X-API-Key: demo-key" \
  -d '{"agent_definition_id": "<id>"}'

# Run (use returned instance id)
curl -N -X POST http://localhost:3000/v1/agent-instances/<id>/runs \
  -H "Content-Type: application/json" \
  -H "X-API-Key: demo-key" \
  -d '{"goal": "Say hello world"}'
```

---

## Known Limitations (Post-MVP)

1. **Tool execution** still uses the simple ToolRegistry; advanced tool governance (policy evaluation) is not yet implemented
2. **Async execution mode** returns SSE same as sync; true async with webhook callbacks is future work
3. **Team execution** is not covered; this plan focuses on single-agent instance execution
4. **Memory integration** during execution uses existing SessionService memory search; v1 memory integration is future work
5. **Checkpoint restore** (`POST /checkpoints/{id}/restore`) is not yet implemented
6. **Approval flow** during execution is not yet implemented

---

## Dependencies

- Existing `SessionService::chat` execution logic
- `torque-kernel` crate for ExecutionRequest/Result types
- `llm` crate for LlmClient trait
- `checkpointer` crate for Checkpoint trait
- Database tables: `v1_agent_definitions`, `v1_agent_instances`, `v1_tasks`, `v1_events`, `checkpoints`

---

## Success Criteria

- [ ] `POST /v1/agent-instances/{id}/runs` streams real LLM responses via SSE
- [ ] Task is created and transitions through states (Created → Running → Completed/Failed)
- [ ] AgentInstance status updates during execution
- [ ] Events are recorded during execution
- [ ] All existing tests continue to pass
- [ ] New integration tests verify run lifecycle

---

## Risk Mitigation

1. **Kernel bridge compatibility:** The existing `KernelRuntimeHandle::execute_chat` works with Session. Refactoring to `execute_v1` may introduce regressions. Mitigation: keep `execute_chat` as a thin wrapper around `execute_v1`.

2. **Database migrations:** No new migrations needed (Task table already exists). But TaskStatus enum change requires sqlx type mapping. Mitigation: verify `#[sqlx(rename_all = "snake_case")]` matches existing DB values.

3. **SSE client disconnection:** If client disconnects mid-stream, the execution continues in background. Mitigation: `event_sink.send().await` returns error on disconnect; spawn handle should be aborted or execution should check channel health.

4. **Concurrent runs:** Same instance receiving multiple run requests. Mitigation: Add run gate (similar to SessionService gate) or return 409 Conflict.

---

## Review Checklist

Before marking this plan as ready for execution:

- [ ] All file paths are exact and correct
- [ ] Each step is bite-sized (2-5 minutes)
- [ ] Code blocks are complete, not pseudocode
- [ ] Test commands and expected outputs are specified
- [ ] Commits are atomic and well-described
- [ ] No references to external context not in the plan
- [ ] Edge cases are noted (errors, disconnections, concurrent requests)
