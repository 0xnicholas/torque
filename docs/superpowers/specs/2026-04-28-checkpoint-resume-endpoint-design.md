# Checkpoint Resume Endpoint Design

## Overview

Add `POST /v1/checkpoints/:id/resume` — restores agent instance state from a checkpoint and continues execution with SSE streaming.

**Date**: 2026-04-28
**Status**: Approved

## Flow

1. Validate checkpoint exists (404 if not)
2. `RecoveryService.restore_from_checkpoint(id)` — restore instance state, replay tail events, reconcile
3. `RecoveryService.assess_recovery(id)` — if terminal (Completed/Failed), return 409 CONFLICT
4. `RunService.execute(instance_id, request, event_sink)` — create task, execute via kernel, stream SSE
5. Return SSE stream

## Endpoint

```
POST /v1/checkpoints/:id/resume
Body: RunRequest
Response: SSE stream
```

## File changes

| File | Change |
|------|--------|
| `api/v1/checkpoints.rs` | Add `resume` handler |
| `api/v1/mod.rs` | Add route |

## No changes

- RecoveryService, RunService, models — all unchanged

## Verification

```bash
cargo test -p torque-harness -- checkpoint
cargo check --workspace
```
