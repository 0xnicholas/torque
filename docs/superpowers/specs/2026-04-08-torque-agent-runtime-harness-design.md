# Torque Agent Runtime & Harness Design

## Overview

This document captures the current design direction for Torque as a general-purpose agent runtime and harness platform.

**Date**: 2026-04-08  
**Status**: Draft  
**Scope**: Runtime kernel, harness layer, team model, data planes, recovery model

---

## 1. Design Goals

- Make Torque a **general-purpose Agent Runtime / Harness Kernel**, not a product-specific workflow engine
- Keep the **runtime kernel neutral** so it can serve different upper-layer orchestration systems
- Treat **Agent** as the kernel's first-class execution abstraction
- Support **long-running, stateful, recoverable execution**
- Support **supervisor-driven multi-agent collaboration**
- Support **short-term state recovery**, **cross-session semantic memory**, and **artifact-based task continuity**
- Avoid hard-coding any specific upper-layer concepts such as a particular playbook DSL or workspace domain model

## 2. Non-Goals

- Torque Kernel does not own a product-specific `Playbook`, `Workflow`, or `Workspace` model
- Torque Kernel does not assume all orchestration lowers to a DAG
- Torque Kernel does not allow unrestricted peer-to-peer multi-agent communication by default
- Torque Kernel does not treat vector memory as the system's primary source of truth

---

## 3. Layered Architecture

Torque is split into two main layers:

### 3.1 Kernel Layer

The kernel is the execution substrate. It owns:

- `AgentDefinition`
- `AgentInstance`
- `ExecutionRequest`
- `Task`
- `Artifact`
- `Event`
- `Checkpoint`
- `ApprovalRequest`
- `MemoryWriteCandidate`
- `ExternalContextRef`

Kernel responsibilities:

- instance lifecycle
- turn execution
- tool mediation
- delegation runtime
- state persistence
- external context boundary handling

### 3.2 Harness Layer

The harness provides higher-level orchestration and batteries-included agent-building features.

Harness responsibilities:

- `TeamDefinition`
- `TeamInstance`
- `TeamTask`
- team governance and delegation policy
- orchestration modes
- planning, task decomposition, and collaboration patterns
- higher-level prompts, routines, and built-in capabilities

The harness lowers its orchestration decisions into kernel-level execution objects.

### 3.3 Standard Boundary

Upper-layer systems should interact with Torque through standard runtime objects, not by requiring Torque to understand their own DSL.

The boundary is:

`Upper-layer orchestration`
-> `ExecutionRequest / DelegationRequest / Artifact / ApprovalRequest`
-> `Torque Kernel`

This keeps Torque reusable across multiple upper-layer systems.

---

## 4. Core Kernel Model

### 4.1 AgentDefinition

Static definition of an agent's identity and policy.

Suggested fields:

- `id`
- `name`
- `system_prompt`
- `tool_policy`
- `memory_policy`
- `delegation_policy`
- `limits`
- `default_model_policy`

### 4.2 AgentInstance

A live execution instance created from an `AgentDefinition`.

It owns:

- message and working context
- tool loop state
- checkpoint lineage
- active task references
- private scratch state
- pending approvals
- child delegation references

`AgentInstance` is the kernel's execution center. `Task` is what the instance is working on, not the other way around.

### 4.3 Task

A runtime-level work item delegated to an `AgentInstance`.

It expresses:

- goal
- instructions
- input references
- constraints
- expected outputs

It is not:

- a playbook step
- a graph node
- a whole session

### 4.4 ExternalContextRef

Torque does not own an internal workspace domain model. Instead, it references external context through neutral references.

Suggested fields:

- `ref_id`
- `kind`
- `locator`
- `access_mode`
- `sync_policy`
- `metadata`

Examples:

- repo
- knowledge base
- ticket/project
- file space
- conversation thread
- upper-layer workspace/container

---

## 5. Execution Lifecycle

Execution is instance-centric.

Suggested lifecycle:

1. `Instantiate`
2. `Hydrate`
3. `Deliberate`
4. `Act`
5. `Checkpoint`
6. `Publish`
7. `Suspend / Resume / Complete / Fail`

### 5.1 AgentInstance States

- `CREATED`
- `HYDRATING`
- `READY`
- `RUNNING`
- `WAITING_TOOL`
- `WAITING_SUBAGENT`
- `WAITING_APPROVAL`
- `SUSPENDED`
- `COMPLETED`
- `FAILED`
- `CANCELLED`

### 5.2 Task States

- `OPEN`
- `IN_PROGRESS`
- `BLOCKED`
- `DONE`
- `FAILED`
- `ABANDONED`

These state machines must remain separate.

---

## 6. Standard Runtime Interfaces

### 6.1 ExecutionRequest

The standard entrypoint into Torque runtime.

Suggested fields:

- `request_id`
- `agent_definition_id`
- `instance_id | optional`
- `goal`
- `instructions`
- `input_artifacts[]`
- `external_context_refs[]`
- `constraints`
- `execution_mode`
- `expected_outputs`
- `caller_ref`
- `idempotency_key`

Notes:

- if `instance_id` is absent, Torque creates a new instance
- if `instance_id` is present, Torque continues an existing instance

### 6.2 ExecutionResult

Torque should return a progression result, not only a final answer.

Suggested fields:

- `instance_id`
- `status`
- `produced_artifacts[]`
- `published_artifacts[]`
- `memory_write_candidates[]`
- `delegation_requests[]`
- `approval_requests[]`
- `checkpoint_id`
- `usage`
- `events_tail[]`

### 6.3 First-Class Runtime Actions

The kernel should treat these as explicit, auditable actions:

- `model_response`
- `tool_call`
- `artifact_create`
- `artifact_publish`
- `memory_candidate_create`
- `delegate`
- `approval_request`
- `checkpoint_create`
- `suspend`
- `resume`
- `complete`
- `fail`

---

## 7. Memory, Artifact, and External Context Planes

Torque must separate three distinct planes.

### 7.1 Artifact Plane

Artifacts are execution outputs.

Examples:

- structured results
- drafts
- tool output snapshots
- files
- research notes
- task progress snapshots

Rules:

- artifacts default to `instance_private`
- artifacts become shared only through explicit `publish/promote`
- artifacts remain the precise, traceable output layer

### 7.2 Memory Plane

Memory is a semantic recall layer, not raw history.

Suggested categories:

- `agent_profile_memory`
- `user_preference_memory`
- `task_or_domain_memory`
- `external_context_memory`

Rules:

- memory is not equivalent to message history
- memory is not equivalent to artifacts
- all long-term writes first become `MemoryWriteCandidate`
- vector memory is a derived retrieval layer, not the source of truth

### 7.3 External Context Plane

External context is mounted or referenced, not owned.

Rules:

- Torque does not own the lifecycle of external contexts
- reading external context does not automatically create memory
- writing back to external systems should go through artifact publishing or explicit adapters

### 7.4 Cross-Session Continuity

State continuity is split across layers:

- short-term continuity: `Checkpoint + Event + Instance State`
- long-running task continuity: `Task Artifact + Published Progress Snapshot`
- cross-session semantic continuity: `Memory Plane`
- external collaboration continuity: `ExternalContextRef + Published Artifact`

---

## 8. Team as a Harness First-Class Object

`Team` is a first-class object in the Harness layer, but not in the Kernel layer.

This preserves a reusable kernel while still making team orchestration a core Torque product capability.

### 8.1 TeamDefinition

Static team template.

Suggested fields:

- `team_id`
- `name`
- `description`
- `supervisor_agent_definition_id`
- `member_roles[]`
- `governance_policy`
- `delegation_policy`
- `available_modes[]`
- `memory_policy`
- `artifact_policy`
- `limits`
- `default_execution_policy`

`member_roles[]` should be role slots, not hard-coded live instances.

### 8.2 TeamInstance

Live team execution container.

Suggested fields:

- `team_instance_id`
- `team_definition_id`
- `status`
- `supervisor_instance_id`
- `member_instance_refs[]`
- `active_team_tasks[]`
- `shared_task_state_ref`
- `external_context_refs[]`
- `checkpoint_ref | optional`
- `created_at`
- `updated_at`

### 8.3 TeamTask

A team-level work item.

Suggested fields:

- `team_task_id`
- `team_instance_id`
- `goal`
- `instructions`
- `input_artifacts[]`
- `external_context_refs[]`
- `constraints`
- `priority`
- `requested_mode | optional`
- `expected_outputs`
- `status`

`TeamTask` is handled by the supervisor and then lowered into kernel-level tasks and delegations.

### 8.4 Team Modes

Team modes belong to the harness layer:

- `coordinate`
- `route`
- `broadcast`
- `tasks`

These are orchestration strategies, not kernel primitives.

---

## 9. Delegation Contract

Torque should default to supervisor-driven delegation.

### 9.1 Core Delegation Model

`Supervisor AgentInstance`
-> creates `DelegationRequest`
-> runtime creates or continues `Child AgentInstance`
-> child works on constrained `Task`
-> child returns `DelegationResult`

Subagents are controlled child execution units, not free-form peer chats.

### 9.2 DelegationRequest

Suggested fields:

- `parent_instance_id`
- `child_agent_definition_selector`
- `task_goal`
- `instructions`
- `input_artifacts[]`
- `visible_context_refs[]`
- `constraints`
- `return_contract`
- `approval_policy`
- `idempotency_key`

### 9.3 Default Visibility Rules

Default delegation must be conservative:

- child cannot see the parent's full conversation history
- child cannot see all private parent scratch state
- child sees only explicitly passed artifacts and context refs
- child outputs remain private by default until accepted/published

### 9.4 Return Contract

Suggested return modes:

- `summary_only`
- `structured_result`
- `artifacts`
- `decision`
- `full_trace`

Default should favor `structured_result + artifacts`.

### 9.5 Controlled Handoff

Peer handoff exists only as an explicit advanced action:

- normal mode: parent retains control
- handoff mode: `transfer_control` event explicitly moves control to another instance

---

## 10. SharedTaskState and Artifact Publish

Shared team state must remain small and coordination-oriented.

### 10.1 SharedTaskState

`SharedTaskState` stores team-level shared facts, not raw content blobs.

Suggested contents:

- `accepted_artifact_refs[]`
- `published_fact_entries[]`
- `delegation_status[]`
- `task_status_summary`
- `decision_log`
- `open_blockers[]`
- `approval_refs[]`

It should not store:

- full tool outputs
- member-private drafts
- full message history
- raw large files
- vector memory entries

### 10.2 Publish Semantics

Artifact publishing is a governance action, not only a storage action.

Suggested fields:

- `artifact_id`
- `published_by`
- `source_scope`
- `target_scope`
- `publish_reason`
- `visibility`
- `summary`

Recommended flow:

`member private artifact`
-> `return to supervisor`
-> `supervisor accepts`
-> `artifact publish/promote`
-> `SharedTaskState` stores only reference + summary/facts

### 10.3 Facts vs Artifacts

- `Artifact`: full output, traceable, retrievable
- `FactEntry`: small accepted statement extracted for coordination

Shared task state should primarily contain facts and artifact refs, not full artifact bodies.

---

## 11. Event, Checkpoint, Replay, and Time Travel

Torque should use an event-sourced, snapshot-assisted recovery model.

### 11.1 Event Log

Events are the factual source of truth.

Required event classes include:

- `instance_created`
- `execution_requested`
- `turn_started`
- `model_response_received`
- `tool_call_started`
- `tool_call_completed`
- `artifact_created`
- `artifact_published`
- `memory_candidate_created`
- `delegation_requested`
- `delegation_completed`
- `approval_requested`
- `approval_resolved`
- `checkpoint_created`
- `instance_suspended`
- `instance_resumed`
- `instance_completed`
- `instance_failed`

### 11.2 Checkpoint

Checkpoint is an instance-level recovery snapshot.

Suggested contents:

- current instance status
- active task refs
- runtime context snapshot
- tool loop cursor
- pending approvals
- delegation state summary
- visible artifact refs
- compressed working context
- replay anchor event id

### 11.3 Recovery Model

Recommended model:

- `Event Log` is the truth source
- `Checkpoint` is the recovery acceleration layer
- recovery hydrates from latest checkpoint
- runtime replays only the event tail after that checkpoint

### 11.4 Side Effects

External side effects must be idempotent-aware.

Event/effect metadata should capture:

- tool call id
- idempotency key
- target system ref
- result summary
- side effect status

Recovery should avoid blindly re-executing already-committed effects.

### 11.5 Replay Modes

- `recovery replay`: restore execution safely
- `audit replay`: reconstruct history for debugging/audit without reproducing side effects

### 11.6 Time Travel

Time travel should branch lineage, not mutate history.

Recommended model:

- select historical checkpoint
- create new instance lineage/branch
- keep original lineage immutable
- isolate new branch outputs by default until explicit promotion

---

## 12. Architectural Constraints

These constraints are deliberate and should remain enforced:

- Kernel is agent-centric, not graph-centric
- `Team` is harness-level, not kernel-level
- default collaboration is `Supervisor -> Subagent`
- peer handoff is explicit, not implicit
- private state and shared state are separated
- artifacts, memory, and external context remain separate planes
- external workspace-like systems are referenced, not owned
- vector memory is derived, not authoritative

---

## 13. Open Questions

- Should the harness support nested `TeamInstance` objects directly, or lower them through supervisor delegation only?
- How much of `SharedTaskState` should be queryable by team members versus only by the supervisor?
- Which storage backend combinations should be first-class in the initial implementation for checkpoints, artifacts, and vector memory?
- How much built-in planning capability should the harness ship by default versus expose as optional capabilities?

---

## 14. Recommended Next Step

After this design is accepted, the next document should define an implementation plan that covers:

- crate boundaries
- core type definitions
- persistence model
- delegation runtime
- team harness objects
- recovery and checkpoint pipeline
