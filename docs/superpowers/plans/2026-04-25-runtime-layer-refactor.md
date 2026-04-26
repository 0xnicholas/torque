# Torque Runtime Layer Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor Torque so `torque-kernel` remains the stable execution-contract layer, `torque-runtime` becomes the replaceable execution environment, and `torque-harness` keeps only product-facing services and assembly concerns.

**Architecture:** Implement the split in three passes. First create a runtime boundary inside `torque-harness` and leave `kernel_bridge` as a compatibility shell. Then extract the runtime host and generic execution-environment helpers into `crates/torque-runtime`. Finally reassemble `torque-harness` around runtime interfaces and tighten `torque-kernel` exports so the three-layer model is explicit.

**Tech Stack:** Rust 2021 workspace, `torque-kernel`, `torque-harness`, `llm`, `checkpointer`, Axum, SQLx/Postgres, existing focused regression tests under `crates/torque-harness/tests`

---

## File Map

### Existing files to modify

- `Cargo.toml`
  Add `crates/torque-runtime` to the workspace at the extraction stage.
- `crates/torque-kernel/src/lib.rs`
  Keep the public kernel surface explicit and limited to contract types plus the in-memory reference runtime.
- `crates/torque-kernel/src/runtime.rs`
  Clarify that the kernel runtime is a contract surface plus reference implementation, not the full production runtime host.
- `crates/torque-harness/Cargo.toml`
  Add a dependency on `torque-runtime` after the crate exists.
- `crates/torque-harness/src/lib.rs`
  Export the transitional `runtime` module and later re-export `torque-runtime`.
- `crates/torque-harness/src/app.rs`
  Build services around runtime dependencies rather than direct kernel-bridge implementation files.
- `crates/torque-harness/src/service/mod.rs`
  Keep product services and assembly, but stop owning generic runtime logic.
- `crates/torque-harness/src/service/session.rs`
  Depend on runtime interfaces for chat execution and context shaping.
- `crates/torque-harness/src/service/run.rs`
  Depend on runtime interfaces for v1 run execution.
- `crates/torque-harness/src/service/tool.rs`
  Keep harness-specific tool assembly only.
- `crates/torque-harness/src/service/tool_offload.rs`
  Migrate generic offload logic into the runtime layer, leaving only harness adapters if still needed.
- `crates/torque-harness/src/service/context_compaction.rs`
  Migrate generic compaction policy into the runtime layer, leaving only harness adapters if still needed.
- `crates/torque-harness/src/service/vfs.rs`
  Migrate generic routed VFS behavior into the runtime layer.
- `crates/torque-harness/src/tools/vfs.rs`
  Keep argument parsing and policy enforcement in harness while calling runtime-provided VFS logic.
- `crates/torque-harness/src/infra/tool_registry.rs`
  Keep only harness-owned registration concerns or shared execution-context support required by the runtime interface.
- `crates/torque-harness/src/kernel_bridge/mod.rs`
  Turn into a compatibility shim that re-exports the new runtime path during migration.
- `crates/torque-harness/src/kernel_bridge/runtime.rs`
  Replace implementation with a thin re-export wrapper once `runtime/host.rs` exists.
- `crates/torque-harness/src/kernel_bridge/checkpointer.rs`
  Replace implementation with a thin re-export wrapper once `runtime/checkpoint.rs` exists.
- `crates/torque-harness/src/kernel_bridge/events.rs`
  Replace implementation with a thin re-export wrapper once `runtime/events.rs` exists.
- `crates/torque-harness/src/kernel_bridge/mapping.rs`
  Replace implementation with a thin re-export wrapper once `runtime/mapping.rs` exists.

### New files to create

- `crates/torque-harness/src/runtime/mod.rs`
  Transitional runtime module inside harness.
- `crates/torque-harness/src/runtime/host.rs`
  Transitional runtime host.
- `crates/torque-harness/src/runtime/environment.rs`
  Transitional runtime ports.
- `crates/torque-harness/src/runtime/events.rs`
  Transitional event sink glue.
- `crates/torque-harness/src/runtime/checkpoint.rs`
  Transitional checkpoint sink glue.
- `crates/torque-harness/src/runtime/mapping.rs`
  Transitional request and context mapping helpers.
- `crates/torque-runtime/Cargo.toml`
  New runtime crate manifest.
- `crates/torque-runtime/src/lib.rs`
  Public runtime exports.
- `crates/torque-runtime/src/host.rs`
  Runtime host implementation.
- `crates/torque-runtime/src/environment.rs`
  Runtime dependency traits.
- `crates/torque-runtime/src/events.rs`
  Runtime event helpers.
- `crates/torque-runtime/src/checkpoint.rs`
  Runtime checkpoint helpers.
- `crates/torque-runtime/src/context.rs`
  Runtime context shaping types and policies.
- `crates/torque-runtime/src/tools.rs`
  Tool execution traits and adapters.
- `crates/torque-runtime/src/vfs.rs`
  Generic routed VFS behavior.
- `crates/torque-runtime/src/offload.rs`
  Generic tool-output offload behavior.

### Files to consult while implementing

- `docs/superpowers/specs/2026-04-25-torque-runtime-layer-design.md`
- `AGENTS.md`
- `docs/learn.md`
- `crates/torque-kernel/src/lib.rs`
- `crates/torque-kernel/src/runtime.rs`
- `crates/torque-harness/src/lib.rs`
- `crates/torque-harness/src/app.rs`
- `crates/torque-harness/src/service/mod.rs`
- `crates/torque-harness/src/service/session.rs`
- `crates/torque-harness/src/service/run.rs`
- `crates/torque-harness/src/kernel_bridge/mod.rs`
- `crates/torque-harness/src/kernel_bridge/runtime.rs`

---

### Task 1: Create a Transitional `runtime/` Module Inside `torque-harness`

**Files:**
- Create: `crates/torque-harness/src/runtime/mod.rs`
- Modify: `crates/torque-harness/src/lib.rs`
- Modify: `crates/torque-harness/src/kernel_bridge/mod.rs`
- Test: `crates/torque-harness/tests/runtime_module_exports_tests.rs`

- [ ] **Step 1: Write the failing export test**

Create `crates/torque-harness/tests/runtime_module_exports_tests.rs` with a compile-oriented test that imports both:

```rust
use torque_harness::kernel_bridge;
use torque_harness::runtime;
```

and asserts the new module path is available.

- [ ] **Step 2: Run the export test to verify it fails**

Run: `cargo test -p torque-harness --test runtime_module_exports_tests -- --nocapture`
Expected: FAIL because `torque_harness::runtime` does not exist yet.

- [ ] **Step 3: Add the transitional module scaffold**

Create `crates/torque-harness/src/runtime/mod.rs`:

```rust
pub mod checkpoint;
pub mod environment;
pub mod events;
pub mod host;
pub mod mapping;
```

- [ ] **Step 4: Export the runtime module from harness**

Update `crates/torque-harness/src/lib.rs` to include:

```rust
pub mod runtime;
```

- [ ] **Step 5: Turn `kernel_bridge` into a compatibility shell**

Update `crates/torque-harness/src/kernel_bridge/mod.rs` so new code can import `crate::runtime` while old code still compiles:

```rust
pub mod checkpointer;
pub mod events;
pub mod mapping;
pub mod runtime;
pub mod v1_mapping;
```

Keep existing exports in place for now.

- [ ] **Step 6: Re-run the export test**

Run: `cargo test -p torque-harness --test runtime_module_exports_tests -- --nocapture`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/torque-harness/src/runtime/mod.rs \
  crates/torque-harness/src/lib.rs \
  crates/torque-harness/src/kernel_bridge/mod.rs \
  crates/torque-harness/tests/runtime_module_exports_tests.rs
git commit -F - <<'EOF'
Create a runtime namespace inside torque-harness

This establishes a first-class runtime module so execution-environment
code has an explicit home before it is extracted into its own crate.

Constraint: Existing kernel_bridge imports must keep compiling during migration
Rejected: Extract a new crate before proving the boundary | too much churn too early
Confidence: high
Scope-risk: narrow
Directive: Add new execution-environment code under runtime, not kernel_bridge
Tested: cargo test -p torque-harness --test runtime_module_exports_tests -- --nocapture
Not-tested: full torque-harness test suite
EOF
```

---

### Task 2: Move the Runtime Host and Ports Under the Transitional Harness Runtime Module

**Files:**
- Create: `crates/torque-harness/src/runtime/host.rs`
- Create: `crates/torque-harness/src/runtime/environment.rs`
- Create: `crates/torque-harness/src/runtime/events.rs`
- Create: `crates/torque-harness/src/runtime/checkpoint.rs`
- Create: `crates/torque-harness/src/runtime/mapping.rs`
- Modify: `crates/torque-harness/src/kernel_bridge/runtime.rs`
- Modify: `crates/torque-harness/src/kernel_bridge/checkpointer.rs`
- Modify: `crates/torque-harness/src/kernel_bridge/events.rs`
- Modify: `crates/torque-harness/src/kernel_bridge/mapping.rs`
- Modify: `crates/torque-harness/src/service/session.rs`
- Modify: `crates/torque-harness/src/service/run.rs`
- Test: `crates/torque-harness/tests/runtime_host_path_tests.rs`
- Test: `crates/torque-harness/tests/agent_runner_tests.rs`

- [ ] **Step 1: Write the failing runtime-host path test**

Create `crates/torque-harness/tests/runtime_host_path_tests.rs` that constructs the host through `torque_harness::runtime::host::KernelRuntimeHandle`.

- [ ] **Step 2: Run the focused tests to verify they fail**

Run: `cargo test -p torque-harness --test runtime_host_path_tests -- --nocapture`
Run: `cargo test -p torque-harness --test agent_runner_tests -- --nocapture`
Expected: FAIL because the host and ports do not exist under `crate::runtime`.

- [ ] **Step 3: Define runtime-environment ports**

Create `crates/torque-harness/src/runtime/environment.rs` with traits for:

```rust
#[async_trait::async_trait]
pub trait RuntimeEventSink: Send + Sync {}

#[async_trait::async_trait]
pub trait RuntimeCheckpointSink: Send + Sync {}

#[async_trait::async_trait]
pub trait RuntimeToolExecutor: Send + Sync {}
```

Use real method signatures based on the existing host behavior in `kernel_bridge/runtime.rs`.

- [ ] **Step 4: Move the host implementation**

Move the current host logic from `crates/torque-harness/src/kernel_bridge/runtime.rs` into `crates/torque-harness/src/runtime/host.rs`.

- [ ] **Step 5: Leave compatibility wrappers behind**

Replace the contents of:
- `crates/torque-harness/src/kernel_bridge/runtime.rs`
- `crates/torque-harness/src/kernel_bridge/checkpointer.rs`
- `crates/torque-harness/src/kernel_bridge/events.rs`
- `crates/torque-harness/src/kernel_bridge/mapping.rs`

with thin re-exports, for example:

```rust
pub use crate::runtime::host::*;
```

- [ ] **Step 6: Rewire session and run services**

Update `crates/torque-harness/src/service/session.rs` and `crates/torque-harness/src/service/run.rs` to import runtime code from `crate::runtime`, not `crate::kernel_bridge`.

- [ ] **Step 7: Re-run the focused tests**

Run: `cargo test -p torque-harness --test runtime_host_path_tests -- --nocapture`
Run: `cargo test -p torque-harness --test agent_runner_tests -- --nocapture`
Expected: PASS

- [ ] **Step 8: Commit**

```bash
git add crates/torque-harness/src/runtime \
  crates/torque-harness/src/kernel_bridge/runtime.rs \
  crates/torque-harness/src/kernel_bridge/checkpointer.rs \
  crates/torque-harness/src/kernel_bridge/events.rs \
  crates/torque-harness/src/kernel_bridge/mapping.rs \
  crates/torque-harness/src/service/session.rs \
  crates/torque-harness/src/service/run.rs \
  crates/torque-harness/tests/runtime_host_path_tests.rs
git commit -F - <<'EOF'
Move the runtime host behind an explicit harness runtime module

This makes the execution-environment boundary concrete before extracting
it into a separate crate and keeps kernel_bridge as a temporary
compatibility path only.

Constraint: Session and run flows must keep their current behavior during migration
Rejected: Rename every import at once without shims | migration risk with no user benefit
Confidence: high
Scope-risk: moderate
Directive: Keep kernel_bridge as a compatibility wrapper, not an implementation home
Tested: cargo test -p torque-harness --test runtime_host_path_tests -- --nocapture; cargo test -p torque-harness --test agent_runner_tests -- --nocapture
Not-tested: full API and DB-backed suites
EOF
```

---

### Task 3: Extract `crates/torque-runtime`

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/torque-runtime/Cargo.toml`
- Create: `crates/torque-runtime/src/lib.rs`
- Create: `crates/torque-runtime/src/host.rs`
- Create: `crates/torque-runtime/src/environment.rs`
- Create: `crates/torque-runtime/src/events.rs`
- Create: `crates/torque-runtime/src/checkpoint.rs`
- Create: `crates/torque-runtime/src/context.rs`
- Create: `crates/torque-runtime/src/tools.rs`
- Modify: `crates/torque-harness/Cargo.toml`
- Modify: `crates/torque-harness/src/runtime/mod.rs`
- Test: `crates/torque-runtime/src/lib.rs`

- [ ] **Step 1: Write the failing crate-local smoke test**

Create a small `#[cfg(test)]` smoke test in `crates/torque-runtime/src/lib.rs` that imports `torque_kernel::ExecutionRequest` and the new `RuntimeHost`.

- [ ] **Step 2: Run the new crate test to verify it fails**

Run: `cargo test -p torque-runtime --lib -- --nocapture`
Expected: FAIL because the crate is not in the workspace yet.

- [ ] **Step 3: Add the new crate to the workspace**

Update the root `Cargo.toml` members list to include:

```toml
"crates/torque-runtime",
```

- [ ] **Step 4: Create `crates/torque-runtime/Cargo.toml`**

Use a minimal dependency set:

```toml
[package]
name = "torque-runtime"
version.workspace = true
edition.workspace = true

[dependencies]
anyhow = { workspace = true }
async-trait = { workspace = true }
chrono = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true }
tracing = { workspace = true }
uuid = { workspace = true }
llm = { path = "../llm" }
checkpointer = { path = "../checkpointer" }
torque-kernel = { path = "../torque-kernel" }
```

- [ ] **Step 5: Move the runtime code into the new crate**

Move the code from the transitional harness runtime module into:
- `crates/torque-runtime/src/host.rs`
- `crates/torque-runtime/src/environment.rs`
- `crates/torque-runtime/src/events.rs`
- `crates/torque-runtime/src/checkpoint.rs`

- [ ] **Step 6: Re-export the runtime crate from harness**

Update `crates/torque-harness/src/runtime/mod.rs` to re-export from the new crate while migration continues:

```rust
pub use torque_runtime::*;
```

- [ ] **Step 7: Add the harness dependency**

Update `crates/torque-harness/Cargo.toml`:

```toml
torque-runtime = { path = "../torque-runtime" }
```

- [ ] **Step 8: Re-run the new crate test**

Run: `cargo test -p torque-runtime --lib -- --nocapture`
Expected: PASS

- [ ] **Step 9: Commit**

```bash
git add Cargo.toml \
  crates/torque-runtime \
  crates/torque-harness/Cargo.toml \
  crates/torque-harness/src/runtime/mod.rs
git commit -F - <<'EOF'
Extract the replaceable runtime environment into its own crate

This creates torque-runtime so the execution environment can evolve
independently from kernel contracts and product-facing harness code.

Constraint: The new crate must not absorb API or repository concerns
Rejected: Leave runtime code in torque-harness permanently | blocks multiple harness variants
Confidence: high
Scope-risk: moderate
Directive: torque-runtime may depend on kernel contracts but must not define competing execution objects
Tested: cargo test -p torque-runtime --lib -- --nocapture
Not-tested: downstream harness integration
EOF
```

---

### Task 4: Reassemble `torque-harness` Around `torque-runtime`

**Files:**
- Modify: `crates/torque-harness/src/app.rs`
- Modify: `crates/torque-harness/src/service/mod.rs`
- Modify: `crates/torque-harness/src/service/session.rs`
- Modify: `crates/torque-harness/src/service/run.rs`
- Modify: `crates/torque-harness/src/lib.rs`
- Test: `crates/torque-harness/tests/context_compaction_tests.rs`
- Test: `crates/torque-harness/tests/tool_offload_tests.rs`
- Test: `crates/torque-harness/tests/runtime_host_path_tests.rs`

- [ ] **Step 1: Write failing integration-oriented assertions**

Extend the focused tests so they construct `SessionService` and `RunService` through the harness assembly path and prove the runtime host now comes from `torque-runtime`.

- [ ] **Step 2: Run the focused harness tests to verify they fail**

Run: `cargo test -p torque-harness --test context_compaction_tests -- --nocapture`
Run: `cargo test -p torque-harness --test tool_offload_tests -- --nocapture`
Run: `cargo test -p torque-harness --test runtime_host_path_tests -- --nocapture`
Expected: FAIL because harness assembly still owns old runtime details.

- [ ] **Step 3: Introduce a runtime-host dependency at service construction**

Update `crates/torque-harness/src/service/mod.rs` and `crates/torque-harness/src/app.rs` so runtime dependencies are assembled once and passed into `SessionService` and `RunService`.

- [ ] **Step 4: Update `SessionService`**

Refactor `crates/torque-harness/src/service/session.rs` so it depends on a runtime-host abstraction rather than constructing a `KernelRuntimeHandle` directly inside `chat()`.

- [ ] **Step 5: Update `RunService`**

Refactor `crates/torque-harness/src/service/run.rs` so it depends on a runtime-host abstraction rather than importing host implementation details from the old bridge path.

- [ ] **Step 6: Keep harness ownership narrow**

Verify the harness still owns:
- repositories
- API wiring
- session/run/team product services

and no longer owns generic runtime-host implementation code.

- [ ] **Step 7: Re-run the focused harness tests**

Run: `cargo test -p torque-harness --test context_compaction_tests -- --nocapture`
Run: `cargo test -p torque-harness --test tool_offload_tests -- --nocapture`
Run: `cargo test -p torque-harness --test runtime_host_path_tests -- --nocapture`
Expected: PASS

- [ ] **Step 8: Commit**

```bash
git add crates/torque-harness/src/app.rs \
  crates/torque-harness/src/service/mod.rs \
  crates/torque-harness/src/service/session.rs \
  crates/torque-harness/src/service/run.rs \
  crates/torque-harness/src/lib.rs
git commit -F - <<'EOF'
Reassemble harness services around the extracted runtime

This keeps torque-harness focused on product services and repository
assembly while delegating the execution environment to torque-runtime.

Constraint: Existing session and run surfaces must keep working during migration
Rejected: Move session and run services into torque-runtime | they remain product orchestration
Confidence: medium
Scope-risk: moderate
Directive: New service code should depend on runtime interfaces, not runtime implementation files
Tested: cargo test -p torque-harness --test context_compaction_tests -- --nocapture; cargo test -p torque-harness --test tool_offload_tests -- --nocapture; cargo test -p torque-harness --test runtime_host_path_tests -- --nocapture
Not-tested: full v1 API and DB-backed end-to-end suites
EOF
```

---

### Task 5: Move Generic VFS, Offload, and Context Logic Into `torque-runtime`

**Files:**
- Create: `crates/torque-runtime/src/vfs.rs`
- Create: `crates/torque-runtime/src/offload.rs`
- Modify: `crates/torque-runtime/src/context.rs`
- Modify: `crates/torque-harness/src/service/vfs.rs`
- Modify: `crates/torque-harness/src/service/tool_offload.rs`
- Modify: `crates/torque-harness/src/service/context_compaction.rs`
- Modify: `crates/torque-harness/src/tools/vfs.rs`
- Test: `crates/torque-harness/tests/vfs_tools_tests.rs`
- Test: `crates/torque-harness/tests/delegation_packet_tests.rs`

- [ ] **Step 1: Write failing focused tests**

Extend the VFS and delegation-focused tests so they prove the harness still behaves correctly when routed VFS, offload policy, and compact-summary policy come from `torque-runtime`.

- [ ] **Step 2: Run the focused tests to verify they fail**

Run: `cargo test -p torque-harness --test vfs_tools_tests -- --nocapture`
Run: `cargo test -p torque-harness --test delegation_packet_tests -- --nocapture`
Expected: FAIL because the generic runtime helpers still live under harness.

- [ ] **Step 3: Move routed VFS**

Move the generic `RoutedVfs` and backend-neutral structs into `crates/torque-runtime/src/vfs.rs`.

- [ ] **Step 4: Move offload policy**

Move generic tool-output offload thresholds and result shaping into `crates/torque-runtime/src/offload.rs`.

- [ ] **Step 5: Move compact-summary policy**

Move compact-summary data types and policy into `crates/torque-runtime/src/context.rs`.

- [ ] **Step 6: Leave thin harness adapters**

Refactor `crates/torque-harness/src/tools/vfs.rs`, `service/vfs.rs`, `service/tool_offload.rs`, and `service/context_compaction.rs` so they only do:
- tool argument parsing
- policy enforcement
- runtime adapter calls

- [ ] **Step 7: Re-run the focused tests**

Run: `cargo test -p torque-harness --test vfs_tools_tests -- --nocapture`
Run: `cargo test -p torque-harness --test delegation_packet_tests -- --nocapture`
Expected: PASS

- [ ] **Step 8: Commit**

```bash
git add crates/torque-runtime/src/vfs.rs \
  crates/torque-runtime/src/offload.rs \
  crates/torque-runtime/src/context.rs \
  crates/torque-harness/src/service/vfs.rs \
  crates/torque-harness/src/service/tool_offload.rs \
  crates/torque-harness/src/service/context_compaction.rs \
  crates/torque-harness/src/tools/vfs.rs
git commit -F - <<'EOF'
Move generic execution-environment helpers into torque-runtime

This leaves harness adapters focused on product policy and tool surfaces
while generic VFS, offload, and context behavior live in the runtime layer.

Constraint: Harness tool names and request shapes must stay stable
Rejected: Duplicate the helpers across future harnesses | drift starts immediately
Confidence: medium
Scope-risk: moderate
Directive: Keep tool parsing and policy in harness, keep environment behavior in runtime
Tested: cargo test -p torque-harness --test vfs_tools_tests -- --nocapture; cargo test -p torque-harness --test delegation_packet_tests -- --nocapture
Not-tested: broader run/session integration paths
EOF
```

---

### Task 6: Tighten `torque-kernel` Exports and Final Verification

**Files:**
- Modify: `crates/torque-kernel/src/runtime.rs`
- Modify: `crates/torque-kernel/src/lib.rs`
- Test: `crates/torque-kernel/tests/kernel_contracts.rs`
- Test: `crates/torque-harness/tests/agent_runner_tests.rs`
- Test: `crates/torque-harness/tests/filesystem_permissions_tests.rs`

- [ ] **Step 1: Write the failing kernel-contract assertion**

Extend `crates/torque-kernel/tests/kernel_contracts.rs` to assert that the public kernel runtime surface exposes contract types plus the in-memory reference runtime, but does not imply ownership of the production runtime environment.

- [ ] **Step 2: Run the focused kernel and harness tests to verify they fail**

Run: `cargo test -p torque-kernel --test kernel_contracts -- --nocapture`
Run: `cargo test -p torque-harness --test agent_runner_tests -- --nocapture`
Run: `cargo test -p torque-harness --test filesystem_permissions_tests -- --nocapture`
Expected: FAIL because comments and exports still reflect the old ambiguity.

- [ ] **Step 3: Clarify `runtime.rs`**

Update `crates/torque-kernel/src/runtime.rs` comments and documentation so:
- `KernelRuntime`, `RuntimeStore`, `RuntimeCommand`, and `ResumeSignal` stay as kernel contracts
- `InMemoryKernelRuntime` and `InMemoryRuntimeStore` are described as reference implementations

- [ ] **Step 4: Tighten `lib.rs` exports**

Keep the public kernel surface explicit and avoid broad wording that suggests production runtime behavior lives there.

- [ ] **Step 5: Re-run the focused tests**

Run: `cargo test -p torque-kernel --test kernel_contracts -- --nocapture`
Run: `cargo test -p torque-harness --test agent_runner_tests -- --nocapture`
Run: `cargo test -p torque-harness --test filesystem_permissions_tests -- --nocapture`
Expected: PASS

- [ ] **Step 6: Run final regression commands**

Run:
- `cargo test -p torque-runtime --lib -- --nocapture`
- `cargo test -p torque-kernel --test kernel_contracts -- --nocapture`
- `cargo test -p torque-harness --test runtime_module_exports_tests -- --nocapture`
- `cargo test -p torque-harness --test runtime_host_path_tests -- --nocapture`
- `cargo test -p torque-harness --test agent_runner_tests -- --nocapture`
- `cargo test -p torque-harness --test context_compaction_tests -- --nocapture`
- `cargo test -p torque-harness --test tool_offload_tests -- --nocapture`
- `cargo test -p torque-harness --test vfs_tools_tests -- --nocapture`
- `cargo test -p torque-harness --test delegation_packet_tests -- --nocapture`
- `cargo test -p torque-harness --test filesystem_permissions_tests -- --nocapture`

Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/torque-kernel/src/runtime.rs \
  crates/torque-kernel/src/lib.rs \
  crates/torque-kernel/tests/kernel_contracts.rs
git commit -F - <<'EOF'
Clarify kernel runtime as a contract and reference surface

This completes the runtime-layer split by keeping stable execution
contracts in torque-kernel and pushing production runtime behavior into
torque-runtime.

Constraint: Kernel contracts must remain usable for tests and local reference execution
Rejected: Remove the in-memory runtime now | still useful as a contract demonstrator
Confidence: high
Scope-risk: narrow
Directive: Add production runtime behavior in torque-runtime, not torque-kernel
Tested: cargo test -p torque-runtime --lib -- --nocapture; cargo test -p torque-kernel --test kernel_contracts -- --nocapture; cargo test -p torque-harness --test runtime_module_exports_tests -- --nocapture; cargo test -p torque-harness --test runtime_host_path_tests -- --nocapture; cargo test -p torque-harness --test agent_runner_tests -- --nocapture; cargo test -p torque-harness --test context_compaction_tests -- --nocapture; cargo test -p torque-harness --test tool_offload_tests -- --nocapture; cargo test -p torque-harness --test vfs_tools_tests -- --nocapture; cargo test -p torque-harness --test delegation_packet_tests -- --nocapture; cargo test -p torque-harness --test filesystem_permissions_tests -- --nocapture
Not-tested: full torque-harness package test suite and live DB-backed APIs
EOF
```

---

## Risks and Mitigations

- Import churn can hide behavior regressions.
  Mitigation: keep `kernel_bridge` as a compatibility layer until the new runtime path is proven.

- `torque-runtime` can accidentally absorb harness-only concerns.
  Mitigation: reject dependencies from `torque-runtime` to `axum`, harness API modules, or repository implementations.

- `torque-kernel` can keep implying ownership of production runtime behavior.
  Mitigation: tighten comments, exports, and tests in the final task.

- Focused tests can pass while broader API paths drift.
  Mitigation: after this plan completes, schedule a follow-up verification pass for DB-backed end-to-end API flows.

## Verification Steps

Run these before claiming completion:

- `cargo test -p torque-runtime --lib -- --nocapture`
- `cargo test -p torque-kernel --test kernel_contracts -- --nocapture`
- `cargo test -p torque-harness --test runtime_module_exports_tests -- --nocapture`
- `cargo test -p torque-harness --test runtime_host_path_tests -- --nocapture`
- `cargo test -p torque-harness --test agent_runner_tests -- --nocapture`
- `cargo test -p torque-harness --test context_compaction_tests -- --nocapture`
- `cargo test -p torque-harness --test tool_offload_tests -- --nocapture`
- `cargo test -p torque-harness --test vfs_tools_tests -- --nocapture`
- `cargo test -p torque-harness --test delegation_packet_tests -- --nocapture`
- `cargo test -p torque-harness --test filesystem_permissions_tests -- --nocapture`

## Notes for Executors

- Do not skip the transitional `runtime/` module inside harness. It is the migration guardrail.
- Do not move `service/session.rs`, `service/run.rs`, or `service/team/**` into `torque-runtime`.
- Do not define alternate versions of kernel execution objects in `torque-runtime`.
- Keep commits lore-compliant and task-scoped.
