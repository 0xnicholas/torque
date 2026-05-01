# Session + Compaction API Implementation Plan

## Objective

Add a first-class `Session` domain entity and an explicit compaction API (`Session.compact(customInstructions?)` + `Session.abortCompaction()`) to Torque, enabling long-running agents to manually trigger context compression with custom summarization instructions and abort in-flight compaction.

This is a **new domain layer** that sits above the existing `AgentInstance`/`Run` primitives:

```
┌──────────────────────────────────────────────────┐
│  Session (domain entity)                          │
│  ┌────────────────────────────────────────────┐   │
│  │  SessionService                            │   │
│  │   ├─ chat(message) → SSE stream            │   │
│  │   ├─ compact(session_id, instructions?)    │   │
│  │   └─ abort_compaction(session_id)          │   │
│  └───────────────┬────────────────────────────┘   │
│                  │ uses                            │
│  ┌───────────────▼────────────────────────────┐   │
│  │  ContextCompactionService (torque-runtime)   │   │
│  │   ├─ compact(messages, instructions,       │   │
│  │   │           &CancellationToken)           │   │
│  │   └─ LlmSummarizer                         │   │
│  │        └─ summarize(messages,              │   │
│  │             instructions, &CancellationToken)│   │
│  └─────────────────────────────────────────────┘   │
│                                                    │
│  AgentInstance (kernel)        Run (execution log) │
└──────────────────────────────────────────────────┘
```

---

## Implementation Plan

### Phase A: Session Domain Layer

**Rationale**: The `Session` does not exist in code yet. It must be created first as the foundation for all subsequent API work. Session wraps an `AgentInstance` + its `Run` history, providing a stable identity for long-running conversations.

- [ ] **A1. Create `SessionStatus` enum** — Define variants: `Active`, `Idle`, `Compacting`, `Error`, `Terminated`. The `Compacting` status is critical for `abortCompaction()` — it signals that a compaction is in flight.
- [ ] **A2. Create `Session` struct** — Fields: `id: Uuid`, `agent_instance_id: Uuid`, `status: SessionStatus`, `title: String`, `metadata: HashMap<String,String>`, `runs: Vec<Uuid>`, `active_compaction_job_id: Option<Uuid>`, `created_at`, `updated_at`. The `active_compaction_job_id` is the handle for abort.
- [ ] **A3. Create `SessionRepository` trait** — Async trait with methods: `create`, `get_by_id`, `list`, `update_status`, `list_by_status`, `set_compaction_job`, `clear_compaction_job`. The compaction job methods are the abort mechanism's persistence layer.
- [ ] **A4. Create `PostgresSessionRepository`** — Implementation using `sqlx`. Map `Session` to a `sessions` table. The `active_compaction_job_id` column allows tracking across restarts.
- [ ] **A5. Create DB migration** — New file `20260501000002_create_sessions.up.sql` with `sessions` table (id UUID PK, agent_instance_id UUID, status VARCHAR, title TEXT, metadata JSONB, active_compaction_job_id UUID NULL, created_at TIMESTAMPTZ, updated_at TIMESTAMPTZ) + indexes on `(status)` and `(agent_instance_id)`.

### Phase B: Compaction Infrastructure Extension

**Rationale**: The existing `ContextCompactionService` and `LlmSummarizer` do not support custom instructions or cancellation. These changes are in `torque-runtime` and must not break existing consumers.

- [ ] **B1. Extend `LlmSummarizer` trait** — Add `instructions: Option<&str>` and `cancel: &CancellationToken` parameters to the `summarize()` method:
  ```
  async fn summarize(&self, messages: &[LlmMessage],
                     instructions: Option<&str>,
                     cancel: &CancellationToken) -> Option<String>;
  ```
  **Rationale**: `instructions` enables the user to guide summarization ("focus on technical decisions", "keep the error discussion"); `cancel` enables abort.
- [ ] **B2. Add `CancellationToken` type** — Simple struct wrapping `Arc<AtomicBool>`:
  ```
  pub struct CancellationToken { inner: Arc<AtomicBool> }
  impl CancellationToken {
      pub fn new() -> Self;
      pub fn cancel(&self);       // sets flag
      pub fn is_cancelled(&self) -> bool;  // checks flag
      pub fn child(&self) -> Self;  // linked to parent
  }
  ```
  **Rationale**: Uses the same pattern as `AbortSignal` and the existing `cancel_signal` in host.rs. No new dependency needed. The `child()` method supports scope-based cancellation (aborting compaction should not cancel the whole session).
- [ ] **B3. Add `CompactionJob` handle struct**:
  ```
  pub struct CompactionJob {
      pub id: Uuid,
      pub cancel: CancellationToken,
      pub status: CompactionJobStatus,
  }
  pub enum CompactionJobStatus { Running, Completed, Aborted }
  ```
  **Rationale**: Provides the identity and abort handle that the Session API returns to the caller. The `status` field enables querying.
- [ ] **B4. Extend `ContextCompactionService::compact()`** — Add `instructions: Option<String>` and `cancel: CancellationToken` parameters. Thread `instructions` and `cancel` to the LLM summarizer. Add `cancel.is_cancelled()` check at each step (before/after summarizer, before key_facts extraction).
  **Rationale**: Cancellation checks between steps allow mid-compaction abort even during LLM summarization.
- [ ] **B5. Add `compact_with_options()` method** — A new public method on `ContextCompactionService` that takes `instructions` and `cancel` alongside `messages`. Keep the existing `compact()` for backward compatibility (it calls the new method with `None` and a no-op `CancellationToken`).
  ```rust
  pub async fn compact_with_options(&self, messages: &[LlmMessage],
      instructions: Option<String>, cancel: &CancellationToken) -> Option<CompactSummary>;
  ```
- [ ] **B6. Add `abort_token` field to `MessageQueue` trait** — Add `fn abort_compaction(&self)` method to the trait. The `InMemoryMessageQueue` implementation tracks the active `CancellationToken` and calls `cancel()` on it.
  **Rationale**: The `RuntimeHost` holds the `MessageQueue` — to abort compaction, the abort signal must reach the queue.

### Phase C: SessionService

**Rationale**: The `SessionService` is the orchestration layer that binds the Session domain with the runtime. It provides the `chat()`, `compact()`, and `abort_compaction()` methods that the API handlers call.

- [ ] **C1. Create `SessionService` struct** — Fields:
  ```
  pub struct SessionService {
      session_repo: Arc<dyn SessionRepository>,
      runtime_factory: Arc<RuntimeFactory>,
      llm: Arc<dyn LlmClient>,
      tools: Arc<ToolService>,
      tool_governance: Arc<ToolGovernanceService>,
      memory: Arc<MemoryService>,
      compaction_service: ContextCompactionService,
      active_jobs: Arc<tokio::sync::RwLock<HashMap<Uuid, CompactionJobRegistryEntry>>>,
  }
  struct CompactionJobRegistryEntry {
      job: CompactionJob,
      queue_id: Uuid,        // identifies which queue to abort
  }
  ```
- [ ] **C2. Implement `SessionService::chat()`** — Adapts the existing `RunService::execute()` flow. Creates/gets session, creates agent instance if needed, builds `KernelRuntimeHandle`, runs `kernel.execute_v1_with_queue()`, stores Run record under session. Returns SSE stream of `StreamEvent`.
  **Rationale**: Uses the existing execution pipeline. No new kernel code; this is a service-layer wrapper with Session tracking.
- [ ] **C3. Implement `SessionService::compact()`** — Takes `session_id` and optional `instructions`. Steps:
  1. Fetch session from repo, verify status is `Active` (not already `Compacting`).
  2. Create `CompactionJob` with new `CancellationToken`. Store in `active_jobs` map.
  3. Update session status to `Compacting`, set `active_compaction_job_id`.
  4. Get the `KernelRuntimeHandle` for the session (or create one from the last checkpoint).
  5. Call `runtime.execute_hooks(HookPoint::PreCompaction, ...)` if extension system is enabled.
  6. Call `runtime.compact_with_options(messages, instructions, cancel)`.
  7. Update session status back to `Active`, clear `active_compaction_job_id`.
  8. Return `CompactionJob { id, status }`.
  **Rationale**: The status transition (`Active → Compacting → Active`) is critical for correctness of `abortCompaction()`. The Hook integration point allows extensions to observe/prepare for compaction.
- [ ] **C4. Implement `SessionService::abort_compaction()`** — Takes `session_id`. Steps:
  1. Look up active job in `active_jobs` map by session_id.
  2. Call `job.cancel.cancel()` (sets the `AtomicBool`).
  3. Call `queue.abort_compaction()` to flush the cancellation to the message queue.
  4. Update session status back to `Active`.
  5. Return `CompactionJob { id, status: Aborted }`.
  **Rationale**: Aborting is orchestrated — setting the flag is not enough; the queue must be notified to interrupt in-progress summarization.
- [ ] **C5. Register `SessionService` in `ServiceContainer`** — Add `pub session: Arc<SessionService>` field. Plumb its dependencies (runtime_factory, llm, tools, etc.) in `ServiceContainer::new()`.

### Phase D: API Layer

**Rationale**: REST endpoints expose the Session + Compaction API to clients. These follow the existing API patterns (axum extractors, JSON bodies, SSE streaming).

- [ ] **D1. Create `crates/torque-harness/src/api/v1/sessions.rs`** — Handlers for:
  - `POST /v1/sessions` — Create session. Body: `{ agent_definition_id, title?, metadata? }`. Returns `Session`.
  - `GET /v1/sessions` — List sessions. Query: `status?`, `limit`, `offset`.
  - `GET /v1/sessions/:id` — Get session details.
  - `POST /v1/sessions/:id/chat` — Send message. Body: `{ message }`. Returns SSE stream (same pattern as `POST /v1/runs/:id`). This is the primary entry point.
  - `POST /v1/sessions/:id/compact` — Trigger compaction. Body: `{ custom_instructions? }`. Returns `{ job_id, status }`.
  - `POST /v1/sessions/:id/compaction/abort` — Abort in-flight compaction. Returns `{ job_id, status: "aborted" }`.
- [ ] **D2. Register routes in `api/v1/mod.rs`** — Add sessions router with all endpoints.
- [ ] **D3. Add session status filter on compaction operations** — `compact()` returns `409 Conflict` if session is not `Active`. `abort_compaction()` returns `404` if no active compaction job found.

### Phase E: Extension Hook Integration

**Rationale**: The existing extension system can observe and influence compaction events without modifying the core path. This is optional but demonstrates the extension system's value.

- [ ] **E1. Add `PRE_COMPACTION` and `POST_COMPACTION` to `HookPointDef`** — Two new predefined hooks in `crates/torque-extension/src/hook/definition.rs`. These fire before and after `SessionService::compact()` runs.
  **Rationale**: Extensions can log compaction sizes, inject metadata into instructions, or abort compaction based on policy.
- [ ] **E2. Add `HookInput::Compaction` variant** — New variant in `crates/torque-extension/src/hook/input.rs` that carries the compaction context:
  ```
  Compaction {
      message_count: usize,
      custom_instructions: Option<String>,
      session_id: String,
  }
  ```
  **Rationale**: Provides extension handlers with full compaction context for decision-making.
- [ ] **E3. Wire hook execution into `SessionService::compact()`** — Before compaction: call `runtime.execute_hooks(HookPoint::PreCompaction, input)`. After compaction: call `runtime.execute_hooks(HookPoint::PostCompaction, result)`. Feature-gated behind `#[cfg(feature = "extension")]`.
  **Rationale**: Keeps extension dependency optional without `#[cfg()]` checks in the core logic path.

### Phase F: Tests

- [ ] **F1. Unit tests for `CancellationToken`** — Create, cancel, is_cancelled, child propagation, cancel is idempotent.
- [ ] **F2. Unit tests for extended `ContextCompactionService::compact_with_options()`** — Without summarizer (custom instructions ignored, heuristic used), with mock summarizer (instructions passed through), cancellation between steps (cancel mid-execution returns None).
- [ ] **F3. Unit tests for `CompactionJob`** — Status transitions: Running → Completed, Running → Aborted, Aborted cannot be cancelled again.
- [ ] **F4. Unit tests for `SessionService::compact()`** — Mock `SessionRepository`. Verify status transitions, verify cancellation token reaches summarizer, verify double-compact returns Conflict.
- [ ] **F5. Unit tests for `SessionService::abort_compaction()`** — Abort active job returns Aborted status, abort on session with no active job returns 404, abort is idempotent.
- [ ] **F6. Integration test for full flow** — Create session → chat → compact with instructions → abort → verify session status.

---

## Verification Criteria

- [ ] **C1**: `cargo test -p torque-runtime` passes with 0 failures after LlmSummarizer + ContextCompactionService changes
- [ ] **C2**: `cargo test -p torque-harness` passes with 0 failures after SessionService + API additions
- [ ] **C3**: `cargo test -p torque-extension` passes with 0 failures after HookPointDef additions
- [ ] **C4**: `cargo check --features extension` compiles cleanly after extension hook wiring
- [ ] **C5**: `POST /v1/sessions/:id/compact` returns HTTP 200 with `{ job_id, status: "running" }`
- [ ] **C6**: `POST /v1/sessions/:id/compaction/abort` returns HTTP 200 with `{ job_id, status: "aborted" }`
- [ ] **C7**: Double compact on same session returns HTTP 409 Conflict
- [ ] **C8**: Abort on session with no active compaction returns HTTP 404

---

## Potential Risks and Mitigations

1. **Backward compatibility of `LlmSummarizer` trait change**
   Mitigation: Use optional parameters with defaults. The trait method signature change is breaking, so add a new method (`summarize_with_options`) and deprecate the old one rather than changing the existing signature. This avoids breaking external implementations that may exist outside the repo.

2. **Concurrent compaction + chat race condition**
   Mitigation: The `SessionStatus::Compacting` gate prevents concurrent modifications. The `ServiceContainer` holds a single `SessionService` with the `active_jobs` map behind `RwLock`. All session operations check status first.

3. **Compaction takes too long / hangs**
   Mitigation: The `CancellationToken` mechanism provides a clean abort. The `compact_with_options()` method should also accept a timeout parameter (or the caller provides one). If the LLM summarizer hangs, `abort_compaction()` will not interrupt it unless the summarizer checks `is_cancelled()`. Mitigation: document that summarizer implementations MUST check `is_cancelled()` periodically.

4. **CancellationToken child() semantics are confusing**
   Mitigation: Keep it simple. Don't implement `child()` initially — use a single shared token. If scope-based cancellation is needed later, it can be added without breaking changes.

5. **Session table migration failure on existing deployments**
   Mitigation: The migration is additive (new table, no column changes to existing tables). No data migration needed. Add migration guard: `IF NOT EXISTS`.

---

## Alternative Approaches

1. **Instead of a new `Session` entity, extend `AgentInstance` to hold compaction state**
   Trade-off: Keeps schema simpler but conflates kernel-level state (instance lifecycle) with session-level concerns (user-facing conversation). Rejected because `AgentInstance` is in `torque-kernel` which has no dependencies — adding compaction state there would pull in concerns that don't belong at the kernel level.

2. **Instead of a new `CancellationToken`, reuse `tokio_util::sync::CancellationToken`**
   Trade-off: External dependency vs. simple `Arc<AtomicBool>`. The `tokio_util` version has richer semantics (linked children, poll-based await). Rejected for now to avoid adding a dependency; the simple flag suffices and matches the existing `AbortSignal` pattern.

3. **Instead of `SessionService::compact()` being synchronous (wait for completion), return immediately and track asynchronously**
   Trade-off: Better UX (non-blocking) vs. simpler implementation (wait). The current design blocks because context compaction operates on the session's message queue which is single-threaded per session. If async is needed later, the `CompactionJob` handle already supports it.

4. **Instead of extension hooks for compaction, let extensions replace the entire compaction strategy**
   Trade-off: More flexible vs. more complex interface. The observation hooks (PRE/POST) are the minimal integration point. A full strategy replacement would require formalizing a `CompactionStrategy` trait in torque-kernel, which is more appropriate for a future Phase.
