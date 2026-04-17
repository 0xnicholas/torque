# Torque Architecture

## Overview

This document is the high-level architectural overview for Torque.

Its role is to make the global model legible:

- what Torque is trying to become
- which layers exist and where their boundaries are
- which runtime objects matter most
- which invariants should remain stable as the codebase evolves

This document is intended to be more complete than a short concept index, but it is still not the full source of truth for every layer-specific contract. Detailed design remains in the specs under `docs/superpowers/specs/`.

When code and docs diverge:

- code is authoritative for current runtime behavior
- the spec set is authoritative for intended architecture and new implementation direction

---

## Project Positioning

Torque is evolving toward:

- an **Agent Runtime Kernel**
- a higher-level **Harness** built on top of that kernel
- explicit models for execution, capability resolution, policy evaluation, context state management, team orchestration, and recovery

Torque is not currently modeled as:

- a DAG-first planner/executor system
- a workflow engine whose kernel abstraction is graph nodes
- a system where chat transcript accumulation is the primary context model
- a product-specific workspace runtime with hard-coded domain objects at the kernel layer

The current architectural center is `AgentInstance`, not DAG.

---

## Document Role

This file should be read as the architectural map for the repository.

It answers:

- what the major architectural layers are
- which objects belong to which layer
- how execution flows across those objects
- which separations are intentional and should not be casually collapsed

It does not attempt to fully define:

- exact persistence schemas
- full API payload definitions
- prompt templates
- retrieval algorithms
- storage engine details

For those details, use the relevant layer-specific spec.

---

## Current Repo Reality

The repository is in transition, so two truths must be held separately:

1. **Target architecture**
   defined by the spec set under `docs/superpowers/specs/`

2. **Current implementation**
   still centered on the early `agent-runtime-service` prototype and supporting crates

Current code paths of interest:

- `crates/llm`
  OpenAI-compatible client, streaming, and tool-call primitives
- `crates/agent-runtime-service`
  the current product-facing MVP slice for a single persistent agent session
- `crates/checkpointer`
  an emerging checkpoint abstraction

This means the implementation should not be read as fully architecture-complete yet. Some modules are prototype-era, while the architecture direction is more complete in the documents than in the code.

---

## How To Read Torque

The most stable mental model is:

`Upper-layer system`
-> `Harness`
-> `Kernel`
-> `Execution objects + policy/context/recovery surfaces`

In practical terms:

- the **kernel** owns execution semantics
- the **harness** owns higher-level collaboration and orchestration behavior
- **capability** decides how upper layers refer to abilities
- **policy** decides what is allowed
- **context/state** decides what execution can see
- **recovery** decides how long-running execution can be restored safely

This is a layered system, not a single all-purpose orchestration object.

---

## Layered Architecture

Torque should be understood as six related layers.

### 1. Kernel Execution

This layer defines the core runtime execution contract.

Primary objects:

- `ExecutionRequest`
- `AgentDefinition`
- `AgentInstance`
- `Task`
- `ExecutionResult`
- `DelegationRequest`
- `DelegationResult`
- `ApprovalRequest`
- `Artifact`
- `MemoryWriteCandidate`
- `ExternalContextRef`
- `Event`
- `Checkpoint`

This layer owns:

- execution entry
- instance lifecycle
- task ownership
- turn progression
- tool mediation
- delegation runtime
- suspension and resumption

This layer does not own:

- team collaboration semantics
- product-specific workflow models
- graph-native planning abstractions

### 2. Capability Layer

This layer defines how upper layers refer to abilities without hard-binding to concrete implementations.

Primary objects:

- `CapabilityRef`
- `CapabilityProfile`
- `CapabilityRegistryBinding`
- `CapabilityResolution`

This layer separates:

- what ability is requested
- what that ability means
- which implementations can satisfy it
- which candidates are valid in the current run

This layer should let upper layers depend on capability identity rather than directly on a specific `AgentDefinition`.

### 3. Policy Layer

This layer defines governance as evaluated policy, not scattered booleans.

Core idea:

`policy inputs -> dimensional evaluation -> conservative merge -> PolicyDecision`

Initial policy dimensions include:

- approval
- visibility
- delegation
- resource
- memory
- tool

This layer owns governance decisions. It should not be replaced by ad hoc flags spread across runtime structs.

### 4. Context and State Layer

This layer defines how execution receives and manages context.

It explicitly rejects "full chat history as the default context model".

Instead, Torque prefers:

- layered state
- structured summaries
- explicit references
- derived execution packets
- lazy loading
- periodic state convergence and compaction

This layer is about visibility, ownership, and shaping of execution input, not just prompt construction.

### 5. Harness / Team Layer

This layer defines higher-level orchestration on top of the kernel.

Primary objects:

- `TeamDefinition`
- `TeamInstance`
- `TeamTask`
- `SharedTaskState`
- `TeamEvent`

This layer owns:

- triage and coordination
- selector-governed dynamic expansion
- collaboration patterns
- shared-state publish
- team-local approval routing
- collaboration-level recovery behavior

Default collaboration model:

`Supervisor -> Subagent`

This layer should lower decisions into kernel-level runtime objects rather than importing team semantics into kernel execution types.

### 6. Recovery Layer

This layer defines how long-running execution is restored safely.

It is based on:

- event truth
- checkpoint acceleration
- replay
- reconciliation

Recovery applies across kernel and team layers, but it should not erase the ownership boundaries of those layers.

---

## Kernel vs Harness Boundary

This boundary is one of the most important separations in the system.

The kernel should remain neutral and reusable. It should not require Torque to understand a particular upper-layer workflow, playbook, or workspace model in order to execute work.

The harness may provide:

- planning and decomposition behavior
- team structures
- orchestration modes
- collaboration strategies
- built-in higher-level routines

But those decisions should lower into standard kernel objects such as:

- `ExecutionRequest`
- `DelegationRequest`
- `ApprovalRequest`
- `Artifact`
- `ExecutionResult`

Recommended boundary:

`Upper-layer orchestration`
-> `Harness decisions`
-> `ExecutionRequest / DelegationRequest / Artifact / ApprovalRequest`
-> `Kernel`
-> `ExecutionResult / Artifact / ApprovalRequest`
-> `Harness and upper layers`

If a concept is only meaningful for collaboration or orchestration, it probably belongs in the harness, not the kernel.

---

## Core Runtime Model

Torque is **instance-centric**.

The core execution chain is:

`ExecutionRequest`
-> create or continue `AgentInstance`
-> assign or continue `Task`
-> instance executes
-> runtime may produce `Artifact`, `DelegationRequest`, `ApprovalRequest`, `MemoryWriteCandidate`, `Checkpoint`
-> runtime emits `ExecutionResult`

Important boundaries:

- `ExecutionRequest` is a kernel intent object, not merely an HTTP payload
- `AgentDefinition` is the static execution template
- `AgentInstance` is the live execution owner
- `Task` is the current work item
- `ExecutionResult` is a progression result, not just a final text response

An `AgentInstance` may outlive an individual `Task`, but it should have only one active primary task at a time. If true concurrency is needed, the system should prefer multiple instances or explicit delegation.

---

## Core Object Relationships

The most important object boundaries are:

- `AgentDefinition`
  static identity, defaults, and policy surfaces for an agent type
- `AgentInstance`
  live execution owner created from an `AgentDefinition`
- `ExecutionRequest`
  kernel entry intent for what execution should happen now
- `Task`
  the specific work item the instance is currently trying to complete
- `ExecutionResult`
  the structured progression result of that execution

Supporting execution objects:

- `Artifact`
  execution output and downstream input surface
- `ExternalContextRef`
  neutral reference to externally owned context
- `MemoryWriteCandidate`
  nomination object for durable semantic retention
- `ApprovalRequest`
  auditable request for human or policy-mediated approval
- `Event`
  truth source for execution history
- `Checkpoint`
  acceleration layer for recovery

These objects should not be collapsed into one session-shaped blob.

---

## Execution Lifecycle

The recommended lifecycle is:

1. `Instantiate`
2. `Hydrate`
3. `Deliberate`
4. `Act`
5. `Checkpoint`
6. `Publish`
7. `Suspend / Resume / Complete / Fail`

Two important state machines remain separate:

- **Agent instance state**
  lifecycle of the live execution owner
- **Task state**
  lifecycle of the work item being attempted

Torque should not collapse instance state and task state into the same object or lifecycle.

---

## Delegation Model

Delegation is a kernel contract, not informal agent-to-agent chat.

The standard relationship is:

`Parent AgentInstance`
-> `DelegationRequest`
-> `Child AgentInstance`
-> `DelegationResult`

Key rules:

- the child does not inherit the parent's full private history
- the child receives a constrained execution packet, not the whole transcript
- the child executes under its own instance lifecycle
- child output returns to the parent, not automatically to the outside world
- child completion is not automatic parent acceptance

Conceptually, parent-side handling should evaluate:

1. contract validity
2. governance and safety
3. work-product fitness
4. integration and publishability

This separation is important because `Task` describes the work itself, while `DelegationRequest` describes how that work is assigned under a parent-child control relationship.

---

## Context and State Model

Torque treats context as a **state system**, not a giant prompt buffer.

The recommended layers are:

- global stable layer
- team/shared coordination layer
- agent-instance private layer
- external knowledge layer
- execution-time `TaskPacket` layer

### Authoritative vs Derived State

Authoritative state should remain in the objects that actually own it:

- `AgentDefinition`
- `AgentInstance`
- `Task`
- `DelegationRequest`
- `TeamDefinition`
- `TeamInstance`
- `TeamTask`
- `SharedTaskState`
- `Artifact`
- `ExternalContextRef`
- `ApprovalRequest`
- `MemoryWriteCandidate`

Torque should not create one persistent `TaskState` god object that tries to combine:

- work definition
- delegation control
- shared coordination state
- artifact index
- memory cache
- external knowledge snapshot

Derived views are still allowed, but they remain derived.

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
- a replacement for persistent state ownership

Default behavior should prefer:

- refs over copied bodies
- structured summaries over long prose
- lazy retrieval over eager stuffing

---

## Context Planes

Torque keeps three context-related planes explicitly separate:

- `ExternalContextRef`
  external reference plane
- `Artifact`
  execution result plane
- `Memory`
  semantic retention plane

These planes are related, but they are not interchangeable.

Recommended transition model:

- `ExternalContextRef`
  may be retrieved into local execution context
- execution
  may create `Artifact`
- accepted output
  may be published into `SharedTaskState`
- `Artifact` or accepted content
  may become `MemoryWriteCandidate`
- `MemoryWriteCandidate`
  may be retained as `Memory`

Important non-rules:

- external context does not automatically become artifact
- artifact does not automatically become memory
- team publish does not automatically become memory write
- memory does not replace artifact retention
- artifact does not replace external reference access

`SharedTaskState` is not one of the three planes. It is a governance-filtered coordination surface.

This separation is important for:

- token control
- ownership boundaries
- replayability
- recovery correctness

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

Capability is not implementation. Those should remain distinct concepts.

---

## Policy Model

Torque treats policy as a first-class governance system.

Policy is not just configuration spread across structs. Instead:

- multiple policy sources contribute input
- evaluation is done per dimension
- same-dimension merge is conservative by default
- runtime receives a structured `PolicyDecision`

Important rule:

`policy source hierarchy is not the same as universal override hierarchy`

Different layers may speak about different dimensions, and no single lower layer should silently override everything else.

Policy should govern:

- approval
- visibility
- delegation
- resource usage
- memory retention
- tool access

If a design problem is fundamentally about runtime governance, it likely belongs in policy rather than as a new ad hoc boolean flag.

---

## Team Model

`Team` is a harness-layer capability, not a kernel primitive.

Key principles:

- supervisor-led by default
- governance-first
- explicit shared-state publication
- dynamic expansion through selectors
- team-local approval routing
- collaboration-level recovery

The default collaboration model remains:

`Supervisor -> Subagent`

`SharedTaskState` is intentionally narrow. It should hold accepted coordination state such as:

- accepted facts
- decision summaries
- blocker summaries
- progress summaries
- approval refs
- published artifact refs

It is not:

- a transcript store
- a blob store
- a private scratch space for every member
- a memory plane

The team layer should capture collaboration facts and decisions, while the kernel continues to capture per-agent execution facts.

---

## Recovery Model

Torque recovery follows one consistent rule:

- `Event` is the truth source
- `Checkpoint` is the acceleration layer
- `Recovery` is restore + replay + reconciliation

This means:

- checkpoints are not canonical truth
- restoring a snapshot is not enough by itself
- replay is part of correctness, not just debugging
- recovery must reconcile against current storage and runtime reality

This applies to:

- kernel instance recovery
- team recovery
- long-running execution continuity

---

## Architectural Invariants

The following invariants should not be casually broken.

### 1. Kernel Is Instance-Centric

- `AgentInstance` is the execution owner
- `ExecutionRequest` is the execution intent object
- `Task` is the current work item
- `ExecutionResult` is a progression result

Do not collapse these back into one "request = task = session" object.

### 2. Context Is a State System, Not a Transcript Dump

- do not default to sharing full chat history
- prefer structured state and refs
- keep `TaskPacket` narrow and derived
- use lazy loading by default

### 3. Keep the Three Context Planes Separate

- `ExternalContextRef` is not `Artifact`
- `Artifact` is not `Memory`
- team publish is not memory retention

### 4. Capability Is Not Implementation

Keep `CapabilityRef`, `CapabilityProfile`, `CapabilityRegistryBinding`, `CapabilityResolution`, and `AgentDefinition` conceptually distinct.

### 5. Policy Is Evaluated Governance

Do not rebuild policy as scattered booleans when the change is fundamentally about approval, visibility, delegation, resource, memory, or tool governance.

### 6. Team Is Supervisor-Led by Default

Do not assume a symmetric peer mesh or unrestricted recursive delegation as the default collaboration model.

### 7. Event Truth, Checkpoint Acceleration

Do not treat checkpoints as a second canonical truth model.

---

## Current MVP Path

The current early product-facing slice is the **Session Agent MVP**.

It is intentionally narrow:

- single agent
- persistent session
- multi-turn interaction
- streaming
- bounded context window
- minimal safe tool support

It demonstrates:

`Torque already supports a stateful agent experience`

It does not yet demonstrate the full target architecture, especially around:

- team orchestration
- capability registry UX
- full policy exposure
- full recovery experience

The current MVP implementation path primarily lives in:

- `crates/agent-runtime-service`
- `crates/llm`

---

## Implementation Guidance

When adding or changing architecture-aligned code:

- prefer placing core runtime concepts in kernel-oriented crates rather than product-facing wrappers
- keep team-specific collaboration logic out of generic kernel execution abstractions
- prefer explicit objects over prompt-only conventions when a concept needs persistence, auditability, or recovery
- prefer refs and structured summaries over large inline payloads
- keep runtime decision points observable and auditable

When changing contracts or ownership boundaries:

- update the relevant spec
- do not silently drift implementation away from the spec set
- state clearly which layer owns any new cross-cutting module

---

## Authoritative Documents

The architecture is split across a focused set of documents.

### High-Level Architecture

- [Torque Agent Runtime / Harness Design](./superpowers/specs/2026-04-08-torque-agent-runtime-harness-design.md)
- [Torque Kernel Execution Contract Design](./superpowers/specs/2026-04-08-torque-kernel-execution-contract-design.md)

### Capability and Governance

- [Torque Capability Registry Model Design](./superpowers/specs/2026-04-08-torque-capability-registry-model-design.md)
- [Torque Policy Model Design](./superpowers/specs/2026-04-08-torque-policy-model-design.md)

### Context and Data Planes

- [Torque Context State Model Design](./superpowers/specs/2026-04-08-torque-context-state-model-design.md)
- [Torque Context Planes Design](./superpowers/specs/2026-04-08-torque-context-planes-design.md)

### Collaboration and Recovery

- [Torque Agent Team Design](./superpowers/specs/2026-04-08-torque-agent-team-design.md)
- [Torque Recovery Core Design](./superpowers/specs/2026-04-08-torque-recovery-core-design.md)

### Concept Navigation

- [Concept Index](./learn.md)

---

## One-Line Summary

Torque can be summarized as:

an instance-centric agent runtime kernel, plus a harness layer that lowers higher-level orchestration into explicit execution, capability, policy, context, team, and recovery contracts instead of hard-coding a workflow or transcript-centric system into the kernel.

总体架构图
+----------------------------------------------------------------------------------+
  |                              Upper-Layer Products / Callers                      |
  |----------------------------------------------------------------------------------|
  | CLI / API / Scheduler / Supervisor / Future Harness / External Orchestrators     |
  +------------------------------------------+---------------------------------------+
                                             |
                                             | standard runtime objects
                                             v
  +----------------------------------------------------------------------------------+
  |                                   Harness Layer                                  |
  |----------------------------------------------------------------------------------|
  | TeamDefinition | TeamInstance | TeamTask | SharedTaskState | Publish | Recovery  |
  | Mode selection | Planning | Decomposition | Supervisor -> Subagent               |
  |                                                                                  |
  | Note: this layer lowers decisions into kernel requests; it does not replace      |
  | the kernel contract.                                                             |
  +------------------------------------------+---------------------------------------+
                                             |
                                             | ExecutionRequest / DelegationRequest
                                             v
  +----------------------------------------------------------------------------------+
  |                                   Kernel Core                                    |
  |----------------------------------------------------------------------------------|
  | AgentDefinition                                                                    |
  | AgentInstance                                                                      |
  | Task                                                                               |
  | ExecutionRequest  ->  ExecutionResult                                              |
  | DelegationRequest ->  DelegationResult                                             |
  | ApprovalRequest                                                                    |
  | Artifact                                                                           |
  | ExternalContextRef                                                                 |
  |                                                                                   |
  | Instance lifecycle: CREATED -> HYDRATING -> READY -> RUNNING -> WAITING_* -> ... |
  | Task lifecycle:     OPEN -> IN_PROGRESS -> BLOCKED -> DONE / FAILED / ABANDONED  |
  +-------------------+----------------------+----------------------+-----------------+
                      |                      |                      |
                      | uses                 | governed by          | derives from
                      v                      v                      v
  +--------------------------+   +--------------------------+   +-------------------+
  |   Capability Layer       |   |      Policy Layer        |   | Context & State   |
  |--------------------------|   |--------------------------|   |-------------------|
  | CapabilityRef            |   | approval                 |   | Global stable     |
  | CapabilityProfile        |   | delegation               |   | Team coordination |
  | RegistryBinding          |   | visibility               |   | Agent private     |
  | CapabilityResolution     |   | resource                 |   | External refs     |
  | AgentDefinition binding  |   | memory                   |   | Artifact plane    |
  |                          |   | tool                     |   | Memory plane      |
  |                          |   | -> PolicyDecision        |   | -> TaskPacket     |
  +--------------------------+   +--------------------------+   +-------------------+
                      \                 |                 /
                       \                |                /
                        \               |               /
                         \              |              /
                          \             |             /
                           v            v            v
  +----------------------------------------------------------------------------------+
  |                              Infrastructure / Adapters                           |
  |----------------------------------------------------------------------------------|
  | agent-runtime-service | llm | checkpointer | DB persistence | SSE/HTTP | tools   |
  | SQLx repos            | OpenAI-compatible client | Redis/Postgres | auth         |
  +----------------------------------------------------------------------------------+

  如果映射到你这个 repo 的“建议落位”，可以再看成这样：

  docs/specs
     ↓ define target contracts

  crates/torque-kernel         <- first scaffold now
     ↓ consumed by

  crates/agent-runtime-service <- current MVP adapter/service layer
  crates/llm                   <- model client adapter
  crates/checkpointer          <- recovery/checkpoint adapter
