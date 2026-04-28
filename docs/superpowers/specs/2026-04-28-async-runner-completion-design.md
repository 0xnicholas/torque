# Async Execution Webhooks — AsyncRunner Completion Design

## Overview

Complete `AsyncRunner` so `POST /v1/runs` with `async_execution: true` actually executes an agent task on the specified instance, updates run status, and sends webhook notifications.

**Date**: 2026-04-28
**Status**: Approved (post spec review)

## Problem

`AsyncRunner.process_run()` (`async_runner.rs:33`) is a no-op — it marks the run as Completed without executing any LLM work. Root cause: the `runs` table was never created in migrations, and the `Run` model lacks `agent_instance_id` and `request_payload`.

## Design

### 1. Migration — create the `runs` table

The `runs` table does not exist. Create it with all required columns:

```sql
CREATE TABLE IF NOT EXISTS runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID NOT NULL DEFAULT gen_random_uuid(),
    status VARCHAR(32) NOT NULL DEFAULT 'queued',
    agent_instance_id UUID NOT NULL,
    instruction TEXT NOT NULL DEFAULT '',
    request_payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    failure_policy VARCHAR(32),
    webhook_url TEXT,
    async_execution BOOLEAN NOT NULL DEFAULT false,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    error TEXT,
    webhook_sent_at TIMESTAMPTZ,
    webhook_attempts INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

### 2. Data model changes

`Run` model gains two fields:

```rust
pub struct Run {
    // ... existing ...
    pub agent_instance_id: Uuid,          // NEW
    pub request_payload: serde_json::Value, // NEW
}
```

`RunRequest` gains one optional field:

```rust
pub struct RunRequest {
    // ... existing ...
    #[serde(default)]
    pub agent_instance_id: Option<Uuid>,  // NEW
}
```

### 3. Repository changes

`RunRepository` trait adds `update_result`:

```rust
async fn update_result(
    &self, id: Uuid, status: RunStatus,
    started_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    error: Option<String>,
) -> anyhow::Result<()>;
```

All `get()` / `get_by_status()` / `create()` / `update_result()` SQL queries updated to include `agent_instance_id` and `request_payload`.

### 4. API validation

`runs::create` handler: if `async_execution: true` and `agent_instance_id` is `None`, return 400.

### 5. AsyncRunner rewrite

```rust
pub struct AsyncRunner {
    run_repo: Arc<dyn RunRepository>,
    run_service: Arc<RunService>,
    webhook_manager: WebhookManager,
}
```

```
process_run(run_id):
  1. fetch run → deserialize request_payload → RunRequest
  2. update_result(run_id, Running, started_at=now)
  3. noop_sink = mpsc::channel(1), drop receiver
  4. result = RunService.execute(agent_instance_id, request, noop_sink).await
  5. on Ok:
       update_result(run_id, Completed, completed_at=now)
       webhook("completed")
     on Err(e):
       update_result(run_id, Failed, error=e.to_string())
       webhook("failed", error=e.to_string())
```

### 6. ServiceContainer

```rust
let async_runner = Arc::new(AsyncRunner::new(repos.run.clone(), run.clone()));
```

## File changes

| File | Change |
|------|--------|
| `migrations/20260428000001_create_runs.up.sql` | New — CREATE TABLE runs |
| `migrations/20260428000001_create_runs.down.sql` | New — DROP TABLE runs |
| `models/v1/run.rs` | Run +2 fields, RunRequest +1 field |
| `repository/run.rs` | Trait + `update_result`; all queries updated |
| `api/v1/runs.rs` | Create handler: store new fields + validation |
| `service/async_runner.rs` | Rewrite process_run + inject RunService |
| `service/mod.rs` | AsyncRunner constructor change |

## No breaking changes

- `RunRequest.agent_instance_id` has `#[serde(default)]`
- `RunService.execute()` unchanged
- SSE endpoint unchanged
- Existing `Run` rows unaffected (table is newly created)

## Testing

Three integration tests in `tests/async_runner_tests.rs`:
1. Successful execution — creates instance, inserts run, AsyncRunner processes → status Completed
2. Failure on bad instance — run with nonexistent instance_id → status Failed, error populated
3. Webhook delivery — run with webhook_url → webhook_sent_at + webhook_attempts set

## Verification

```bash
cargo test -p torque-harness -- async_runner
cargo check --workspace
```
