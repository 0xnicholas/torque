# Torque Architecture

## Overview

Torque is a Rust-based **Agent Runtime Kernel** with a higher-level **Harness** built on top of it.

The project is intentionally organized around a small number of architectural layers rather than a product-specific workflow model.

Torque is designed to be:

- agent-centric
- stateful
- recoverable
- policy-governed
- context-aware

Torque is not designed around:

- a built-in DAG-first planner/executor core
- a mandatory workflow DSL
- a graph node as the kernel execution primitive
- chat transcript accumulation as the primary context model

The current architectural center is `AgentInstance`, not DAG.

---

## Current Repo Reality

The repository has two truths that must be held separately:

1. **Target architecture**
   defined by the spec set under `docs/superpowers/specs/`

2. **Current implementation**
   still centered on the early `session-agent` prototype and supporting crates

Current code paths of interest:

- `crates/llm`
  OpenAI-compatible client, streaming, tool-call primitives
- `crates/session-agent`
  current product-facing MVP path for a single persistent agent session
- `crates/checkpointer`
  emerging checkpoint abstraction

When code and architecture docs diverge:

- code is authoritative for current runtime behavior
- specs are authoritative for intended architecture and new implementation direction

---

## Layered Architecture

Torque should be understood as six related layers.

### 1. Kernel Execution

Defines the core runtime execution contract:

- `ExecutionRequest`
- `AgentInstance`
- `Task`
- `ExecutionResult`
- `DelegationRequest`
- `DelegationResult`
- `ApprovalRequest`

This layer is responsible for:

- execution entry
- instance lifecycle
- task ownership
- delegation
- streaming progression
- suspension and resumption

### 2. Capability Layer

Defines how upper layers refer to abilities without hard-binding to concrete implementations.

Core objects:

- `CapabilityRef`
- `CapabilityProfile`
- `CapabilityRegistryBinding`
- `CapabilityResolution`

This layer separates:

- what ability is requested
- what that ability means
- which implementations can satisfy it
- which candidates are valid in the current run

### 3. Policy Layer

Defines governance as an evaluated rule system rather than scattered booleans.

Core idea:

`policy inputs -> dimensional evaluation -> conservative merge -> PolicyDecision`

Initial policy dimensions include:

- approval
- visibility
- delegation
- resource
- memory
- tool

### 4. Context and State Layer

Defines how execution receives and manages context.

This layer explicitly rejects “full chat history as context” as the default model.

Instead, Torque uses:

- layered state
- structured summaries
- explicit references
- derived execution packets
- lazy loading
- periodic state convergence / compaction

### 5. Harness / Team Layer

Defines higher-level orchestration on top of the kernel.

Core objects:

- `TeamDefinition`
- `TeamInstance`
- `TeamTask`
- `SharedTaskState`
- `TeamEvent`

Default collaboration model:

`Supervisor -> Subagent`

This layer is responsible for:

- triage and coordination
- selector-governed dynamic expansion
- shared-state publish
- team-local approval routing
- collaboration-level recovery

### 6. Recovery Layer

Defines recovery as:

- event truth
- checkpoint acceleration
- replay
- reconciliation

This applies consistently across kernel and team layers.

---

## Core Runtime Model

Torque is **instance-centric**.

The core execution chain is:

`ExecutionRequest`
-> create or continue `AgentInstance`
-> assign or continue `Task`
-> instance executes
-> runtime emits `ExecutionResult`

Important boundaries:

- `ExecutionRequest` is a kernel intent object, not merely an HTTP payload
- `AgentInstance` is the execution owner
- `Task` is the current work item
- `ExecutionResult` is a progression result, not just a final text response

A single instance may outlive a task, but it should have only one active primary task at a time.

---

## Delegation Model

Delegation is a kernel contract, not informal agent-to-agent chat.

The standard relationship is:

`Parent AgentInstance`
-> `DelegationRequest`
-> `Child AgentInstance`
-> `DelegationResult`

Important rules:

- the child does not inherit the parent’s full private history
- the child receives a constrained execution packet, not the whole transcript
- child output is returned to the parent, not automatically published
- completion is not automatic acceptance

Conceptually, parent-side handling should evaluate:

1. contract validity
2. governance and safety
3. work-product fitness
4. integration and publishability

---

## Context Model

Torque treats context as a **state management system**, not a giant prompt buffer.

The effective model is layered:

- global stable layer
- team/shared coordination layer
- agent-instance private layer
- external knowledge layer
- execution-time `TaskPacket` layer

### TaskPacket

`TaskPacket` is:

- a narrow execution envelope
- derived from authoritative state
- assembled by the assigning authority
- consumed by the executing instance

It is not:

- the source of truth
- a full transcript
- a full shared-state dump

Default behavior should prefer:

- refs over copied bodies
- structured summaries over long prose
- lazy retrieval over eager stuffing

---

## Context Planes

Torque keeps three planes separate:

- `ExternalContextRef`
  external reference plane
- `Artifact`
  execution result plane
- `Memory`
  semantic retention plane

These planes interact only through explicit, policy-governed transitions.

Important non-rules:

- external context does not automatically become artifact
- artifact does not automatically become memory
- team publish does not automatically become memory write

This separation is important for:

- token control
- replayability
- ownership boundaries
- recovery correctness

---

## Team Model

`Team` is a harness-layer capability, not a kernel primitive.

Key principles:

- supervisor-led by default
- governance-first
- dynamic expansion through selectors
- explicit shared-state publication
- team-local approval routing

`SharedTaskState` is intentionally narrow. It should hold accepted coordination state such as:

- accepted facts
- decision summaries
- blocker summaries
- progress summaries
- artifact refs

It is not a transcript store and not a blob store.

---

## Capability Model

Torque separates ability identity from implementation identity.

The conceptual chain is:

`CapabilityRef`
-> resolves to `CapabilityProfile`
-> looks up `CapabilityRegistryBinding`
-> yields `CapabilityResolution`
-> upper layer chooses a candidate `AgentDefinition`

This keeps:

- authoring stable
- capability meaning explicit
- implementation choice replaceable
- runtime resolution auditable

Upper layers should usually depend on capability-level references, not directly on concrete agent definitions.

---

## Policy Model

Torque treats policy as a first-class governance system.

Policy is not just configuration spread across structs.

Instead:

- multiple policy sources contribute input
- evaluation is done per dimension
- same-dimension merge is conservative by default
- runtime receives a structured `PolicyDecision`

Important rule:

`policy source hierarchy is not the same as universal override hierarchy`

Different layers may speak about different dimensions, and no single lower layer should silently override everything else.

---

## Recovery Model

Torque recovery follows one consistent rule:

- `Event` is the truth source
- `Checkpoint` is the recovery acceleration layer
- `Recovery` is restore + replay + reconciliation

This means:

- checkpoints are not canonical truth
- restoring a snapshot is not enough
- recovery must reconcile against current storage/runtime reality

This applies to:

- kernel instance recovery
- team recovery
- long-running execution continuity

---

## Current MVP Path

The current early product-facing slice is the **Session Agent MVP**.

It is intentionally narrow:

- single agent
- persistent session
- multi-turn chat
- SSE streaming
- bounded context window
- optional minimal safe tool support

It is meant to demonstrate:

`Torque already supports a stateful agent experience`

It is not meant to demonstrate:

- team orchestration
- approval UI
- recovery UI
- capability registry UX
- full policy exposure

The current MVP implementation path primarily lives in:

- `crates/session-agent`
- `crates/llm`

---

## Authoritative Documents

The architecture is split across a small set of focused documents.

### Architecture and contracts

- [Torque Agent Runtime / Harness Design](./superpowers/specs/2026-04-08-torque-agent-runtime-harness-design.md)
- [Torque Kernel Execution Contract Design](./superpowers/specs/2026-04-08-torque-kernel-execution-contract-design.md)
- [Torque Recovery Core Design](./superpowers/specs/2026-04-08-torque-recovery-core-design.md)

### Capability and governance

- [Torque Capability Registry Model Design](./superpowers/specs/2026-04-08-torque-capability-registry-model-design.md)
- [Torque Policy Model Design](./superpowers/specs/2026-04-08-torque-policy-model-design.md)

### Context and data planes

- [Torque Context State Model Design](./superpowers/specs/2026-04-08-torque-context-state-model-design.md)
- [Torque Context Planes Design](./superpowers/specs/2026-04-08-torque-context-planes-design.md)

### Team and collaboration

- [Torque Agent Team Design](./superpowers/specs/2026-04-08-torque-agent-team-design.md)

### MVP

- [Session Agent MVP Design](./superpowers/specs/2026-04-08-session-agent-mvp-design.md)
- [Session Agent MVP Implementation Plan](./superpowers/plans/2026-04-08-session-agent-mvp.md)

### High-level concept index

- [Torque Concepts](./learn.md)

---

## Practical Guidance

When making changes in this repo:

- read the specific spec for the layer you are touching
- keep prototype-era code and target architecture mentally separate
- prefer explicit contracts over prompt-only conventions when persistence, auditing, or recovery matters
- keep context narrow and structured
- avoid reintroducing DAG-first or transcript-first assumptions unless the architecture explicitly changes

If a change spans multiple layers, update the relevant design docs first or keep the implementation aligned with the current contracts.
