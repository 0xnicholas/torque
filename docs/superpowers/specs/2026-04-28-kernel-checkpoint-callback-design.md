# Kernel CheckpointCallback Design

## Overview

Define a `CheckpointCallback` trait in `torque-kernel` so the execution engine can notify persistence layers when the instance enters a checkpoint-worthy state (AwaitingTool, AwaitingApproval, AwaitingDelegation, Suspended).

**Date**: 2026-04-28
**Status**: Approved

## Problem

`engine.rs:53` contains a TODO documenting the need for checkpoint creation on state transitions. The `RuntimeHost` in `torque-runtime` already creates checkpoints at the call-site level, but the kernel has no formal mechanism to signal these transitions.

## Design

### CheckpointCallback trait

```rust
pub trait CheckpointCallback {
    fn on_awaiting_state(
        &self,
        instance_id: AgentInstanceId,
        task_id: TaskId,
        from_state: AgentInstanceState,
        to_state: AgentInstanceState,
        approval_ids: &[ApprovalRequestId],
        delegation_ids: &[DelegationRequestId],
    );
}
```

### Engine changes

`ExecutionEngine::step()` accepts an optional `&dyn CheckpointCallback`. After state transitions to any waiting/suspended state, the callback is invoked.

### Callers

- `InMemoryKernelRuntime::handle_command()` passes `None` (backward compatible)
- Future callers can pass concrete implementations

## File changes

| File | Change |
|------|--------|
| `crates/torque-kernel/src/engine.rs` | Add trait, update step() signature, invoke callback, delete TODO |
| `crates/torque-kernel/src/runtime.rs` | Pass `None` in handle_command |
| `crates/torque-kernel/src/lib.rs` | Export `CheckpointCallback` |
| `crates/torque-kernel/tests/kernel_contracts.rs` | Add callback tests, update existing calls |

## Verification

```bash
cargo test -p torque-kernel
cargo check --workspace
```
