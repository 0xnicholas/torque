# Torque Agent Team Design

## Overview

This document defines the current design for **Agent Team** in Torque.

In Torque, `Team` is a first-class object in the **Harness** layer, not in the **Kernel** layer.

The purpose of this design is to make team-based collaboration:

- governable
- recoverable
- auditable
- cost-aware
- compatible with Torque's agent-centric runtime model

**Date**: 2026-04-08  
**Status**: Draft  
**Scope**: Team model, delegation, shared state, event model, recovery

---

## 1. Design Goals

- Define `Team` as a reusable harness-level collaboration container
- Keep Torque Kernel neutral and avoid pushing team semantics into core runtime types
- Default to **supervisor-driven collaboration**
- Support both lightweight and role-based team definitions
- Support controlled shared state without collapsing into one giant shared context pool
- Make team execution observable and recoverable at the collaboration layer

## 2. Non-Goals

- Team is not a kernel primitive
- Team is not a free-form peer-to-peer agent society by default
- Team is not a built-in workspace domain model
- Team is not a playbook/workflow DSL
- Team shared state is not a memory store and not a full transcript store

---

## 3. Position in the Architecture

The architectural split remains:

- **Kernel**: `AgentDefinition`, `AgentInstance`, `ExecutionRequest`, `Task`, `Artifact`, `Event`, `Checkpoint`, `ApprovalRequest`, `MemoryWriteCandidate`, `ExternalContextRef`
- **Harness**: `TeamDefinition`, `TeamInstance`, `TeamTask`, orchestration modes, built-in collaboration features

`Team` should lower into kernel runtime objects rather than redefine them.

In practical terms:

`TeamDefinition`
-> creates `TeamInstance`
-> receives `TeamTask`
-> supervisor triages and selects a collaboration shape
-> emits kernel-level delegation and execution requests

---

## 4. Core Principles

### 4.1 Supervisor First

Torque should default to:

`Supervisor -> Subagent`

This is the preferred default because it gives better:

- convergence
- context isolation
- observability
- governance
- cost structure

Peer handoff may exist, but only as an explicit advanced action.

### 4.2 Team Is a Collaboration Container

A team is not "many agents in one bag". It is a governed collaboration container with:

- a leader
- member selection rules
- delegation rules
- shared-state rules
- event history
- recovery state

### 4.3 Shared State Must Stay Small

Shared team state should hold only coordination-relevant facts:

- accepted results
- published artifact references
- decisions
- blockers
- approvals

It should not hold:

- all raw tool outputs
- all member-private drafts
- all message history
- vector memory contents

### 4.4 Team Events Must Be Team-Level

Team event history should capture coordination facts, not duplicate every low-level agent event.

The team layer records:

- what the team accepted
- what the supervisor decided
- how work was routed
- when governance paths changed

The kernel still records per-agent execution facts.

---

## 5. TeamDefinition

`TeamDefinition` is the static template for a team.

It answers:

"What is this team, who leads it, how does it govern collaboration, and what members can it use?"

### 5.1 Supported Authoring Modes

Torque should support two authoring styles:

#### Lightweight Team

A light governance-oriented definition:

- supervisor
- member selection rules
- delegation policy
- shared-state policy
- limits and governance

This is suitable for ad hoc or lightly structured teams.

#### Role-Based Team

A more explicit template:

- leader role
- core roles
- dynamic roles
- capability references
- default agent implementations
- role-level policy

This is suitable for stable, reusable team templates.

### 5.2 Canonical Internal Form

Even when authored in different styles, every team definition should normalize into the same internal structure.

Suggested minimum fields:

- `team_id`
- `name`
- `description`
- `definition_mode`
- `supervisor_spec`
- `member_specs[]`
- `available_modes[]`
- `leadership_policy`
- `delegation_policy`
- `shared_state_policy`
- `approval_policy`
- `resource_policy`
- `failure_policy`

### 5.2.1 Canonical Shape

Regardless of authoring style, the normalized internal shape should be close to:

```yaml
team_definition:
  team_id: string
  name: string
  description: string
  definition_mode: lightweight | role_based

  supervisor_spec:
    agent_definition_ref: string
    capability_refs: []
    policy_overrides: {}

  member_specs:
    - member_id: string
      role_ref: string | optional
      kind: core | dynamic
      default_agent_definition_ref: string | optional
      candidate_agent_definition_refs: []
      capability_refs: []
      can_be_lead: bool
      can_delegate: bool
      policy_overrides: {}

  available_modes: [route, broadcast, coordinate, tasks]

  leadership_policy: {}
  delegation_policy: {}
  shared_state_policy: {}
  approval_policy: {}
  resource_policy: {}
  failure_policy: {}
```

The exact field names may evolve, but the semantic shape should remain stable.

### 5.3 Team Leader

`Team leader` is not a separate object model from `Supervisor`.

It is represented by:

- `supervisor_spec` in `TeamDefinition`
- `supervisor_instance_id` in `TeamInstance`
- governance authority in the policy blocks

The leader is the team-level control authority.

### 5.4 Member Model

Team members should use a mixed model:

- **fixed core members** for stable team identity
- **dynamic specialists** for task-specific expansion

This allows:

- stable governance
- reusable teams
- runtime elasticity

The recommended runtime composition is:

- one `supervisor`
- zero or more `core members`
- zero or more `dynamic specialists`

Dynamic specialists should be activated through policy-governed selection, not through unconstrained free spawning.

### 5.5 Role, Capability, AgentDefinition, MemberInstance

These concepts should remain distinct:

- `Role`: collaboration responsibility slot
- `Capability`: ability contract
- `AgentDefinition`: execution implementation template
- `MemberInstance`: runtime member in a `TeamInstance`

Recommended relation:

`Role`
-> references `Capability`
-> resolves candidate `AgentDefinition`s
-> creates runtime `MemberInstance`

### 5.6 Authoring Guidance

Use `lightweight` team definitions when:

- the team is mostly an orchestration shell
- member identity is dynamic
- governance matters more than long-lived role taxonomy

Use `role_based` team definitions when:

- the team is intended to be reusable
- stable specialist slots matter
- policy needs to be attached to named roles
- the team should be inspectable by humans as a durable template

---

## 6. Team Governance Policies

The most important part of `TeamDefinition` is governance.

### 6.1 Leadership Policy

Defines supervisor authority.

Suggested concerns:

- whether all `TeamTask`s must enter through the supervisor
- whether the supervisor can terminate a task path
- whether the supervisor can override member conclusions
- whether the supervisor can activate dynamic specialists
- whether the supervisor can initiate handoff or escalation

Recommended default:

- all `TeamTask`s enter through the supervisor
- supervisor may accept or reject member results
- supervisor may activate dynamic specialists
- supervisor may terminate branches
- supervisor may escalate
- supervisor does not default to performing specialist work itself

### 6.2 Delegation Policy

Defines how work can flow inside the team.

Suggested concerns:

- default delegation visibility
- default return contract
- maximum delegation depth
- which roles may further delegate
- whether parallel delegation is allowed
- whether dynamic specialists may be injected

Recommended default:

- delegation depth is shallow by default
- only the supervisor may delegate freely
- lead specialists may delegate only when explicitly allowed
- ordinary specialists may not recursively delegate by default

### 6.3 Shared State Policy

Defines who can see and update team shared state.

Recommended default:

- members can see accepted facts
- members can see published artifact refs
- members can see blockers and progress summaries
- members cannot see other members' private scratch state
- members cannot see full unfiltered history

Recommended default write permissions:

- supervisor may publish facts and artifacts
- lead specialists may propose publications
- ordinary specialists may return results but not directly mutate shared state

### 6.4 Approval Policy

Defines when the team must stop for approval or escalation.

Suggested concerns:

- which actions require approval
- who can escalate
- whether the supervisor can approve locally
- which cases require external HITL

### 6.5 Resource Policy

Defines team-level limits.

Suggested concerns:

- max active members
- max parallel delegations
- token/budget ceiling
- branch timeout
- tool risk constraints

Recommended default:

- cap concurrent active members
- cap concurrent delegations
- prefer route/guided-delegate over structured orchestration unless justified
- require supervisor triage before fan-out

### 6.6 Failure Policy

Defines how team-level failures are handled.

Suggested concerns:

- member failure strategy: retry / reroute / escalate / abort branch / abort team
- leader failure handling
- partial failure tolerance
- branch failure aggregation

Recommended default:

- member failure does not imply team failure
- branch failure may trigger reroute or escalation
- supervisor failure is team-critical
- explicit abort should be rare and policy-governed

---

## 7. TeamInstance

`TeamInstance` is the live execution container for a team.

It answers:

"How is this team operating right now?"

Suggested minimum fields:

- `team_instance_id`
- `team_definition_id`
- `status`
- `supervisor_instance_id`
- `active_member_refs[]`
- `active_team_task_refs[]`
- `shared_task_state_ref`
- `external_context_refs[]`
- `active_delegation_refs[]`
- `checkpoint_ref | optional`
- `created_at`
- `updated_at`

### 7.1 TeamInstance Statuses

Suggested statuses:

- `CREATED`
- `TRIAGING`
- `ROUTING`
- `ORCHESTRATING`
- `WAITING_MEMBERS`
- `WAITING_APPROVAL`
- `BLOCKED`
- `SUSPENDED`
- `COMPLETED`
- `FAILED`
- `CANCELLED`

### 7.2 Meaning of Core Statuses

- `TRIAGING`: supervisor is deciding collaboration shape
- `ROUTING`: task is on a light single-owner path
- `ORCHESTRATING`: supervisor is coordinating multiple members or branches
- `WAITING_MEMBERS`: team is waiting for member results
- `WAITING_APPROVAL`: governance pause
- `BLOCKED`: team needs external resolution, not just member completion

### 7.3 Recommended Core Transitions

The most important transitions are:

- `CREATED -> TRIAGING`
- `TRIAGING -> ROUTING`
- `TRIAGING -> ORCHESTRATING`
- `ROUTING -> WAITING_MEMBERS`
- `ORCHESTRATING -> WAITING_MEMBERS`
- `WAITING_MEMBERS -> ORCHESTRATING`
- `WAITING_MEMBERS -> WAITING_APPROVAL`
- `WAITING_MEMBERS -> BLOCKED`
- `WAITING_MEMBERS -> COMPLETED`
- `WAITING_APPROVAL -> ORCHESTRATING`
- `BLOCKED -> ORCHESTRATING`
- `any active state -> FAILED`
- `any active state -> CANCELLED`

### 7.4 Important Rule

`TeamInstance` status must be team-controlled, not inferred mechanically from member states.

Example:

- one member failing does not imply team failure
- all members being idle does not imply team completion

---

## 8. TeamTask

`TeamTask` is the work unit that enters a team.

It answers:

"What does this team need to accomplish?"

Suggested minimum fields:

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

### 8.1 What TeamTask Is Not

It is not:

- a graph node
- a kernel task
- a member message
- a direct agent invocation

### 8.2 Default Handling Flow

The default handling path should be:

1. task enters team
2. supervisor performs lightweight triage
3. simple task -> single route
4. medium task -> lead specialist with constrained support delegation
5. complex task -> structured orchestration

### 8.3 TeamTask Status

Suggested task-level statuses:

- `OPEN`
- `TRIAGED`
- `ROUTED`
- `IN_PROGRESS`
- `WAITING_APPROVAL`
- `BLOCKED`
- `DONE`
- `FAILED`
- `CANCELLED`

These are separate from `TeamInstance` status. One team may carry multiple team tasks over its lifetime.

---

## 9. Triage and Team Modes

### 9.1 Triage First

The first team decision should not be full decomposition. It should be triage.

The supervisor should judge:

- whether multiple specialist capabilities are needed
- whether parallelism is useful
- whether validation/review is needed
- whether the task has meaningful dependency structure
- whether the cost/risk justifies full orchestration

### 9.1.1 Triage Output

Triage should produce a small explicit decision object, conceptually similar to:

```yaml
triage_result:
  complexity: simple | medium | complex
  processing_path: single_route | guided_delegate | structured_orchestration
  selected_mode: route | broadcast | coordinate | tasks | null
  lead_member_ref: string | null
  rationale: string
```

This is useful for observability, replay, and supervisor auditability.

### 9.2 Default Processing Paths

#### Single-Route

Use when one specialist is sufficient.

Expected shape:

- one primary member
- minimal shared-state churn
- supervisor mainly waits and accepts/rejects the result

#### Guided-Delegate

Use when one lead specialist should drive the work, with limited support delegation.

Expected shape:

- one lead specialist
- optional small support fan-out
- supervisor remains global owner

#### Structured-Orchestration

Use when multiple roles, stages, or review gates are clearly needed.

Expected shape:

- explicit orchestration mode
- clearer branch structure
- stronger use of shared task state
- heavier governance and observability

### 9.3 Team Modes

Modes should remain harness-level orchestration strategies:

- `route`
- `broadcast`
- `coordinate`
- `tasks`

They should not be treated as free-form magic strings.

Each mode should eventually define:

- expected input shape
- delegation pattern
- shared-state usage pattern
- expected return shape
- failure-handling expectations

#### `route`

- single-owner execution
- lowest coordination overhead
- best default for simple tasks

#### `broadcast`

- parallel exploration or comparison
- suitable for research, option generation, cross-checking

#### `coordinate`

- supervisor-led multi-round coordination
- suitable when intermediate arbitration is required

#### `tasks`

- decomposition-oriented execution
- suitable when the problem naturally separates into multiple task units

---

## 10. SharedTaskState

`SharedTaskState` is the team's shared coordination surface.

It answers:

"What does the team currently agree on?"

Suggested minimum fields:

- `shared_task_state_id`
- `accepted_artifact_refs[]`
- `published_fact_entries[]`
- `delegation_status[]`
- `task_status_summary`
- `decision_log`
- `open_blockers[]`
- `approval_refs[]`
- `updated_at`

### 10.1 Purpose

It exists for:

- coordination
- progress visibility
- accepted outputs
- decision visibility
- recovery support

### 10.2 Explicit Non-Purposes

It is not:

- a full transcript store
- a memory database
- a raw artifact blob store
- a workspace model

### 10.3 Artifact Publish Relationship

Shared state should store references and accepted facts, not full artifact bodies.

Recommended flow:

`member private artifact`
-> `result returned`
-> `supervisor accepts`
-> `artifact publish/promote`
-> `SharedTaskState` stores reference + fact/summary

### 10.4 Shared Visibility Levels

For clarity, shared visibility should distinguish at least three conceptual levels:

- `private`
  only the producing member or supervisor
- `team_shared`
  visible according to shared-state policy
- `external_published`
  visible beyond the team through upper-layer integration

This distinction prevents "published to team" from being confused with "published to the outside world".

---

## 11. Delegation Model

Delegation inside a team should be explicit and governed.

### 11.1 Core Delegation Pattern

`Supervisor AgentInstance`
-> `DelegationRequest`
-> `Child AgentInstance`
-> `DelegationResult`

### 11.2 Default Visibility Rules

Default delegation should be conservative:

- child does not inherit full parent history
- child sees only explicitly passed artifacts and visible external contexts
- child output is private until accepted/published

### 11.2.1 Return Contract

Delegation inside the team should still honor the runtime-level return-contract idea.

Recommended defaults:

- `structured_result + artifacts` for most specialist work
- `summary_only` for cheap exploratory fan-out
- `decision` for review or arbitration branches

`full_trace` should remain exceptional.

### 11.3 Lead Specialist

For medium-complexity work, a lead specialist may temporarily own a task branch.

Important constraint:

- a lead specialist is not the team leader
- it is only the branch-level primary executor
- it may receive constrained delegation rights
- it still reports upward to the supervisor

### 11.4 Permission Tiers

The default authority tiers should be:

- **Supervisor**
  may triage, select mode, delegate broadly, accept/reject results, publish shared state, escalate, and terminate branches

- **Lead Specialist**
  may execute a primary branch, optionally perform constrained delegation, and propose publications

- **Specialist**
  may execute assigned work and return results, but does not mutate team shared state directly by default

---

## 12. Team Event Model

`TeamEvent` records collaboration-layer history.

Suggested minimum fields:

- `team_event_id`
- `team_instance_id`
- `event_type`
- `timestamp`
- `actor_ref`
- `team_task_ref | optional`
- `related_instance_refs[]`
- `related_artifact_refs[]`
- `payload`
- `causal_event_refs[]`

### 12.1 Event Categories

Recommended event categories:

- lifecycle
- task intake
- triage
- delegation
- member activation/result handling
- shared-state updates
- approval and escalation
- failure and recovery

Recommended representative event types include:

- `team_created`
- `team_task_received`
- `triage_completed`
- `mode_selected`
- `lead_assigned`
- `member_activated`
- `delegation_created`
- `member_result_received`
- `member_result_accepted`
- `member_result_rejected`
- `artifact_published`
- `fact_published`
- `approval_requested`
- `team_blocked`
- `team_unblocked`
- `team_completed`
- `team_failed`

### 12.2 Important Distinction

Team event history should record collaboration facts such as:

- mode selected
- lead assigned
- result accepted or rejected
- fact published
- blocker raised

It should not duplicate every low-level agent tool call.

### 12.3 Team Event vs SharedTaskState

- `TeamEvent`: history
- `SharedTaskState`: current consensus snapshot

Both are required. Neither replaces the other.

---

## 13. Team Recovery Model

Team recovery should be layered, not monolithic.

### 13.1 Principle

Team recovery should not depend on one giant snapshot of the whole team.

Instead:

- each `AgentInstance` keeps its own checkpoint/event history
- `TeamInstance` keeps its team-level coordination state
- recovery reconciles those layers

### 13.2 Team Checkpoint Contents

Suggested contents:

- `team_instance_id`
- `team_status`
- `active_team_tasks`
- `current_mode`
- `supervisor_instance_id`
- `active_member_refs`
- `active_delegation_refs`
- `shared_task_state_snapshot`
- `open_approvals`
- `open_blockers`
- `decision_summary`
- `event_anchor_id`

This checkpoint should remain coordination-focused. It should never become a dump of all member internals.

### 13.3 Recommended Recovery Strategy

Recommended default:

- restore team checkpoint
- restore shared task state snapshot
- restore supervisor first
- reconcile active delegations
- lazily rehydrate members as needed
- replay tail team events after checkpoint

This gives a strong balance of correctness and cost.

Recommended default recovery mode:

- `supervisor-first`

That means:

- recover the team shell and supervisor first
- then recover only the members needed to continue safely

### 13.4 Time Travel

Team time travel should branch lineage rather than rewrite history.

Recommended behavior:

- restore from historical team checkpoint
- create new `TeamInstance` lineage
- keep original lineage immutable
- keep new shared state isolated unless explicitly promoted

---

## 14. Minimal Object Graph

The minimal harness-level object graph is:

- `TeamDefinition`
- `TeamInstance`
- `TeamTask`
- `SharedTaskState`
- `TeamEvent`

And it depends on kernel-owned objects:

- `AgentDefinition`
- `AgentInstance`
- `Task`
- `Artifact`
- `Checkpoint`
- `ApprovalRequest`
- `ExternalContextRef`

In condensed form:

`TeamDefinition`
-> creates `TeamInstance`
-> receives `TeamTask`
-> supervisor triages and chooses mode
-> coordinates members through delegation
-> accepts/publishes outputs into `SharedTaskState`
-> records collaboration facts in `TeamEvent`
-> restores via team checkpoint + member reconciliation

---

## 15. Open Questions

- Should nested teams be explicit first-class harness objects or be lowered through supervisor delegation only?
- How much of shared task state should be directly queryable by regular members?
- Should team-level approval support both supervisor-local approval and mandatory external approval from day one?
- How should lightweight team definitions be authored ergonomically while still compiling to the canonical form?
