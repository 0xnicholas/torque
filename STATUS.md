# Torque Project Status

**Branch:** `feat/kernel-execution`
**Date:** 2026-04-18
**Plan:** [Kernel Execution Engine Implementation](docs/superpowers/plans/2026-04-17-torque-kernel-execution-implementation.md)

---

## Overview

Implementing the v1 AgentInstance execution engine so that `/v1/agent-instances/{id}/runs` streams real tool-augmented LLM execution, with proper Task lifecycle management and Event recording.

This builds on top of the completed Architecture Optimization and Platform API v1 work (merged to main).

---

## Completed Tasks

### Phase 0: Foundation (from previous work)
- [x] Architecture Optimization Plan - Repository/Service/Kernel-bridge layers
- [x] Platform API v1 - All CRUD handlers, migrations, models
- [x] OpenAPI 3.1 spec at `docs/openapi/torque-v1.yaml`
- [x] v1 end-to-end integration tests

### Phase 1: Task State Management
- [x] **Task 1:** Extend Task Model with Status Enum
  - Added `TaskStatus` enum (Created, Queued, Running, WaitingTool, WaitingSubagent, WaitingApproval, Completed, Failed, Cancelled)
  - Added `is_terminal()` and `can_transition_to()` methods
  - Updated `Task` struct to use `TaskStatus` instead of `String`

- [x] **Task 2:** Extend Task Repository with State Management
  - Added `create()` method for creating tasks with initial status
  - Added `update_status()` for state transitions
  - Added `update_produced_artifacts()` for artifact tracking
  - Updated `cancel()` to use `TaskStatus::Cancelled`

- [x] **Task 3:** Extend AgentInstance Repository
  - Added `update_current_task()` to link instances to active tasks

### Phase 2: Execution Mapping
- [x] **Task 4:** Create v1 Execution Mapping
  - Created `kernel_bridge/v1_mapping.rs`
  - `v1_agent_definition_to_kernel()` - Maps v1 AgentDefinition to torque-kernel AgentDefinition
  - `run_request_to_execution_request()` - Maps v1 RunRequest to kernel ExecutionRequest

### Phase 3: Run Service
- [x] **Task 5:** Create Run Service
  - Created `service/run.rs` - `RunService` struct
  - Orchestrates full execution lifecycle:
    1. Fetch instance + definition
    2. Update instance status to Running
    3. Create Task with Created status
    4. Link task to instance, transition to Running
    5. Build kernel execution request
    6. Execute via `KernelRuntimeHandle::execute_chat()`
    7. Update task status (Completed/Failed)
    8. Update instance status (Ready/Failed)
    9. Send terminal SSE event (Done/Error)

- [x] **Task 6:** Wire RunService into ServiceContainer
  - Added `run` field to `ServiceContainer`
  - Constructed `RunService` with all required dependencies

### Phase 4: Handler Implementation
- [x] **Task 7:** Implement Real v1 Runs Handler
  - Rewrote `api/v1/runs.rs` to use `RunService::execute()`
  - SSE streaming with real event forwarding
  - Added `event_name()` helper to `StreamEvent`

---

## Remaining Tasks

- [x] **Task 8:** Refactor KernelRuntimeHandle
  - Extracted `execute_v1()` method with generic message support
  - `execute_chat()` now calls `execute_v1()` as backward-compatible wrapper
  - RunService updated to use `execute_v1()` for clarity

- [x] **Task 9:** Add Run Execution Integration Tests
  - Created `tests/v1_execution_tests.rs` with 3 tests
  - Test: Agent definition → instance → run → task lifecycle ✅
  - Test: SSE event stream validation (Start, Chunk, Done events) ✅
  - Test: Error handling for nonexistent instance ✅
  - Test: Task status transitions ✅
  - Uses FakeLlm to avoid external API calls

- [x] **Task 10:** Update OpenAPI Spec
  - Updated `RunRequest` schema with all actual fields
  - Added `RunEvent` schemas for SSE events (start, chunk, tool_call, tool_result, done, error)

- [x] **Task 11:** Final Verification
  - ✅ Full test suite: 20/20 tests pass
  - ✅ Compilation check: clean (no errors)
  - ✅ Working tree: clean

### Code Review Fixes (Post-Implementation)
- [x] **Critical 1-2:** Decouple policy evaluation from kernel execution
  - PolicyEvaluator moved to RunService (orchestration layer)
  - Kernel now receives pre-validated execution intent
  - PolicyDecision returned to caller instead of aborting execution

- [x] **Critical 3-4:** Recovery transaction safety and branching
  - Pre-validate recovery plan before mutations
  - time_travel creates new instance (branch) instead of modifying existing

- [x] **Critical 5-6:** Task state validation and PolicyEvaluator lifecycle
  - TaskStatus transition validation in TaskRepository::update_status
  - PolicyEvaluator instantiated once as RunService field

- [x] **Important 7-8:** Multi-source policy and event replay
  - PolicySources supports 6 source layers (system, capability, agent, team, selector, runtime)
  - Conservative merge across dimensions
  - EventReplayRegistry with async trait-based handlers
  - RecoveryService uses registry instead of hardcoded matching

- [x] **Important 9-12:** State validation, SSE, instance_id
  - TaskStatus transition table includes Created -> Running
  - SSE start event sent before execution begins
  - instance_id passed through v1 mapping to ExecutionRequest

---

## Blockers / Issues

### 1. ✅ RESOLVED: KernelRuntimeHandle.execute_chat Signature Mismatch (Task 8)
**Status:** Completed
**Resolution:** Extracted `execute_v1()` method that accepts `initial_messages: Vec<LlmMessage>`. `execute_chat()` is now a thin wrapper around `execute_v1()`. Both session chat and v1 runs share the same core execution logic. RunService uses `execute_v1()` with empty message history (conversation context across runs is future work).

### 2. Mock LLM for Testing (Task 9)
**Status:** Blocking integration tests
**Details:** Integration tests for run execution need a mock LLM client to avoid external API calls. The project already has `tests/common/fake_llm.rs` but it may need extension.
**Impact:** Medium - prevents automated testing of run endpoint
**Resolution:** Create or extend fake LLM implementation for tests

### 3. Concurrent Run Requests
**Status:** Potential issue, not yet implemented
**Details:** Same instance receiving multiple run requests simultaneously. No run gate or conflict detection is implemented yet.
**Impact:** Medium - could lead to race conditions
**Resolution:** Add run gate similar to SessionService's gate (return 409 Conflict)

---

## Current State

### Compilation
```bash
cargo check -p agent-runtime-service
```
✅ Clean (no errors)

### Tests
```bash
cargo test -p agent-runtime-service
```
✅ 17/17 tests passing:
- 2 project_scope_tests
- 6 session_http_api tests  
- 3 stream_event_tests
- 1 tool_registry_tests
- 4 v1_end_to_end tests
- 3 v1_execution_tests
- 0 doc-tests

### Git Status
Branch: `feat/kernel-execution`
Ahead of main: 7 commits
Working tree: clean

---

## Phase 2: Team Execution (COMPLETED)

### Task 1: Add TeamMember Model and Repository
- [x] Created `v1_team_members` table migration
- [x] Added `TeamMember` and `TeamMemberCreate` models
- [x] Added `TeamMemberRepository` with create/list/remove methods
- [x] Wired into `TeamService`, `RepositoryContainer`, `ServiceContainer`, `app.rs`

### Tasks 2-5: Implement Team Handlers
- [x] **create_task**: Creates `TeamTask`, returns 202 Accepted
- [x] **list_tasks**: Lists tasks filtered by `team_instance_id` with pagination
- [x] **list_members**: Lists team members with pagination
- [x] **publish**: Placeholder (returns 200), full shared state is future work

### Task 6: Team Execution Integration Tests
- [x] Created `tests/v1_team_execution_tests.rs` with 3 tests
- [x] Test: Team task lifecycle (definition → instance → task → list)
- [x] Test: Team member management (add, list, remove)
- [x] Test: Error handling for nonexistent team instance

### Task 7: Final Verification
- [x] Full test suite: 20/20 tests pass
- [x] Compilation check: clean (no errors)
- [x] Working tree: clean

---

## Next Steps

Branch `feat/kernel-execution` is ready with both Phase 1 (Kernel Execution) and Phase 2 (Team Execution) complete.

**Options:**
1. **Merge to main** — 14 commits ahead, all tests pass (20/20)
2. **Phase 3: Policy Evaluation** — Tool governance, delegation constraints, approval flow
3. **Phase 4: Recovery** — Checkpoint restore, event replay, state reconciliation
4. **Phase 5: Context State Management** — Lazy loading, context compaction, TaskPacket

---

## Known Limitations (Post-MVP)

1. **Tool execution** uses simple ToolRegistry; advanced tool governance (policy evaluation) not yet implemented
2. **Async execution mode** returns SSE same as sync; true async with webhooks is future work
3. **Team execution** not covered; this focuses on single-agent instance execution
4. **Memory integration** during execution uses existing SessionService memory search; v1 memory integration is future work
5. **Checkpoint restore** (`POST /checkpoints/{id}/restore`) not yet implemented
6. **Approval flow** during execution not yet implemented
7. **Conversation context** across multiple runs not yet implemented (each run starts with empty message history)

---

## Architecture Decisions

- **State type:** axum 0.7 nested routers share `(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)` tuple
- **Task status:** Typed enum with explicit transition rules prevents invalid state changes
- **Execution flow:** `RunRequest` → `ExecutionRequest` → `KernelRuntimeHandle` → LLM + Tools → SSE events
- **Event recording:** Kernel events are persisted to `v1_events` table during execution

---

## Resources

- **Plan:** `docs/superpowers/plans/2026-04-17-torque-kernel-execution-implementation.md`
- **Kernel Spec:** `docs/superpowers/specs/2026-04-08-torque-kernel-execution-contract-design.md`
- **OpenAPI:** `docs/openapi/torque-v1.yaml`
- **Worktree:** `.worktrees/kernel-execution`

---

## Contact

For questions or blockers, see:
- `/help` for opencode usage
- GitHub issues: https://github.com/anomalyco/opencode/issues
