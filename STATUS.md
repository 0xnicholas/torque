# Torque Project Status

**Branch:** `main`
**Date:** 2026-04-24
**Compilation:** Clean (20 warnings from existing code)
**Tests:** 141/141 passing

---

## Phase 0: Foundation (COMPLETED)

### Architecture Optimization
- Repository/Service/Kernel-bridge layers
- `crates/torque-harness/src/repository/`
- `crates/torque-harness/src/service/`
- `crates/torque-harness/src/kernel_bridge/`

### Platform API v1
- All CRUD handlers for AgentDefinitions, AgentInstances, Tasks, Events, Artifacts, Approvals
- Migrations for all v1 tables
- `crates/torque-harness/src/api/v1/`
- `crates/torque-harness/src/models/v1/`

### OpenAPI 3.1 Spec
- Full API spec at `docs/openapi/torque-v1.yaml`

---

## Phase 1: Agent Runtime Kernel (COMPLETED)

### Task State Management
- `TaskStatus` enum: Created, Queued, Running, WaitingTool, WaitingSubagent, WaitingApproval, Completed, Failed, Cancelled
- `is_terminal()` and `can_transition_to()` methods
- `TaskRepository` with state transition validation

### Run Service
- `RunService` orchestrates full execution lifecycle
- `KernelRuntimeHandle::execute_v1()` for chat execution
- SSE streaming with real event forwarding

### Kernel Bridge
- `kernel_bridge/v1_mapping.rs` - Maps v1 types to kernel types
- `kernel_bridge/runtime.rs` - Kernel runtime handle
- `kernel_bridge/events.rs` - Event recording

**Implementation:** `crates/torque-harness/src/service/run.rs`, `kernel_bridge/`
**Tests:** `v1_execution_tests` (3 tests)

---

## Phase 2: Memory System P0 (COMPLETED)

### Memory Tables + pgvector
- `v1_memory_entries` table with vector(1536) embedding support
- `v1_memory_write_candidates` table with status enum
- `session_memory` table (KV + TTL)
- `memory_decision_log` table (audit trail)
- HNSW indexes for semantic search

### Embedding Pipeline
- `EmbeddingGenerator` trait with OpenAI implementation
- `memory_to_embedding_text()` helper
- Integrated into `MemoryService::v1_create_entry()`

### Semantic Retrieval
- `semantic_search()` - vector similarity with pgvector `<=>`
- `hybrid_search()` - RRF fusion of vector + keyword (ts_rank_cd)
- `POST /v1/memory-entries/search` API

### Session Memory
- `SessionMemoryRepository` with TTL support
- Internal service (no public API in P0)

### Backfill APIs
- `GET /v1/memory-entries/without-embedding`
- `POST /v1/memory-entries/backfill`

**Implementation:** `crates/torque-harness/src/service/memory.rs`, `repository/memory_v1.rs`, `vector_type.rs`
**Tests:** `memory_*` tests (17 tests total)

---

## Phase 3: Team Execution (COMPLETED)

### TeamMember Model & Repository
- `v1_team_members` table migration
- `TeamMemberRepository` with create/list/remove

### Team Handlers
- `POST /v1/team-instances/{id}/tasks` - Creates TeamTask, returns 202
- `GET /v1/team-instances/{id}/tasks` - List with pagination
- `GET /v1/team-instances/{id}/members` - List with pagination
- `POST /v1/team-instances/{id}/publish` - Placeholder

### Member Agent
- `MemberAgent` for team member execution
- `LocalMemberAgent` implementation

**Implementation:** `crates/torque-harness/src/models/v1/team.rs`, `repository/team.rs`, `service/team/service.rs`, `service/team/member_agent.rs`
**Tests:** `member_agent_tests` (3), `v1_team_execution_tests` (4)

---

## Phase 4: Team Supervisor Agent (COMPLETED)

### Supervisor Tools (14 total)
- delegate_task, add_blocker, resolve_blocker
- accept_result, reject_result, complete_team_task
- get_task_details, get_delegation_status, list_team_members
- get_shared_state, update_shared_fact, publish_to_team
- request_approval, fail_team_task

### SupervisorAgent
- LLM-driven triage (no heuristic fallback)
- SupervisorTools registry
- Mode handlers: Execute, Route, Await, React

### EventListener
- `wait_for_delegation_completion` helper
- Event-driven delegation waiting
- Async delegation flow support

**Implementation:** `crates/torque-harness/src/service/team/supervisor*.rs`, `modes.rs`, `supervisor_tools.rs`
**Tests:** `v1_team_supervisor_agent_tests` (6), `v1_team_supervisor_tools_tests` (16), `async_delegation_flow_tests` (6)
**Total:** 28 supervisor-related tests

---

## Phase 5: Checkpoint & Recovery (COMPLETED)

### Snapshot Format
- Aligned checkpoint snapshot format between creation and reading
- `RecoveryService` reads `custom_state` directly

### Event Replay
- `ApprovalRequestedHandler`
- `DelegationRequestedHandler`
- `EventReplayRegistry` with async trait-based handlers

### Kernel Assessment Integration
- `RecoveryService` uses kernel `assess_recovery` when available
- Fallback manual assessment when kernel not available

### Reconciliation
- Detect child instance failures
- Resolution actions: ReissueDelegation, AcceptCompletedOutput
- `ReconciliationResult` with inconsistencies and resolutions

### Restore + Resume
- `POST /v1/checkpoints/{id}/restore` - Returns detailed RecoveryResult
- `POST /v1/checkpoints/{id}/resume` - Checks terminal state, triggers execution

**Implementation:** `crates/torque-harness/src/service/recovery.rs`, `repository/checkpoint.rs`, `service/event_replay.rs`
**Tests:** `checkpoint_recovery_tests` (8 tests)

---

## Phase 6: Capability Registry (COMPLETED)

### CapabilityRef
- `CapabilityRef` newtype with `as_str()` method
- `ResolvedCandidate` and `CapabilityResolution` types

### Repositories
- `PostgresCapabilityProfileRepository::get_by_name()`
- `PostgresCapabilityRegistryBindingRepository::list_by_profile()`

### Resolution Service
- `CapabilityService::resolve_by_ref()` implementation
- Sorts candidates by compatibility_score (descending)

### API Endpoint
- `POST /v1/capabilities/resolve` - Resolve capability ref to agents

**Implementation:** `crates/torque-harness/src/models/v1/capability.rs`, `service/capability.rs`, `repository/capability.rs`, `api/v1/capabilities.rs`
**Tests:** `capability_resolution_tests` (3 tests)

---

## Phase 7: Delegation System (COMPLETED)

### Delegation Model
- `Delegation` with states: Pending, InProgress, Completed, Failed, Cancelled
- `DelegationEvent` for state transitions

### Delegation Service
- Create, update, cancel delegations
- Event emission for state changes

### Async Delegation Flow
- `EventListener` for delegation completion
- `wait_for_delegation_completion` helper

**Implementation:** `crates/torque-harness/src/models/v1/delegation.rs`, `service/delegation.rs`, `repository/delegation.rs`
**Tests:** `delegation_repo_tests` (2), `delegation_status_tests` (3), `event_listener_tests` (6), `async_delegation_flow_tests` (6)

---

## Phase 8: Policy Evaluation (COMPLETED)

### Policy Model
- Multi-source policy: system, capability, agent, team, selector, runtime
- Conservative merge across dimensions

### PolicyEvaluator
- Moved to RunService (orchestration layer)
- Kernel receives pre-validated execution intent
- `PolicyDecision` returned to caller

### Governance
- Tool policy evaluation
- Memory policy evaluation
- Delegation policy evaluation

**Implementation:** `crates/torque-harness/src/policy/evaluator.rs`, `models/v1/gating.rs`
**Tests:** `agent_runner_tests` (5)

---

## Phase 9: Memory Pipeline (COMPLETED)

### Candidate Generation
- `CandidateGenerator` service
- LLM fact extraction integrated with RunService

### Memory Gating
- `MemoryGatingService` with quality assessment
- Risk/conflict/consent rules
- `GateDecision` with types: Approve, Reject, Review

### Memory Compaction
- Background job for memory compaction
- `jobs/memory_compaction.rs`

**Implementation:** `crates/torque-harness/src/service/candidate_generator.rs`, `service/gating.rs`, `jobs/memory_compaction.rs`
**Tests:** `jobs_memory_compaction_tests` (1), `memory_candidate_api_tests` (11)

---

## Phase 10: Notification System (COMPLETED)

### NotificationService
- Async notification dispatch
- Hook-based architecture

### NotificationHooks
- `NotificationHooks` registry
- Support for multiple hook types

**Implementation:** `crates/torque-harness/src/service/notification.rs`, `notification/hooks.rs`
**Tests:** `notification_service_tests` (1), `notification_hooks_tests` (2)

---

## Phase 11: Deduplication (COMPLETED)

### Dynamic Dedup Thresholds
- `DedupThresholds` with per-category thresholds (duplicate, merge, minimum_content_length)
- `GatingConfig::dedup_thresholds: HashMap<MemoryCategory, DedupThresholds>`
- Environment variable override support: `MEMORY_DEDUP_{CATEGORY}_DUPLICATE/MERGE`
- `DedupThresholds::for_category()`, `from_config()`, `with_env_override()`

### Equivalence Check Integration
- Serial integration: dedup → equivalence check
- `check_equivalence_for_candidate()` - triggers on boundary cases and mergeable results
- Uses rules engine `check_equivalence()` for decision support

### LLM Retry and Fallback
- 3 retry attempts for LLM API calls
- Fallback to `Distinct` on all retries failing
- Log warnings for failed LLM calls

### Conflict Detection
- `ConflictResult` and `ConflictType` types
- `detect_conflict()` - identifies when keys differ but content is similar
- Routes to Review with high priority

### Four Merge Strategies
- `AppendStrategy` - combines values into array with deduplication
- `KeepSeparateStrategy` - stores as separate entries
- `WithProvenanceStrategy` - tracks merge history in `_provenance` field
- `SummarizeStrategy` - uses LLM for consolidation
- `MergeStrategyExecutor` - routes to appropriate handler

### Gating Flow Rewrite
- `gate_candidate()` orchestrates: quality → risk → dedup → equivalence → conflict → decision
- `resolve_with_rules()` - decision matrix combining dedup action and equivalence result
- `check_equivalence_via_llm_with_fallback()` - LLM fallback for boundary cases
- Decision logging at end of gating flow

### Configuration Validation
- `ConfigError` enum for validation errors
- `GatingConfigValidator::validate()` - validates merge <= duplicate for all categories

**Implementation:**
- `crates/torque-harness/src/models/v1/gating.rs` - DedupThresholds, GatingConfig, ConflictResult, ConfigError
- `crates/torque-harness/src/service/gating.rs` - gate_candidate, equivalence, merge
- `crates/torque-harness/src/service/merge_strategy.rs` - Four merge strategies
- `crates/torque-harness/src/config/memory.rs` - Dynamic threshold loading
- `crates/torque-harness/tests/dedup_thresholds_tests.rs` - DedupThresholds unit tests
- `crates/torque-harness/tests/merge_strategy_tests.rs` - Merge strategy unit tests

**Tests:** `dedup_thresholds_tests` (5), `merge_strategy_tests` (4)

---

## Phase 12: Governance & Audit (COMPLETED)

### Decision Log Query API
- `GET /v1/memory/decisions` - Query decision history with filtering
- Filter by: agent_instance_id, decision_type, start_date, end_date
- Pagination with cursor-based offset
- `list_decisions()` in MemoryRepositoryV1 and MemoryService

### Enhanced Review Queue
- `GET /v1/memory/candidates` now returns `CandidateListResponse` with stats
- `CandidateStats`: total, pending, review_required, auto_approved, approved, rejected, merged
- `count_candidates_by_status()` for aggregate counts by status

### Manual Compaction Trigger
- `POST /v1/memory/compact` - Triggers background compaction job
- `CompactionJob` and `CompactionJobStatus` models
- Returns job with Pending status (stub implementation)

### Decision Analytics
- `GET /v1/memory/decisions/stats` - Decision statistics endpoint
- `DecisionStats`: total_decisions, approved, rejected, merged, review, approval_rate, rejection_rate, avg_quality_score, top_rejection_reasons
- `RejectionReasonCount`: reason and count

**Implementation:** `crates/torque-harness/src/repository/memory_v1.rs`, `service/memory.rs`, `api/v1/memory.rs`, `models/v1/memory.rs`
**Tests:** `decision_log_tests` (8)

---

## P3: Advanced Features (COMPLETED)

### Proper Memory Compaction with Summarization
- `CompactionStrategy` enum: NoOp, Truncate, Summarize, Consolidate
- `CompactionRecommendation` with strategy selection rationale
- `superseded_by` field on `MemoryEntry` for tracking replacements
- Memory compaction job uses recommendations for strategy selection

### Context Anchors in Checkpoint
- `ContextAnchor` struct: anchor_type, reference_id, captured_at
- `ContextAnchorType` enum (5 variants): ExternalContextRef, Artifact, MemoryEntry, SharedState, EventAnchor
- Captured during checkpoint creation via `MemoryService::capture_context_anchors()`
- Stored in `v1_checkpoints.context_anchors` JSONB column

### Team-Level Recovery Foundation
- `TeamRecoveryDisposition` enum: Recoverable, RequirableSupervisor, NonRecoverable
- `assess_team_recovery()` - evaluates team-level recovery options
- `recover_team_task()` - coordinates team task recovery with child instances
- `recover_with_children()` - cascading recovery across team hierarchy

**Implementation:** `crates/torque-harness/src/models/v1/checkpoint.rs`, `service/memory.rs`, `service/recovery.rs`
**Tests:** `checkpoint_recovery_tests` (8 tests), `jobs_memory_compaction_tests` (1)

---

## Current Test Suite (141 tests passing)

| Test File | Count | Status |
|-----------|-------|--------|
| agent_runner_tests | 5 | ✅ |
| api_tests | 2 | ✅ |
| async_delegation_flow_tests | 6 | ✅ |
| capability_resolution_tests | 3 | ✅ |
| chat_streaming_api | 3 | ✅ |
| checkpoint_recovery_tests | 8 | ✅ |
| context_window_tests | 2 | ✅ |
| decision_log_tests | 8 | ✅ |
| delegation_repo_tests | 2 | ✅ |
| delegation_status_tests | 3 | ✅ |
| event_listener_tests | 6 | ✅ |
| jobs_memory_compaction_tests | 1 | ✅ |
| member_agent_tests | 3 | ✅ |
| memory_candidate_api_tests | 11 | ✅ |
| memory_recall_tests | 2 | ✅ |
| memory_sse_tests | 1 | ✅ |
| notification_hooks_tests | 2 | ✅ |
| notification_service_tests | 1 | ✅ |
| project_scope_tests | 2 | ✅ |
| session_http_api | 6 | ✅ |
| stream_event_tests | 3 | ✅ |
| tool_registry_tests | 1 | ✅ |
| v1_end_to_end | 4 | ✅ |
| v1_execution_tests | 3 | ✅ |
| v1_team_execution_tests | 4 | ✅ |
| v1_team_supervisor_agent_tests | 6 | ✅ |
| v1_team_supervisor_tools_tests | 16 | ✅ |
| dedup_thresholds_tests | 5 | ✅ |
| merge_strategy_tests | 4 | ✅ |
| **TOTAL** | **141** | ✅ |

---

## Known Limitations (Post-MVP)

1. **Full message history replay** - MVP restarts execution from checkpoint; replay is future work
2. **Operator escalation endpoints** - For high-severity reconciliation issues
3. **Async execution mode** - Returns SSE same as sync; true async with webhooks is future work
4. **Tool execution** - Uses simple ToolRegistry; advanced tool governance not yet implemented

---

## Architecture Decisions

- **State type:** axum 0.7 nested routers share `(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)` tuple
- **Task status:** Typed enum with explicit transition rules prevents invalid state changes
- **Execution flow:** `RunRequest` → `ExecutionRequest` → `KernelRuntimeHandle` → LLM + Tools → SSE events
- **Event recording:** Kernel events persisted to `v1_events` table during execution
- **Supervisor collaboration:** Supervisor → Subagent model (not symmetric peers)
- **Context planes:** ExternalContextRef, Artifact, Memory kept separate

---

## Resources

- **Specs:** `docs/superpowers/specs/`
  - `torque-kernel-execution-contract-design.md`
  - `torque-agent-runtime-harness-design.md`
  - `torque-agent-team-design.md`
  - `torque-capability-registry-model-design.md`
  - `torque-context-state-model-design.md`
  - `torque-recovery-core-design.md`
  - `torque-memory-system-design.md`
  - `torque-policy-model-design.md`
- **OpenAPI:** `docs/openapi/torque-v1.yaml`
- **Plans:** `docs/superpowers/plans/`

---

## Git Log (Recent)

```
952e0e6 fix: change COMPACTION_ERROR to DB_ERROR for consistency
c5a8c09 docs: mark P2 Governance & Audit complete
f6b2deb feat(governance): add GET /v1/memory/decisions/stats endpoint
8b96d20 Add POST /v1/memory/compact endpoint for manual compaction trigger
0b8108e fixup: add auto_approved and merged to CandidateStats
fac996c feat(governance): add stats to review queue endpoint
077491c fix: refactor list_decisions to use separate compiled queries
a533a54 Add GET /v1/memory/decisions endpoint for decision log query
```

---

## Next Steps

### Future
- Full message history replay
- Operator escalation endpoints
- Advanced tool governance
- True async execution with webhooks
