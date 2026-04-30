# AGENTS.md — Torque Architecture Guide

## Purpose

This file is the repo-level guide for humans and coding agents working in `torque`.

It answers three questions:

1. What Torque is trying to become
2. Which documents are authoritative for each part of the architecture
3. Which invariants should not be casually broken while implementing

This file is intentionally high level. Detailed design belongs in the linked specs.

---

## Project Positioning

Torque is evolving toward:

- an **Agent Runtime Kernel**
- a higher-level **Harness** built on top of that kernel
- explicit models for **Team orchestration**, **Capability resolution**, **Policy evaluation**, **Context state management**, and **Recovery**

Torque is not currently modeled as:

- a DAG-first planner/executor system
- a workflow engine whose kernel abstraction is graph nodes
- a system where context is primarily accumulated chat history

The current architectural center is `AgentInstance`, not DAG.

---

## Current Crate Structure

The repository contains four production crates:

- `crates/torque-kernel`
  Core execution contracts: `AgentInstance`, `Task`, `ExecutionRequest`, `Event`, `Checkpoint`, `DelegationRequest`, `TaskPacket`, `ExternalContextRef`

- `crates/torque-runtime`
  Runtime implementation: `Environment`, `Host`, `VFS`, `Checkpoint` persistence, `Event` storage, `Message` handling, `Tool` infrastructure

- `crates/torque-harness`
  Harness layer: API handlers, Service orchestration, Repository persistence, Team supervisor, Policy evaluation, Capability registry

- `crates/llm`
  OpenAI-compatible LLM client with streaming and tool-call support

---

## Core Architecture

Torque should be understood as a layered system:

| Layer | Crate | Key Concepts |
|-------|-------|-------------|
| 1. Kernel Execution | `torque-kernel` | `AgentInstance`, `Task`, `ExecutionRequest`, `DelegationRequest`, checkpoint, recovery |
| 2. Capability Layer | `torque-harness` | `CapabilityRef`, `CapabilityProfile`, `CapabilityRegistry`, resolution |
| 3. Policy Layer | `torque-harness` | `PolicyDecision`, dimensional evaluation, tool/approval/resource governance |
| 4. Context and State | `torque-kernel` + `torque-harness` | `TaskPacket`, `ExternalContextRef`, lazy loading, compaction |
| 5. Harness / Team | `torque-harness` | `TeamInstance`, supervisor, `SharedTaskState`, selector expansion, publish |
| 6. Recovery | `torque-kernel` + `torque-runtime` | `Event` truth source, `Checkpoint` acceleration, replay |

These layers should stay separated in both code and design.

---

## Authoritative Specs

Read these before making architectural changes:

- [Torque Agent Runtime / Harness Design](./docs/superpowers/specs/2026-04-08-torque-agent-runtime-harness-design.md)
- [Torque Kernel Execution Contract Design](./docs/superpowers/specs/2026-04-08-torque-kernel-execution-contract-design.md)
- [Torque Capability Registry Model Design](./docs/superpowers/specs/2026-04-08-torque-capability-registry-model-design.md)
- [Torque Policy Model Design](./docs/superpowers/specs/2026-04-08-torque-policy-model-design.md)
- [Torque Context State Model Design](./docs/superpowers/specs/2026-04-08-torque-context-state-model-design.md)
- [Torque Context Planes Design](./docs/superpowers/specs/2026-04-08-torque-context-planes-design.md)
- [Torque Agent Team Design](./docs/superpowers/specs/2026-04-08-torque-agent-team-design.md)
- [Torque Recovery Core Design](./docs/superpowers/specs/2026-04-08-torque-recovery-core-design.md)
- [Concept Index](./docs/learn.md)

If a change touches multiple layers, update the relevant spec first or at least keep the implementation aligned with the existing contracts.

---

## Key Invariants

### 1. Kernel is instance-centric

- `AgentInstance` is the execution owner
- `ExecutionRequest` is a kernel intent object
- `Task` is the current work item
- `ExecutionResult` is a progression result, not just a final answer blob

Do not collapse these back into a single “request = task = session” object.

### 2. Context is a state system, not a transcript dump

- do not default to sharing full chat history
- prefer structured state, refs, and derived execution packets
- keep `TaskPacket` narrow and derived
- use lazy context loading by default
- keep periodic state convergence / compaction in mind for long-running flows

### 3. Keep the three context planes separate

- `ExternalContextRef` = external reference plane
- `Artifact` = execution result plane
- `Memory` = semantic retention plane

These do not automatically collapse into each other.

In particular:

- external context does not automatically become artifact
- artifact does not automatically become memory
- team publish does not automatically become memory write

### 4. Capability is not implementation

Keep these concepts distinct:

- `CapabilityRef`
- `CapabilityProfile`
- `CapabilityRegistryBinding`
- `CapabilityResolution`
- `AgentDefinition`

Upper layers should usually reference capability, not directly bind themselves to a concrete agent implementation.

### 5. Policy is evaluated governance, not scattered config

Policy should be treated as:

`policy inputs -> dimensional evaluation -> conservative merge -> PolicyDecision`

Do not reintroduce ad hoc boolean flags everywhere when the change is really a policy concern.

### 6. Team is supervisor-led by default

The default collaboration model is:

`Supervisor -> Subagent`

Not:

- fully symmetric peers
- shared full-context agent mesh
- unconstrained recursive delegation

### 7. Event truth, checkpoint acceleration

Recovery depends on:

- `Event` as truth source
- `Checkpoint` as acceleration layer
- `Recovery` as restore + replay + reconciliation

Do not treat checkpoints as a second canonical truth model.

---

## Execution and Delegation Rules

### Task vs Delegation

Keep these separate:

- `Task`
  describes the work itself
- `DelegationRequest`
  describes how that work is assigned under a parent-child control relationship

`Task` is content contract.
`DelegationRequest` is control contract.

### Result handling

`DelegationResult` is not automatic acceptance.

Parent-side handling should conceptually follow:

1. contract validity
2. governance and safety
3. work-product fitness
4. integration and publishability

Do not shortcut child completion into automatic publish or automatic parent success.

---

## Implementation Guidance

When adding new code:

- prefer placing core runtime concepts in kernel-oriented crates rather than mixing them into product-facing APIs
- keep team-specific collaboration logic out of generic kernel execution abstractions
- prefer explicit objects over prompt-only conventions when a concept needs persistence, auditability, or recovery
- prefer refs and structured summaries over large inline payloads
- keep runtime decision points auditable

When changing architecture:

- update the relevant spec if the change affects contracts or boundaries
- do not silently drift the implementation model away from the spec set
- if a new module crosses multiple layers, state clearly which layer owns it

---

## Working Notes For Agents

Before substantial implementation work in this repo:

- read [docs/learn.md](./docs/learn.md)
- open the specific spec for the layer you are touching
- verify whether the code is prototype-era or already aligned with the newer model

When unsure where something belongs, use this default mapping:

| Concern | Layer | Spec |
|---------|-------|------|
| execution semantics | Kernel Execution | kernel execution contract |
| capability lookup / resolution | Capability Layer | capability registry model |
| governance / limits / approvals | Policy Layer | policy model |
| context shaping / retrieval / compaction | Context and State | context state model |
| artifacts / memory / external references | Context Planes | context planes |
| collaboration / shared state / publish / selector | Harness / Team | team design |
| replay / checkpoint / restore / reconciliation | Recovery | recovery core |

---

## Out of Scope For This File

This file does not attempt to define:

- concrete database schemas for the new architecture
- exact API payloads for every service
- prompt templates
- retrieval algorithms
- storage engine implementation details

Those belong in the relevant specs or implementation plans.
