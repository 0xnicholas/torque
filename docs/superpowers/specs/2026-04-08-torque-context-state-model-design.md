# Torque Context State Model Design

## Overview

This document defines the current design direction for **context and state management** in Torque.

Torque should treat context as a layered state system, not as one large prompt or one giant shared transcript.

The core design goal is:

- keep execution context narrow
- keep authoritative state separated by responsibility
- make delegation conservative by default
- avoid turning transcripts into the primary state model
- support recovery, auditability, and lazy context loading

**Date**: 2026-04-08  
**Status**: Draft  
**Scope**: Context layers, authoritative vs derived state, task packet, visibility, lazy loading, artifact/memory/external context boundaries

---

## 1. Design Goals

- Define a clear layered context model for Torque runtime and harness
- Keep private execution state separate from shared coordination state
- Prefer structured state and references over long natural-language history
- Support delegation through narrow task packets rather than transcript inheritance
- Preserve clean boundaries between `ExternalContextRef`, `Artifact`, `Memory`, and shared task state
- Make context handling compatible with recovery, replay, and policy-governed execution

## 2. Non-Goals

- Torque does not treat chat transcript as the primary source of state
- Torque does not create one global mutable context object for all agents
- Torque does not preload all available context into every execution
- Torque does not collapse artifacts, memory, and external knowledge into one storage plane
- Torque does not require a single persisted `TaskState` god object

---

## 3. Core Principles

### 3.1 Structure First, Transcript Last

Torque should prefer:

- structured state
- accepted fact records
- artifact references
- narrow summaries

over:

- long raw transcripts
- inherited full chat history
- ad hoc prompt stuffing

### 3.2 Layered Context, Not One Big Prompt

Context should be modeled in layers with distinct ownership and visibility rules.

No single layer should become the default container for everything.

### 3.3 Authoritative State Stays Split

Authoritative state should remain distributed across the objects that actually own it.

Torque should not create a single persistent `TaskState` object that tries to combine:

- work definition
- delegation control
- shared state
- external context
- artifacts
- memory

### 3.4 Derived Execution Context Is Narrow

Execution should consume a narrow derived input envelope, not the full state universe.

The default runtime posture should be:

- derive a minimal task packet
- execute
- load more only when necessary

### 3.5 Lazy Loading by Default

Long documents, history, codebases, logs, and large result sets should remain outside the prompt by default.

They should be loaded through controlled retrieval only when needed.

---

## 4. Context Layers

Torque should distinguish at least five context layers.

### 4.1 Global Stable Layer

This layer contains small, long-lived, relatively stable execution context, such as:

- system policy
- agent definition policy
- stable role or capability definition
- high-level global objective when one exists

This layer should stay short and stable.

It should not become a dumping ground for team-local history or large shared state.

### 4.2 Team Coordination Layer

This layer is represented primarily by `SharedTaskState`.

It contains only governance-filtered coordination state, such as:

- accepted facts
- accepted artifact refs
- decision summaries
- blocker summaries
- progress summaries
- approval refs

It is not:

- a full transcript store
- a raw artifact body store
- a member-private working memory store

### 4.3 Agent Instance Private Layer

Each live `AgentInstance` owns its own private execution context.

This includes:

- message and working context
- tool loop state
- private scratch state
- local intermediate summaries
- active task references
- pending approvals
- child delegation references

This layer is private by default and should not be implicitly shared with other agents.

### 4.4 External Knowledge Layer

This layer is represented by `ExternalContextRef`.

It covers externally owned knowledge and resources such as:

- repositories
- documents
- tickets
- logs
- file spaces
- knowledge bases
- conversation threads

Recommended default:

- `ExternalContextRef` is read-only by default
- external content stays outside the prompt until explicitly retrieved

### 4.5 Artifact Plane

`Artifact` stores execution outputs and retrievable result bodies.

Artifacts are:

- precise
- traceable
- retrievable
- suitable for downstream consumption

Artifacts are not automatically memory.

### 4.6 Memory Plane

`Memory` is a long-term semantic recall layer.

It should contain selectively retained durable information, not every execution result.

All long-term writes should first pass through `MemoryWriteCandidate`.

### 4.7 Execution Layer: Task Packet

At execution time, Torque should materialize a narrow derived input envelope for the current agent.

This document calls that envelope `TaskPacket`.

`TaskPacket` is not a new source of truth. It is a derived execution view assembled from authoritative layers.

---

## 5. Authoritative vs Derived State

### 5.1 Authoritative State Objects

Authoritative state should remain in the objects that own specific responsibilities, including:

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

No single aggregate object should replace these boundaries.

### 5.2 No Persistent TaskState God Object

Torque should not define a single persistent mutable object that attempts to serve as:

- execution context
- coordination state
- artifact index
- memory cache
- external knowledge snapshot

That object would quickly become ambiguous and difficult to recover correctly.

### 5.3 Derived Views Are Allowed

Derived views are still useful.

Examples include:

- a materialized task packet
- a shared-state slice
- a replay/debug snapshot

But these are derived execution aids, not primary truth.

---

## 6. TaskPacket

### 6.1 Purpose

`TaskPacket` is the narrow execution input envelope for a single execution step or delegation.

It exists to answer:

"What is the minimum context this agent needs right now to execute safely and effectively?"

### 6.2 Ownership

`TaskPacket` should be assembled by the assigning authority, not by the child agent itself.

Recommended ownership:

- top-level execution: runtime assembles the packet
- team delegation: supervisor or parent layer assembles the packet
- executing child instance: consumes the packet

This preserves visibility and governance boundaries.

### 6.3 Suggested Shape

Conceptually, a task packet should be close to:

```json
{
  "goal": "Complete the assigned work",
  "instructions": "Specific execution guidance",
  "expected_outputs": ["structured_result", "artifact"],
  "input_artifact_refs": ["artifact://input-1"],
  "visible_context_refs": ["repo://service-a", "kb://policy-17"],
  "shared_state_slice": {
    "current_step": "verification",
    "relevant_decisions": ["Use risk-tiered review"],
    "open_questions": ["Need escalation threshold"],
    "relevant_fact_refs": ["fact://kyc-threshold"]
  },
  "constraints": {
    "risk_level": "medium",
    "tool_limits": ["read_only"]
  }
}
```

Field names may evolve, but the semantic shape should remain:

- explicit goal
- explicit instructions
- explicit expected outputs
- explicit input refs
- explicit visible context refs
- explicit shared-state slice
- explicit constraints

### 6.4 What TaskPacket Should Contain

Recommended contents:

- current goal
- current instructions
- expected outputs
- input artifact refs
- visible context refs
- relevant shared-state slice
- execution constraints

### 6.5 What TaskPacket Should Not Contain

By default, `TaskPacket` should not include:

- full parent transcript
- parent private scratch state
- full team shared state
- raw policy trees
- full external documents
- the entire memory or repository universe

### 6.6 Persistence Rule

`TaskPacket` should be derived by default and should not act as a mutable source of truth.

If materialized snapshots are stored for:

- audit
- replay
- debugging
- checkpoint assistance

they should be treated as read-only derived artifacts, not as the authoritative state model.

---

## 7. Shared State Slicing

### 7.1 SharedTaskState Is Too Broad to Pass Whole

Even though `SharedTaskState` is already lightweight compared with raw transcript history, it may still be broader than what a particular child execution needs.

Child execution should therefore receive a `shared_state_slice`, not the whole shared state by default.

### 7.2 Two-Stage Slicing Model

Recommended approach:

1. apply stable default slicing rules
2. allow supervisor or parent layer to explicitly add or remove a small number of items

This provides both:

- consistency
- situational judgment

### 7.3 Default Slice Contents

By default, a shared-state slice should include only items directly relevant to the current execution, such as:

- current step or phase summary
- relevant decision summaries
- branch-relevant blockers or open questions
- accepted fact refs needed for execution
- relevant artifact refs
- approval or execution-boundary summary when needed

### 7.4 Default Exclusions

By default, a shared-state slice should exclude:

- the full decision log
- unrelated branch state
- historical closed issues that do not matter now
- the full fact set
- all prior summaries

### 7.5 Escalation for More Context

If a child instance needs more context than the current slice provides, the default path should be:

- request more through the parent or supervisor
- or use controlled retrieval through visible tools and references

It should not silently expand its own visibility boundary to all available state.

---

## 8. Delegation Visibility Model

### 8.1 Agent-to-Agent Transfer Uses State Packets, Not Transcript Inheritance

Delegation should move:

- task packets
- state slices
- artifact refs
- context refs

not raw full chat transcript by default.

### 8.2 Conservative Default Visibility

Default delegation should remain conservative:

- child does not inherit full parent history
- child does not inherit all private scratch state
- child sees only explicitly passed artifacts and visible context refs
- child output remains private until accepted or published

### 8.3 Parent As Visibility Authority

The parent or assigning authority remains responsible for deciding what the child can see.

This keeps governance local and prevents uncontrolled context growth.

---

## 9. ExternalContextRef, Artifact, and Memory Boundaries

### 9.1 ExternalContextRef

`ExternalContextRef` represents mounted external context without making Torque own that domain model.

Recommended default:

- read-only by default
- not eagerly loaded into prompt context
- passed by reference, then retrieved on demand

### 9.2 Artifact

`Artifact` represents execution output.

Artifacts should remain:

- first-class outputs
- complete and retrievable
- separate from shared-state summaries

### 9.3 Memory

`Memory` is semantic retention, not output archiving.

Recommended rule:

- artifact does not automatically become memory
- accepted or useful content may be nominated as `MemoryWriteCandidate`
- memory writes remain a separate decision path

### 9.4 Publish Is Not Memory Write

Publishing to team shared state does not automatically imply long-term memory retention.

These actions serve different purposes:

- `team_shared`
  current collaboration coordination
- `memory`
  durable cross-session semantic recall

---

## 10. Lazy Context Loading

### 10.1 Default Behavior

Torque should default to lazy context loading rather than eager preload.

The runtime should first provide:

- the narrow task packet
- the minimum shared-state slice
- the minimum required references

Then the executing instance may retrieve more only when necessary.

### 10.2 Retrieval Pattern

Recommended pattern:

1. start with the minimal packet
2. retrieve additional external content only when needed
3. summarize or structure that retrieved content locally
4. continue without promoting all retrieved raw content into global state

### 10.3 Benefits

This helps:

- reduce token cost
- preserve visibility boundaries
- improve specialization
- reduce prompt clutter
- keep recovery and replay cleaner

---

## 11. Recommended Summary Rule

The default context rule for Torque should be:

`authoritative state stays split; execution context is materialized as a narrow derived packet`

And the default transfer rule should be:

`agent-to-agent transfer uses task packets and state objects, not raw chat transcripts`

---

## 12. Open Questions

- Should Torque define a first-class named `TaskPacket` object in the runtime API, or keep it as an internal execution concept at first?
- How much of shared-state slicing should be policy-driven versus runtime-default heuristics?
- When materialized packet snapshots are stored, should they be attached to events, checkpoints, or artifacts?
- Which retrieval requests should require parent approval before expanding child-visible context?
