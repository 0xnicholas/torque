# Torque Runtime Layer Design

> Date: 2026-04-25  
> Status: Draft  
> Scope: `torque-kernel` / `torque-runtime` / `torque-harness` layer split, dependency rules, migration constraints  
> Goal: Make Torque's execution environment explicit as a replaceable runtime layer while keeping kernel contracts stable and harness services product-facing.

---

## 1. Overview

Torque is currently described as an **Agent Runtime Kernel** with a higher-level **Harness** on top, but the implementation still blends two different concerns inside `torque-harness`:

- product-facing services and APIs
- execution-environment logic that every harness would likely need

That second category includes the runtime host, LLM/tool loop integration, event recording, checkpoint glue, output offload, and context compaction. Those are not API details, and they are not kernel contracts either. They are runtime-environment concerns.

This design introduces an explicit three-layer model:

`torque-kernel`
-> stable execution contracts and state semantics

`torque-runtime`
-> replaceable execution environment built on those contracts

`torque-harness`
-> product-facing services, orchestration surfaces, repositories, and APIs

The intent is not to redesign the kernel object model. The intent is to move execution-environment logic out of the harness without pushing product concerns down into the kernel.

---

## 2. Design Goals

- Keep `torque-kernel` focused on stable execution contracts and progression semantics
- Introduce `torque-runtime` as the place where a kernel actually runs in a concrete environment
- Keep `torque-harness` focused on product services, HTTP/API surfaces, repositories, and harness-specific orchestration
- Preserve current harness APIs during migration
- Make future harness variants possible without duplicating runtime host logic
- Reduce the architectural ambiguity around `kernel_bridge`

## 3. Non-Goals

- This design does not introduce a new execution object model separate from the kernel
- This design does not move team orchestration into the runtime layer
- This design does not define a new database schema
- This design does not require immediate removal of the in-memory runtime from `torque-kernel`
- This design does not split `torque-harness` into multiple service crates in the first migration pass

---

## 4. Problem Statement

The current implementation shows three tensions:

### 4.1 Kernel owns contracts, but not the full running environment

`torque-kernel` already defines:

- `ExecutionRequest`
- `AgentInstance`
- `Task`
- `ExecutionResult`
- `RuntimeCommand`
- `ResumeSignal`
- `RuntimeStore`
- `KernelRuntime`

This is the right place for execution semantics. But production-oriented runtime behavior is still missing from the kernel and appears elsewhere.

### 4.2 Harness owns more than product services

`torque-harness` currently contains:

- HTTP and SSE APIs
- repository construction and database wiring
- session/run/team product services
- runtime host logic in `kernel_bridge`
- tool-loop integration
- generic output offload
- generic context compaction
- generic VFS execution surface

The first three belong in a harness. The latter five are reusable runtime-environment behavior.

### 4.3 `kernel_bridge` is doing the job of an unnamed runtime layer

The name `kernel_bridge` suggests an adapter, but in practice it is hosting real execution logic:

- it advances the kernel
- it calls the LLM
- it executes tools
- it records events
- it creates checkpoints
- it shapes tool results for further execution

That is no longer a narrow bridge. It is a runtime host.

---

## 5. Target Layer Model

Torque should converge on three layers.

### 5.1 Kernel Layer

`torque-kernel` owns execution semantics.

It defines:

- execution objects
- lifecycle states
- progression results
- delegation and approval contracts
- recovery and checkpoint semantics
- runtime commands and resume semantics
- the minimum store contract needed for state-machine advancement

The kernel answers:

`What do these execution objects mean?`

The kernel does not answer:

`How do we talk to a model provider?`
`How do we stream SSE to a browser?`
`How do we persist Postgres events?`

### 5.2 Runtime Layer

`torque-runtime` owns the execution environment for the kernel.

It provides:

- runtime host
- model-driver integration
- tool execution integration
- event sink integration
- checkpoint sink integration
- context shaping, compaction, and offload
- runtime-level VFS and file work surface
- approval handoff hooks

The runtime answers:

`In what environment does the kernel run?`

It is replaceable because different deployments may choose different:

- persistence strategies
- tool execution strategies
- model backends
- checkpoint backends
- context policies

### 5.3 Harness Layer

`torque-harness` owns product-facing and harness-specific concerns.

It provides:

- HTTP APIs
- SSE and webhook surfaces
- repository implementations
- session-oriented chat surface
- run/task APIs
- team supervisor and team workflows
- capability resolution and governance services that are product-layer decisions

The harness answers:

`How is this runtime exposed and orchestrated for a specific product surface?`

---

## 6. Ownership Boundaries

### 6.1 What stays in `torque-kernel`

Kernel-owned modules include:

- `execution`
- `agent_instance`
- `task`
- `task_packet`
- `delegation`
- `approval`
- `recovery`
- `runtime`
- `engine`
- IDs, errors, and core object definitions

Kernel-owned concepts include:

- `ExecutionRequest`
- `ExecutionResult`
- `AgentInstance`
- `Task`
- `TaskPacket`
- `DelegationRequest`
- `ApprovalRequest`
- `Checkpoint`
- `RecoveryView`
- `RuntimeCommand`
- `ResumeSignal`
- `RuntimeStore`

### 6.2 What moves to `torque-runtime`

Runtime-owned modules should include the execution-environment pieces currently spread across `torque-harness`:

- runtime host
- runtime environment ports
- event sink glue
- checkpoint sink glue
- request/context mapping helpers used by the host
- generic tool execution adapters
- output offload policy
- compact summary and context shaping policy
- routed VFS

These are runtime behaviors, not product APIs.

### 6.3 What stays in `torque-harness`

Harness-owned modules include:

- `api/**`
- `app.rs`
- `main.rs`
- `repository/**`
- session/run/task services
- team and supervisor services
- notification, webhook, and message-bus surfaces
- policy evaluation where the policy is a product-level governance decision

The harness may assemble runtime dependencies, but it should not own the generic runtime host implementation.

---

## 7. Dependency Rules

The allowed direction is:

`torque-kernel` <- `torque-runtime` <- `torque-harness`

### 7.1 `torque-kernel`

`torque-kernel` must not depend on:

- `llm`
- `axum`
- SQLx or repository implementations
- harness services
- HTTP, SSE, or webhook concerns

### 7.2 `torque-runtime`

`torque-runtime` may depend on:

- `torque-kernel`
- `llm`
- `checkpointer`
- generic async and serialization libraries

`torque-runtime` must not depend on:

- `axum`
- harness API modules
- harness repository implementations
- harness HTTP/message-bus/webhook surfaces
- product-specific team orchestration modules

### 7.3 `torque-harness`

`torque-harness` may depend on:

- `torque-kernel`
- `torque-runtime`
- repository implementations
- API and transport concerns

`torque-harness` is the correct place for:

- assembling runtime dependencies
- adapting product requests to runtime entry
- exposing runtime results to external clients

---

## 8. Runtime Ports

`torque-runtime` should be built around explicit ports so it does not absorb harness-specific implementations.

Recommended runtime ports:

- `ModelDriver`
  Drives model turns and tool-call streaming
- `ToolExecutor`
  Executes a named tool with runtime execution context
- `RuntimeEventSink`
  Persists or emits execution results and step events
- `RuntimeCheckpointSink`
  Creates and restores checkpoints
- `RuntimeContextBuilder`
  Builds derived execution context such as compact summaries and refs
- `OutputOffloader`
  Decides whether tool output stays inline or is offloaded
- `ApprovalGateway`
  Suspends and resumes work that requires external approval

These ports are environment contracts. They are not kernel contracts.

---

## 9. Kernel Runtime Status

`torque-kernel::runtime` should remain, but its role must be explicit.

It should continue to define:

- `KernelRuntime`
- `RuntimeCommand`
- `ResumeSignal`
- `RuntimeStore`

The in-memory runtime may remain in the kernel as:

- reference implementation
- test utility
- local contract demonstrator

It should not be treated as the production runtime host for Torque as a whole.

That distinction matters because otherwise future work will continue to overload the kernel with environment-specific behavior.

---

## 10. Migration Strategy

The migration should happen in three controlled phases.

### 10.1 Phase 1: Create a runtime boundary inside `torque-harness`

Before introducing a new crate, create a dedicated `runtime/` module inside `torque-harness`.

This phase should:

- move `kernel_bridge` logic under `runtime/`
- leave `kernel_bridge` as a compatibility shim
- introduce runtime ports
- update imports so new code references `crate::runtime`

This proves the boundary before crate extraction.

### 10.2 Phase 2: Extract `torque-runtime`

Once the boundary is clear:

- create `crates/torque-runtime`
- move the runtime host and generic runtime helpers into it
- keep `torque-harness` re-exports temporarily for migration stability

This phase should keep behavior stable while changing ownership.

### 10.3 Phase 3: Thin harness assembly

After extraction:

- make `torque-harness` assemble runtime dependencies rather than own runtime logic
- keep session/run/team services in the harness
- ensure new harness variants can depend on `torque-runtime`

This phase should reduce architectural ambiguity rather than maximize code motion.

---

## 11. Constraints and Invariants

The migration must preserve these invariants:

### 11.1 Do not create a second execution model

`torque-runtime` must not define alternate versions of:

- `ExecutionRequest`
- `ExecutionResult`
- `Task`
- `AgentInstance`
- `TaskPacket`

Those remain kernel-owned.

### 11.2 Do not move product orchestration into runtime

Team supervisor behavior, run APIs, and session product semantics stay in `torque-harness`.

### 11.3 Do not let kernel absorb environment details

Model drivers, VFS, offload policy, and event persistence details do not belong in `torque-kernel`.

### 11.4 Keep migration compatibility paths short-lived

`kernel_bridge` compatibility shims are useful during migration, but they should not become a permanent second home for runtime logic.

---

## 12. Acceptance Criteria

This design is considered successfully implemented when:

1. `torque-runtime` exists as a separate crate in the workspace.
2. The generic runtime host and environment helpers no longer live primarily under `torque-harness`.
3. `torque-harness` still exposes its current product surfaces without forcing callers to understand the new layering.
4. `torque-kernel` still owns the execution contract surface and does not gain model/provider/API concerns.
5. The dependency direction remains `kernel <- runtime <- harness`.
6. Focused runtime and harness regression tests still pass.

---

## 13. Open Questions

These questions should be answered in implementation, not by expanding this spec prematurely:

- Should `RuntimeStore` stay in `torque-kernel` permanently, or eventually split into a smaller kernel contract plus runtime-owned persistence composition?
- Which policy surfaces belong in runtime versus harness when a policy has both generic and product-specific aspects?
- When multiple harness services exist, should they stay in one crate with feature flags or split into multiple crates?

For now, the correct move is to establish the layer boundary first and postpone these second-order decisions until the extraction is working.
