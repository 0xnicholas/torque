# Torque Runtime-Neutral Interfaces Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Introduce runtime-neutral ports and adapters so the runtime host can move from `torque-harness` into `torque-runtime` without depending on harness transport, repository, or service types.

**Architecture:** Implement this as an adapter-first slice. First add runtime-owned neutral message, tool, checkpoint, and output types in `torque-runtime`. Then add harness adapters for model driving, tool execution, event persistence, checkpoint persistence, hydration, and streaming output. Only after those ports compile and are covered by focused tests should the host constructor be rewritten to depend solely on runtime-neutral interfaces.

**Tech Stack:** Rust 2021 workspace, `torque-kernel`, `torque-runtime`, `torque-harness`, `llm`, `checkpointer`, Axum/SSE transport in harness, existing focused tests under `crates/torque-harness/tests`

---

## File Map

### Existing files to modify

- `crates/torque-runtime/src/environment.rs`
  Replace the placeholder port set with the runtime-neutral interfaces from the spec.
- `crates/torque-runtime/src/tools.rs`
  Expand the runtime-owned tool result and tool definition types.
- `crates/torque-runtime/src/events.rs`
  Add runtime-owned output-event and checkpoint-event types.
- `crates/torque-runtime/src/checkpoint.rs`
  Add runtime-owned checkpoint payload and hydration-state types.
- `crates/torque-runtime/src/host.rs`
  Change the host constructor and execution flow to depend on neutral ports, not harness types.
- `crates/torque-runtime/src/lib.rs`
  Export the new runtime-neutral surface.
- `crates/torque-harness/src/runtime/host.rs`
  Replace implementation with a thin re-export once the host lives in `torque-runtime`.
- `crates/torque-harness/src/runtime/environment.rs`
  Re-export or adapt to the final runtime-neutral interfaces.
- `crates/torque-harness/src/service/session.rs`
  Build and pass the new runtime adapters instead of direct harness-owned dependencies.
- `crates/torque-harness/src/service/run.rs`
  Build and pass the new runtime adapters instead of direct harness-owned dependencies.
- `crates/torque-harness/src/service/mod.rs`
  Assemble the adapters centrally inside `ServiceContainer`.
- `crates/torque-harness/src/app.rs`
  Build adapter instances for app startup paths.
- `crates/torque-harness/src/kernel_bridge/v1_mapping.rs`
  Keep only mapping logic that still belongs in harness, or move neutral pieces into `torque-runtime` if needed.
- `crates/torque-harness/tests/agent_runner_tests.rs`
  Assert the session flow still works through runtime-neutral adapters.
- `crates/torque-harness/tests/memory_recall_tests.rs`
  Update constructor wiring if it currently depends on old host signatures.
- `crates/torque-harness/tests/v1_execution_tests.rs`
  Update v1 execution wiring if it currently depends on old host signatures.

### New files to create

- `crates/torque-runtime/src/message.rs`
  Runtime-owned normalized message type and conversion helpers.
- `crates/torque-harness/src/runtime/adapters/model_driver.rs`
  Adapter from harness `LlmClient` to `RuntimeModelDriver`.
- `crates/torque-harness/src/runtime/adapters/tool_executor.rs`
  Adapter from harness `ToolService` to `RuntimeToolExecutor`.
- `crates/torque-harness/src/runtime/adapters/event_sink.rs`
  Adapter from harness repositories to `RuntimeEventSink`.
- `crates/torque-harness/src/runtime/adapters/checkpoint_sink.rs`
  Adapter from harness checkpointer to `RuntimeCheckpointSink`.
- `crates/torque-harness/src/runtime/adapters/hydration_source.rs`
  Adapter from `SessionRepository` and related state loaders to `RuntimeHydrationSource`.
- `crates/torque-harness/src/runtime/adapters/output_sink.rs`
  Adapter from runtime-neutral output callbacks to `StreamEvent`.
- `crates/torque-harness/src/runtime/adapters/mod.rs`
  Harness adapter exports.
- `crates/torque-runtime/tests/environment_contracts.rs`
  Focused tests for the neutral type and trait contracts.

### Files to consult while implementing

- `docs/superpowers/specs/2026-04-25-torque-runtime-layer-design.md`
- `docs/superpowers/specs/2026-04-26-torque-runtime-neutral-interfaces-design.md`
- `docs/superpowers/plans/2026-04-25-runtime-layer-refactor.md`
- `.worktrees/runtime-layer-refactor/crates/torque-harness/src/runtime/host.rs`
- `.worktrees/runtime-layer-refactor/crates/torque-harness/src/service/session.rs`
- `.worktrees/runtime-layer-refactor/crates/torque-harness/src/service/run.rs`
- `.worktrees/runtime-layer-refactor/crates/torque-runtime/src/environment.rs`

---

### Task 1: Add Runtime-Neutral Core Types

**Files:**
- Create: `crates/torque-runtime/src/message.rs`
- Modify: `crates/torque-runtime/src/tools.rs`
- Modify: `crates/torque-runtime/src/events.rs`
- Modify: `crates/torque-runtime/src/checkpoint.rs`
- Modify: `crates/torque-runtime/src/lib.rs`
- Test: `crates/torque-runtime/tests/environment_contracts.rs`

- [ ] **Step 1: Write the failing runtime contract tests**

Create tests covering:
- `RuntimeMessage` round-trips role/content data
- `RuntimeToolResult` carries success/content/error/offload metadata
- `RuntimeCheckpointPayload` captures kernel-derived checkpoint information
- `RuntimeOutputEvent` or equivalent neutral output type does not mention `StreamEvent`

- [ ] **Step 2: Run the crate-focused tests and verify they fail**

Run: `cargo test -p torque-runtime --test environment_contracts -- --nocapture`
Expected: FAIL because the neutral types do not exist yet.

- [ ] **Step 3: Add runtime-owned message and tool types**

Implement:
- `RuntimeMessage`
- `RuntimeMessageRole`
- `RuntimeToolDef`
- expanded `RuntimeToolResult`
- `RuntimeToolCall`
- `RuntimeOffloadRef`

- [ ] **Step 4: Add runtime-owned checkpoint and output types**

Implement:
- `RuntimeCheckpointPayload`
- `RuntimeCheckpointRef`
- `HydrationState`
- `RuntimeOutputEvent` or equivalent neutral output representation

- [ ] **Step 5: Export the new runtime surface**

Update `crates/torque-runtime/src/lib.rs` so the new types are public and discoverable.

- [ ] **Step 6: Re-run the crate-focused tests**

Run: `cargo test -p torque-runtime --test environment_contracts -- --nocapture`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/torque-runtime/src/message.rs \
  crates/torque-runtime/src/tools.rs \
  crates/torque-runtime/src/events.rs \
  crates/torque-runtime/src/checkpoint.rs \
  crates/torque-runtime/src/lib.rs \
  crates/torque-runtime/tests/environment_contracts.rs
git commit -F - <<'EOF'
Define the runtime-neutral data types for host extraction

This gives torque-runtime its own message, tool, checkpoint, and output
shapes so the host can stop depending on harness transport and service
types.

Constraint: The new types must not overlap with kernel execution objects
Rejected: Reuse harness StreamEvent and ToolService shapes | keeps runtime coupled to harness
Confidence: high
Scope-risk: moderate
Directive: Keep runtime-neutral types transport-agnostic and service-agnostic
Tested: cargo test -p torque-runtime --test environment_contracts -- --nocapture
Not-tested: host integration
EOF
```

---

### Task 2: Replace Placeholder Runtime Ports With Neutral Interfaces

**Files:**
- Modify: `crates/torque-runtime/src/environment.rs`
- Modify: `crates/torque-runtime/src/lib.rs`
- Test: `crates/torque-runtime/tests/environment_contracts.rs`

- [ ] **Step 1: Extend the failing tests**

Add compile-oriented assertions for:
- `RuntimeModelDriver`
- `RuntimeToolExecutor`
- `RuntimeEventSink`
- `RuntimeCheckpointSink`
- `RuntimeHydrationSource`
- `RuntimeOutputSink`

- [ ] **Step 2: Run the runtime contract tests and verify they fail**

Run: `cargo test -p torque-runtime --test environment_contracts -- --nocapture`
Expected: FAIL because the current port definitions are incomplete.

- [ ] **Step 3: Implement the neutral port traits**

Update `crates/torque-runtime/src/environment.rs` to match the spec:
- remove direct harness-local assumptions
- use runtime-owned message/tool/checkpoint types
- define sink signatures in terms of runtime-neutral payloads

- [ ] **Step 4: Re-export the ports**

Update `crates/torque-runtime/src/lib.rs` so the port traits are part of the public runtime interface.

- [ ] **Step 5: Re-run the runtime contract tests**

Run: `cargo test -p torque-runtime --test environment_contracts -- --nocapture`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/torque-runtime/src/environment.rs \
  crates/torque-runtime/src/lib.rs \
  crates/torque-runtime/tests/environment_contracts.rs
git commit -F - <<'EOF'
Define the runtime-neutral ports for host extraction

This replaces placeholder runtime interfaces with the minimal neutral
port set needed to move the host out of harness-owned transport,
repository, and service types.

Constraint: Port signatures must not mention StreamEvent, ToolService, or harness repositories
Rejected: Keep broad placeholder traits | not enough structure to extract the host safely
Confidence: high
Scope-risk: moderate
Directive: Add new host dependencies through these ports instead of direct harness imports
Tested: cargo test -p torque-runtime --test environment_contracts -- --nocapture
Not-tested: harness adapters
EOF
```

---

### Task 3: Add Harness Adapters for Model, Tool, Event, Checkpoint, Hydration, and Output

**Files:**
- Create: `crates/torque-harness/src/runtime/adapters/mod.rs`
- Create: `crates/torque-harness/src/runtime/adapters/model_driver.rs`
- Create: `crates/torque-harness/src/runtime/adapters/tool_executor.rs`
- Create: `crates/torque-harness/src/runtime/adapters/event_sink.rs`
- Create: `crates/torque-harness/src/runtime/adapters/checkpoint_sink.rs`
- Create: `crates/torque-harness/src/runtime/adapters/hydration_source.rs`
- Create: `crates/torque-harness/src/runtime/adapters/output_sink.rs`
- Modify: `crates/torque-harness/src/runtime/mod.rs`
- Test: `crates/torque-harness/tests/runtime_adapter_tests.rs`

- [ ] **Step 1: Write the failing adapter tests**

Cover:
- model driver adapts harness LLM messages to `RuntimeMessage`
- tool executor adapts `ToolService`
- output sink translates neutral output into `StreamEvent`
- event/checkpoint/hydration adapters compile and return expected shapes in focused fake-backed tests

- [ ] **Step 2: Run the adapter tests and verify they fail**

Run: `cargo test -p torque-harness --test runtime_adapter_tests -- --nocapture`
Expected: FAIL because the adapter modules do not exist.

- [ ] **Step 3: Implement model and tool adapters**

Add:
- adapter from `Arc<dyn llm::LlmClient>` to `RuntimeModelDriver`
- adapter from `Arc<ToolService>` to `RuntimeToolExecutor`

- [ ] **Step 4: Implement event, checkpoint, and hydration adapters**

Add:
- adapter from harness repositories to `RuntimeEventSink`
- adapter from harness checkpointer to `RuntimeCheckpointSink`
- adapter from `SessionRepository` or equivalent loaders to `RuntimeHydrationSource`

- [ ] **Step 5: Implement the output sink adapter**

Add a harness-local adapter that translates runtime-neutral output callbacks into `StreamEvent` emissions without leaking `StreamEvent` into `torque-runtime`.

- [ ] **Step 6: Export the adapters from `runtime/mod.rs`**

Make the harness runtime adapters easy to assemble from `SessionService` and `RunService`.

- [ ] **Step 7: Re-run the adapter tests**

Run: `cargo test -p torque-harness --test runtime_adapter_tests -- --nocapture`
Expected: PASS

- [ ] **Step 8: Commit**

```bash
git add crates/torque-harness/src/runtime/adapters \
  crates/torque-harness/src/runtime/mod.rs \
  crates/torque-harness/tests/runtime_adapter_tests.rs
git commit -F - <<'EOF'
Add harness adapters for the runtime-neutral interfaces

This lets harness-owned services and transport surfaces satisfy the new
runtime-neutral ports without leaking their types into torque-runtime.

Constraint: Adapters may depend on harness services, but torque-runtime must not depend back on them
Rejected: Pass harness types directly through runtime traits | defeats the layer split
Confidence: high
Scope-risk: moderate
Directive: Keep adapters one-way; runtime owns the interface, harness owns the implementation
Tested: cargo test -p torque-harness --test runtime_adapter_tests -- --nocapture
Not-tested: host migration
EOF
```

---

### Task 4: Rewrite the Host to Depend Only on Runtime-Neutral Ports

**Files:**
- Modify: `crates/torque-runtime/src/host.rs`
- Modify: `crates/torque-runtime/src/lib.rs`
- Modify: `crates/torque-harness/src/runtime/host.rs`
- Test: `crates/torque-runtime/tests/host_port_integration.rs`
- Test: `crates/torque-harness/tests/runtime_host_path_tests.rs`

- [ ] **Step 1: Write the failing host integration tests**

Create tests proving:
- `RuntimeHost` can be constructed using only the neutral interfaces
- harness can still import the host through the compatibility path

- [ ] **Step 2: Run the focused tests and verify they fail**

Run: `cargo test -p torque-runtime --test host_port_integration -- --nocapture`
Run: `cargo test -p torque-harness --test runtime_host_path_tests -- --nocapture`
Expected: FAIL because the host still depends on harness-owned types.

- [ ] **Step 3: Move the host constructor and host-owned logic**

Rewrite `crates/torque-runtime/src/host.rs` so it accepts only:
- `RuntimeModelDriver`
- `RuntimeToolExecutor`
- `RuntimeEventSink`
- `RuntimeCheckpointSink`
- `RuntimeHydrationSource`
- optional `RuntimeOutputSink`

- [ ] **Step 4: Reduce harness host file to a compatibility re-export**

Update `crates/torque-harness/src/runtime/host.rs` to re-export the runtime crate host.

- [ ] **Step 5: Re-run the focused tests**

Run: `cargo test -p torque-runtime --test host_port_integration -- --nocapture`
Run: `cargo test -p torque-harness --test runtime_host_path_tests -- --nocapture`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/torque-runtime/src/host.rs \
  crates/torque-runtime/src/lib.rs \
  crates/torque-harness/src/runtime/host.rs \
  crates/torque-runtime/tests/host_port_integration.rs \
  crates/torque-harness/tests/runtime_host_path_tests.rs
git commit -F - <<'EOF'
Move the runtime host onto runtime-neutral interfaces

This completes the host extraction path by making the host depend only
on runtime-owned contracts and harness-supplied adapters.

Constraint: The host must no longer mention ToolService, SessionRepository, or StreamEvent
Rejected: Keep a harness-owned host wrapper indefinitely | leaves the runtime split incomplete
Confidence: medium
Scope-risk: moderate
Directive: Future host features must be added through runtime-neutral ports first
Tested: cargo test -p torque-runtime --test host_port_integration -- --nocapture; cargo test -p torque-harness --test runtime_host_path_tests -- --nocapture
Not-tested: full session/run integration
EOF
```

---

### Task 5: Reassemble `SessionService` and `RunService` Around Adapters

**Files:**
- Modify: `crates/torque-harness/src/service/mod.rs`
- Modify: `crates/torque-harness/src/service/session.rs`
- Modify: `crates/torque-harness/src/service/run.rs`
- Modify: `crates/torque-harness/src/app.rs`
- Modify: `crates/torque-harness/tests/agent_runner_tests.rs`
- Modify: `crates/torque-harness/tests/memory_recall_tests.rs`
- Modify: `crates/torque-harness/tests/v1_execution_tests.rs`

- [ ] **Step 1: Write or extend the failing focused integration tests**

Ensure these tests build the services through the new adapter path:
- `agent_runner_tests`
- `memory_recall_tests`
- `v1_execution_tests`

- [ ] **Step 2: Run the focused integration tests and verify they fail**

Run: `cargo test -p torque-harness --test agent_runner_tests -- --nocapture`
Run: `cargo test -p torque-harness --test memory_recall_tests -- --nocapture`
Run: `cargo test -p torque-harness --test v1_execution_tests -- --nocapture`
Expected: FAIL because service wiring still assumes direct host dependencies.

- [ ] **Step 3: Update central service assembly**

Refactor `ServiceContainer::new` and `app.rs` so they build adapter instances once and pass them into session/run services.

- [ ] **Step 4: Update `SessionService`**

Stop constructing host dependencies ad hoc inside `chat()`. Use the runtime-neutral host and harness adapters instead.

- [ ] **Step 5: Update `RunService`**

Stop constructing host dependencies ad hoc in the v1 execution path. Use the runtime-neutral host and harness adapters instead.

- [ ] **Step 6: Re-run the focused integration tests**

Run: `cargo test -p torque-harness --test agent_runner_tests -- --nocapture`
Run: `cargo test -p torque-harness --test memory_recall_tests -- --nocapture`
Run: `cargo test -p torque-harness --test v1_execution_tests -- --nocapture`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/torque-harness/src/service/mod.rs \
  crates/torque-harness/src/service/session.rs \
  crates/torque-harness/src/service/run.rs \
  crates/torque-harness/src/app.rs \
  crates/torque-harness/tests/agent_runner_tests.rs \
  crates/torque-harness/tests/memory_recall_tests.rs \
  crates/torque-harness/tests/v1_execution_tests.rs
git commit -F - <<'EOF'
Reassemble harness services around runtime-neutral adapters

This completes the adapter-first migration by making session and run
services consume the runtime host through neutral interfaces rather than
direct harness-owned host dependencies.

Constraint: Existing session and v1 execution behavior must remain intact
Rejected: Leave service construction split across app and handlers | makes adapter ownership unclear
Confidence: medium
Scope-risk: moderate
Directive: Build runtime adapters centrally and inject them; do not reconstruct them ad hoc inside service methods
Tested: cargo test -p torque-harness --test agent_runner_tests -- --nocapture; cargo test -p torque-harness --test memory_recall_tests -- --nocapture; cargo test -p torque-harness --test v1_execution_tests -- --nocapture
Not-tested: full end-to-end API suites
EOF
```

---

## Risks and Mitigations

- The new ports can become too abstract and harder to use than the direct harness types.
  Mitigation: keep only the six ports from the spec and resist adding policy or service concerns to them.

- Adapter code can accidentally duplicate business logic.
  Mitigation: adapters should translate and delegate only; decision-making stays in services or runtime host.

- The host migration can regress streaming semantics.
  Mitigation: keep `RuntimeOutputSink` narrow and cover `StreamEvent` translation in focused tests before moving the host.

- The existing `runtime-layer-refactor` worktree already has in-progress edits in target files.
  Mitigation: implement this plan on top of that branch, reading current diffs carefully and not reverting unrelated in-flight changes.

## Verification Steps

Run these before claiming completion:

- `cargo test -p torque-runtime --test environment_contracts -- --nocapture`
- `cargo test -p torque-runtime --test host_port_integration -- --nocapture`
- `cargo test -p torque-harness --test runtime_adapter_tests -- --nocapture`
- `cargo test -p torque-harness --test runtime_host_path_tests -- --nocapture`
- `cargo test -p torque-harness --test agent_runner_tests -- --nocapture`
- `cargo test -p torque-harness --test memory_recall_tests -- --nocapture`
- `cargo test -p torque-harness --test v1_execution_tests -- --nocapture`

## Notes for Executors

- Start from `.worktrees/runtime-layer-refactor`, not from `main`.
- Do not delete or revert unrelated in-progress edits already present in that worktree.
- Do not pass `StreamEvent`, `ToolService`, or repository traits through runtime interfaces.
- Do not create new execution objects in `torque-runtime`; use kernel-owned execution types and runtime-owned adapter types only.
