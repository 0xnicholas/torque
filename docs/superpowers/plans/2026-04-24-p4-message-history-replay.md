# P4: Full Message History Replay Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enable true point-in-time resume by storing, retrieving, and replaying full message history from checkpoints.

**Architecture:**
- Checkpoints already store `messages` in `CheckpointState`
- Need to: (1) expose messages via API, (2) use messages when resuming, (3) rebuild state via event replay
- RecoveryResult will include message history for client-side use

**Tech Stack:** Rust (tokio, sqlx, axum), PostgreSQL

---

## Task 1: Expose Checkpoint Messages via API

### Files
- Modify: `crates/torque-harness/src/api/v1/checkpoints.rs`
- Modify: `crates/torque-harness/src/service/recovery.rs`
- Create: `crates/torque-harness/tests/checkpoint_message_tests.rs`

- [ ] **Step 1: Add get_checkpoint_messages method to CheckpointRepository**

Read `crates/torque-harness/src/repository/checkpoint.rs`.

Add to `CheckpointRepository` trait:
```rust
async fn get_messages(&self, checkpoint_id: Uuid) -> anyhow::Result<Vec<checkpointer::Message>>;
```

Add implementation that loads checkpoint and returns its `messages` field.

- [ ] **Step 2: Add get_checkpoint_messages to RecoveryService**

Read `crates/torque-harness/src/service/recovery.rs`.

Add method:
```rust
pub async fn get_checkpoint_messages(
    &self,
    checkpoint_id: Uuid,
) -> anyhow::Result<Vec<checkpointer::Message>> {
    self.checkpoint_repo.get_messages(checkpoint_id).await
}
```

- [ ] **Step 3: Add GET /v1/checkpoints/{id}/messages endpoint**

Read `crates/torque-harness/src/api/v1/checkpoints.rs`.

Add handler:
```rust
pub async fn get_messages(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
) -> Result<Json<CheckpointMessagesResponse>, (StatusCode, Json<ErrorBody>)> {
    let messages = services
        .recovery
        .get_checkpoint_messages(id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    code: "DB_ERROR".into(),
                    message: e.to_string(),
                    details: None,
                    request_id: None,
                }),
            )
        })?;

    Ok(Json(CheckpointMessagesResponse { messages }))
}

#[derive(serde::Serialize)]
pub struct CheckpointMessagesResponse {
    pub checkpoint_id: Uuid,
    pub messages: Vec<checkpointer::Message>,
}
```

- [ ] **Step 4: Register route**

Read `crates/torque-harness/src/api/v1/mod.rs`.

Add route:
```rust
.route("/v1/checkpoints/:id/messages", get(checkpoints::get_messages))
```

- [ ] **Step 5: Add to recovery restore response**

Modify `restore_from_checkpoint` to also return messages alongside the instance:
```rust
pub async fn restore_from_checkpoint(
    &self,
    checkpoint_id: Uuid,
) -> anyhow::Result<(AgentInstance, Vec<checkpointer::Message>)> {
    // ... existing restore logic ...
    // Return both instance and messages
    Ok((instance, checkpoint.messages))
}
```

Update callers of `restore_from_checkpoint` to handle the new return type.

- [ ] **Step 6: Create checkpoint_message_tests.rs**

Create `crates/torque-harness/tests/checkpoint_message_tests.rs`:
```rust
mod common;
use common::setup_test_db_or_skip;
use checkpointer::{CheckpointState, Message};

#[tokio::test]
async fn test_get_checkpoint_messages() {
    let db = match setup_test_db_or_skip().await {
        Some(db) => db,
        None => return,
    };
    let repo = PostgresCheckpointRepository::new(db);

    let state = CheckpointState {
        messages: vec![
            Message { role: "user".to_string(), content: "Hello".to_string() },
            Message { role: "assistant".to_string(), content: "Hi!".to_string() },
        ],
        tool_call_count: 0,
        intermediate_results: vec![],
        custom_state: None,
    };

    let checkpoint_id = repo.save(run_id, node_id, state).await.unwrap();
    let messages = repo.get_messages(checkpoint_id).await.unwrap();

    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].content, "Hello");
}
```

- [ ] **Step 7: Run cargo check**

Run: `cargo check -p torque-harness`

- [ ] **Step 8: Commit**

```bash
git add crates/torque-harness/src/api/v1/checkpoints.rs crates/torque-harness/src/service/recovery.rs crates/torque-harness/src/repository/checkpoint.rs crates/torque-harness/tests/checkpoint_message_tests.rs
git commit -m "feat(recovery): expose checkpoint messages via API"
```

---

## Task 2: Resume with Message History

### Files
- Modify: `crates/torque-harness/src/kernel_bridge/runtime.rs`
- Modify: `crates/torque-harness/src/service/run.rs`

- [ ] **Step 1: Modify resume_instance to return messages**

Read `crates/torque-harness/src/service/recovery.rs`.

Change `resume_instance` to return messages:
```rust
pub async fn resume_instance(
    &self,
    instance_id: Uuid,
) -> anyhow::Result<(AgentInstance, Vec<checkpointer::Message>)> {
    let checkpoints = self.checkpoint_repo.list_by_instance(instance_id, 1).await?;

    if let Some(checkpoint) = checkpoints.into_iter().next() {
        let (instance, messages) = self.restore_from_checkpoint(checkpoint.id).await?;
        Ok((instance, messages))
    } else {
        let instance = self.agent_instance_repo.get(instance_id).await?;
        Ok((instance, vec![]))
    }
}
```

- [ ] **Step 2: Add execute_with_messages to RunService**

Read `crates/torque-harness/src/service/run.rs`.

Add method:
```rust
pub async fn execute_with_messages(
    &self,
    request: ExecuteRequest,
    initial_messages: Vec<LlmMessage>,
) -> anyhow::Result<ExecuteResponse> {
    // Use provided messages instead of building fresh
    let state = self.runtime.execute_v1(
        request.agent_instance_id,
        request.llm_config.as_deref().unwrap_or(&self.default_llm),
        &request.tools,
        request.system_prompt.as_deref(),
        initial_messages,
        request.event_sink,
    ).await?;

    Ok(ExecuteResponse { state })
}
```

- [ ] **Step 3: Wire resume to use stored messages**

Find where resume is called and update to:
1. Call `resume_instance` to get (instance, messages)
2. If messages exist, call `execute_with_messages` with those messages
3. Otherwise, start fresh

- [ ] **Step 4: Run cargo check**

Run: `cargo check -p torque-harness`

- [ ] **Step 5: Commit**

```bash
git add crates/torque-harness/src/kernel_bridge/runtime.rs crates/torque-harness/src/service/run.rs crates/torque-harness/src/service/recovery.rs
git commit -m "feat(recovery): resume with message history"
```

---

## Task 3: Event Replay for State Reconstruction

### Files
- Modify: `crates/torque-harness/src/service/recovery.rs`
- Modify: `crates/torque-harness/src/service/event_replay.rs`

- [ ] **Step 1: Add rebuild_state_from_events method**

Read `crates/torque-harness/src/service/recovery.rs`.

Add method:
```rust
pub async fn rebuild_state_from_events(
    &self,
    instance_id: Uuid,
    from_event_id: Uuid,
) -> anyhow::Result<RebuiltState> {
    let events = self.event_repo.list_after(from_event_id).await?;

    let mut state = RebuiltState::default();
    for event in events {
        self.apply_event_to_state(&event, &mut state).await?;
    }

    Ok(state)
}

#[derive(Default)]
pub struct RebuiltState {
    pub tool_call_count: u32,
    pub intermediate_results: Vec<ArtifactPointer>,
}
```

- [ ] **Step 2: Integrate with restore flow**

Modify `restore_from_checkpoint` to:
1. Load checkpoint messages
2. If checkpoint has event_anchor, rebuild state from events after that anchor
3. Return (instance, messages, rebuilt_state)

- [ ] **Step 3: Run cargo check**

Run: `cargo check -p torque-harness`

- [ ] **Step 4: Commit**

```bash
git add crates/torque-harness/src/service/recovery.rs crates/torque-harness/src/service/event_replay.rs
git commit -m "feat(recovery): event replay for state reconstruction"
```

---

## Task 4: Final Verification

- [ ] **Step 1: Run full test suite**

Run: `cargo test -p torque-harness 2>&1 | tail -50`
Expected: All tests pass

- [ ] **Step 2: Run cargo check for warnings**

Run: `cargo check -p torque-harness 2>&1 | grep -E "warning|error"`
Expected: Only existing warnings

- [ ] **Step 3: Update STATUS.md**

Add P4 section documenting:
- Message history in checkpoints
- GET /v1/checkpoints/{id}/messages endpoint
- Resume with message history
- Event replay for state reconstruction

- [ ] **Step 4: Final commit**

```bash
git add STATUS.md
git commit -m "docs: mark P4 Full Message History Replay complete"
```

---

## New Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/v1/checkpoints/{id}/messages` | GET | Get message history from checkpoint |

## Test Count Impact

- New tests: checkpoint_message_tests (2-3)
- Expected total: ~145 tests