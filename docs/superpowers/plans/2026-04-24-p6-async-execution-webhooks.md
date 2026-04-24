# P6: Async Execution Mode with Webhooks Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement true async execution mode where `POST /v1/runs` returns immediately with queued status, and webhook notifications are sent when execution completes.

**Architecture:**
- Add `async_execution` field to `RunConfig`
- `POST /v1/runs` with `async: true` returns immediately with `RunId` and `status: Queued`
- Webhook URL stored in run metadata
- Background worker processes queued runs
- Webhook callback on completion with run results
- `GET /v1/runs/{id}` returns status
- `GET /v1/runs/{id}/result` returns final output when complete

**Tech Stack:** Rust (tokio, sqlx, axum), PostgreSQL

---

## Task 1: Async Execution Data Model and Configuration

### Files
- Modify: `crates/torque-harness/src/models/v1/run.rs`
- Modify: `crates/torque-harness/src/service/run.rs`
- Create: `crates/torque-harness/migrations/{timestamp}_add_async_fields_to_v1_runs.up.sql`

- [ ] **Step 1: Add async fields to Run model**

Read `crates/torque-harness/src/models/v1/run.rs`.

Add to `Run` struct:
```rust
pub webhook_url: Option<String>,
pub async_execution: bool,
pub status: RunStatus,
```

Add new `RunStatus` variant:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RunStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}
```

- [ ] **Step 2: Add RunRepository methods**

Read `crates/torque-harness/src/repository/run.rs`.

Add to trait and implementation:
```rust
async fn update_status(&self, id: Uuid, status: RunStatus) -> anyhow::Result<()>;
async fn get_by_status(&self, status: RunStatus, limit: i64) -> anyhow::Result<Vec<Run>>;
```

- [ ] **Step 3: Add migration**

Create migration to add `webhook_url`, `async_execution` columns and update `status` enum.

- [ ] **Step 4: Run cargo check**

Run: `cargo check -p torque-harness`

- [ ] **Step 5: Commit**

```bash
git add crates/torque-harness/src/models/v1/run.rs crates/torque-harness/src/repository/run.rs
git commit -m "feat(async): add async execution fields to Run model"
```

---

## Task 2: Async Run Handler

### Files
- Modify: `crates/torque-harness/src/api/v1/runs.rs`
- Create: `crates/torque-harness/src/service/async_runner.rs`

- [ ] **Step 1: Modify POST /v1/runs for async**

Read `crates/torque-harness/src/api/v1/runs.rs`.

When `async: true` in request:
```rust
pub async fn create(
    // ...
) -> Result<Json<RunResponse>, ...> {
    let run = if request.async_execution {
        // Create run with Queued status
        let run = Run {
            id: Uuid::new_v4(),
            webhook_url: request.webhook_url.clone(),
            async_execution: true,
            status: RunStatus::Queued,
            // ... other fields
        };
        repo.create(&run).await?;

        // Spawn background worker
        tokio::spawn(async move {
            async_runner.process_run(run.id).await;
        });

        run
    } else {
        // Existing sync execution
        // ...
    };

    Ok(Json(RunResponse { run }))
}
```

- [ ] **Step 2: Create AsyncRunner service**

Create `crates/torque-harness/src/service/async_runner.rs`:
```rust
pub struct AsyncRunner {
    run_repo: Arc<dyn RunRepository>,
    // ... other dependencies
}

impl AsyncRunner {
    pub async fn process_run(&self, run_id: Uuid) -> anyhow::Result<()> {
        // 1. Update status to Running
        self.run_repo.update_status(run_id, RunStatus::Running).await?;

        // 2. Execute run (call existing execution logic)
        let result = self.execute_run(run_id).await?;

        // 3. Update status to Completed/Failed
        let final_status = match result {
            Ok(_) => RunStatus::Completed,
            Err(_) => RunStatus::Failed,
        };
        self.run_repo.update_status(run_id, final_status).await?;

        // 4. Send webhook if configured
        if let Some(webhook_url) = self.get_webhook_url(run_id).await? {
            self.send_webhook(&webhook_url, &result).await?;
        }

        Ok(())
    }
}
```

- [ ] **Step 3: Add webhook notification**

Add `send_webhook` method using `reqwest`:
```rust
async fn send_webhook(
    &self,
    url: &str,
    result: &Result<RunResult, anyhow::Error>,
) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let payload = serde_json::json!({
        "run_id": self.run_id,
        "status": result.as_ref().map(|_| "completed").unwrap_or("failed"),
        "result": result,
    });

    client.post(url)
        .json(&payload)
        .send()
        .await?;

    Ok(())
}
```

- [ ] **Step 4: Run cargo check**

Run: `cargo check -p torque-harness`

- [ ] **Step 5: Commit**

```bash
git add crates/torque-harness/src/api/v1/runs.rs crates/torque-harness/src/service/async_runner.rs
git commit -m "feat(async): add async run handler with webhook support"
```

---

## Task 3: Webhook Management

### Files
- Modify: `crates/torque-harness/src/service/async_runner.rs`
- Modify: `crates/torque-harness/src/api/v1/runs.rs`

- [ ] **Step 1: Add webhook registration endpoint**

Add `POST /v1/runs/{id}/webhook`:
```rust
pub async fn register_webhook(
    Path(id): Path<Uuid>,
    Json(req): Json<WebhookRegisterRequest>,
) -> Result<Json<Run>, ...> {
    let run = repo.get(id).await?.ok_or(StatusCode::NOT_FOUND)?;
    repo.update_webhook(id, &req.url).await?;

    Ok(Json(run))
}
```

- [ ] **Step 2: Add retry logic for webhooks**

Implement retry with exponential backoff:
```rust
async fn send_webhook_with_retry(
    &self,
    url: &str,
    payload: &serde_json::Value,
    max_retries: u32,
) -> anyhow::Result<()> {
    let mut retries = 0;
    loop {
        match self.send_webhook(url, payload).await {
            Ok(_) => return Ok(()),
            Err(e) if retries < max_retries => {
                retries += 1;
                let backoff = Duration::from_secs(2_u64.pow(retries));
                tokio::time::sleep(backoff).await;
            }
            Err(e) => return Err(e),
        }
    }
}
```

- [ ] **Step 3: Run cargo check**

Run: `cargo check -p torque-harness`

- [ ] **Step 4: Commit**

```bash
git add crates/torque-harness/src/api/v1/runs.rs crates/torque-harness/src/service/async_runner.rs
git commit -m "feat(async): add webhook registration and retry logic"
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

Add P6 section documenting:
- Async execution mode with webhooks
- POST /v1/runs with async: true returns Queued
- Webhook notifications on completion
- GET /v1/runs/{id} for status

- [ ] **Step 4: Final commit**

```bash
git add STATUS.md
git commit -m "docs: mark P6 Async Execution Mode complete"
```

---

## New Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/v1/runs` | POST | Create run (with `async: true` for async mode) |
| `/v1/runs/{id}` | GET | Get run status |
| `/v1/runs/{id}/result` | GET | Get run result (when complete) |
| `/v1/runs/{id}/webhook` | POST | Register webhook URL |

## Test Count Impact

- New tests: async_run_tests (3-5)
- Expected total: ~150 tests