# Team Supervisor Design

**Date**: 2026-04-21
**Status**: Draft
**Scope**: Team Supervisor, Triage, Mode Handlers, Selector Resolution, SharedTaskState, Team Events

---

## 1. Overview

This document defines the design for **Team Supervisor** in Torque.

Team Supervisor is the orchestration core within the Harness layer that enables supervisor-led collaboration. It is responsible for:

- Task triage and mode selection
- Delegation lifecycle management
- Result acceptance and rejection
- SharedTaskState updates
- Team event emission

The Supervisor is implemented as a **Supervisor-as-tool-agent**: a `ReActHarness` agent with team-specific tools (delegate, publish, accept_result, etc.). This keeps the supervisor's domain logic in the LLM layer while maintaining clean separation from the orchestration infrastructure.

---

## 2. Design Goals

- Supervisor-driven collaboration as the default team pattern
- Support for all four team modes: route, broadcast, coordinate, tasks
- Explicit delegation and result handling (not implicit)
- Governance-filtered shared state (not a transcript dump)
- Observable team events for collaboration facts
- Selector-based dynamic member activation

---

## 3. Architecture

### 3.1 Component Overview

```
External Caller
      │
      ▼
POST /v1/team-instances/{id}/tasks
      │
      ▼
┌─────────────────────────────────────────────────────────┐
│              TeamSupervisor (new module)                │
│  - Polls for new team tasks                             │
│  - Performs triage (simple/medium/complex)              │
│  - Routes to appropriate mode handler                   │
│  - Manages delegation lifecycle                         │
│  - Handles result acceptance/rejection                  │
│  - Updates SharedTaskState                             │
│  - Emits team events                                   │
└─────────────────────────────────────────────────────────┘
      │
      ├──────────────────────────────────────────────────┐
      │                        │                        │
      ▼                        ▼                        ▼
┌──────────┐          ┌──────────────┐          ┌──────────┐
│RouteMode │          │BroadcastMode │          │TasksMode │
│          │          │              │          │          │
│ Single   │          │ Parallel     │          │ Decompose│
│ delegation│         │ delegation   │          │ aggregate│
└──────────┘          └──────────────┘          └──────────┘
                              │
                              ▼
                     ┌──────────────┐
                     │CoordinateMode │
                     │              │
                     │ Sequential   │
                     │ coordination │
                     └──────────────┘
```

### 3.2 Supervisor Agent

The supervisor is a `ReActHarness` agent with team-specific tools. This means:

- Supervisor decides through LLM reasoning + tool calls
- Triage is done by the supervisor's reasoning, not hardcoded logic
- Mode selection can be overridden by the supervisor
- Clean separation: orchestration layer handles routing, supervisor handles content decisions

### 3.3 Entry Point: API-First

Team tasks enter through the API:

```
POST /v1/team-instances/{id}/tasks
```

The supervisor polls for new tasks assigned to its team instance. This design was chosen because:

- Teams are long-running, async collaboration containers
- Multiple tasks flow in over the team's lifetime
- External systems create tasks and read results
- Simpler to reason about than run-triggered execution

---

## 4. Supervisor Tools

The supervisor agent has these tools available:

### 4.1 Delegation Tools

| Tool | Parameters | Purpose |
|------|------------|---------|
| `delegate_task` | `member_selector`, `goal`, `instructions`, `return_contract` | Create delegation to selected member(s) |
| `accept_result` | `delegation_id` | Accept member's delegation result |
| `reject_result` | `delegation_id`, `reason`, `reroute` | Reject result, optionally reroute |
| `get_delegation_status` | `delegation_id` | Get current status of a delegation |

### 4.2 Shared State Tools

| Tool | Parameters | Purpose |
|------|------------|---------|
| `publish_to_team` | `artifact_ref`, `summary`, `scope` | Publish artifact to SharedTaskState |
| `update_shared_fact` | `key`, `value` | Update a coordination fact |
| `get_shared_state` | - | Read current SharedTaskState |
| `add_blocker` | `description`, `source` | Add a blocker to shared state |
| `resolve_blocker` | `blocker_id` | Mark a blocker as resolved |

### 4.3 Task Tools

| Tool | Parameters | Purpose |
|------|------------|---------|
| `complete_team_task` | `summary`, `output_artifacts` | Mark team task complete |
| `fail_team_task` | `reason` | Mark team task failed |
| `request_approval` | `tool_name`, `reason` | Request team-level approval |

### 4.4 Info Tools

| Tool | Parameters | Purpose |
|------|------------|---------|
| `list_team_members` | - | List available team members |
| `get_task_details` | `task_id` | Get details of a team task |

---

## 5. Mode Handlers

### 5.1 RouteMode (Simple Task)

Used when one specialist is sufficient.

**Flow:**
1. Supervisor receives task
2. Supervisor selects a member via selector
3. Supervisor calls `delegate_task(member_selector, goal, instructions)`
4. Mode handler creates delegation, sets member to WAITING_SUBAGENT
5. Supervisor waits for delegation result
6. On result: `accept_result` or `reject_result`
7. Supervisor calls `publish_to_team` for accepted results
8. Supervisor calls `complete_team_task`

**Characteristics:**
- Single delegation
- Low coordination overhead
- Supervisor mainly waits and accepts/rejects

### 5.2 BroadcastMode (Parallel Exploration)

Used for research, option generation, cross-checking.

**Flow:**
1. Supervisor receives task
2. Supervisor calls `delegate_task` with multiple selectors or a group selector
3. Mode handler creates multiple delegations in parallel
4. Supervisor waits for all delegation results
5. Supervisor evaluates results
6. Supervisor accepts best result(s), rejects others
7. Supervisor aggregates and publishes

**Characteristics:**
- Multiple parallel delegations
- Fan-out/fan-in pattern
- Supervisor evaluates and aggregates

### 5.3 TasksMode (Structured Decomposition)

Used when the problem naturally separates into multiple task units.

**Flow:**
1. Supervisor receives task
2. Supervisor decomposes into subtasks
3. For each subtask: `delegate_task` to appropriate member
4. Track subtask completion
5. Aggregate subtask results
6. Supervisor publishes final aggregated result

**Characteristics:**
- Explicit decomposition
- Subtask dependency tracking
- Result aggregation

### 5.4 CoordinateMode (Multi-Round Coordination)

Used when intermediate arbitration is required.

**Flow:**
1. Supervisor receives task
2. Supervisor sets up initial SharedTaskState
3. Supervisor delegates first step to a member
4. Member writes intermediate results to SharedTaskState
5. Supervisor reads SharedTaskState, decides next action
6. Repeat until completion

**Characteristics:**
- SharedTaskState as coordination plane
- Multi-round iteration
- Supervisor arbitrates each round

---

## 6. Selector Resolution

### 6.1 MemberSelector

```rust
pub struct MemberSelector {
    pub selector_type: SelectorType,
    pub capability_profiles: Vec<String>,  // e.g., ["specialist.research"]
    pub role: Option<String>,              // e.g., "writer"
    pub agent_definition_id: Option<Uuid>, // Direct reference
}
```

### 6.2 SelectorResolver

```rust
pub struct SelectorResolver {
    capability_registry: Arc<dyn CapabilityRegistry>,
}

impl SelectorResolver {
    pub async fn resolve(
        &self,
        selector: &MemberSelector,
        team_instance_id: Uuid,
    ) -> anyhow::Result<Vec<CandidateMember>> {
        // 1. Load team instance and available members
        // 2. Filter members by capability profile match
        // 3. Check team policy (resource limits, approval requirements)
        // 4. Return ranked candidates with rationale
    }
}
```

### 6.3 Resolution Rules

- Selector binds to **capability profile**, not directly to `AgentDefinition`
- Resolved candidates include compatibility info and policy check summary
- Supervisor makes final selection from candidates
- Runtime creates actual delegation only after supervisor choice

---

## 7. SharedTaskState

### 7.1 Purpose

`SharedTaskState` is the team's shared coordination surface. It answers: "What does the team currently agree on?"

### 7.2 Contents

```rust
pub struct SharedTaskState {
    pub id: Uuid,
    pub team_instance_id: Uuid,
    pub accepted_artifact_refs: Vec<ArtifactRef>,
    pub published_facts: Vec<PublishedFact>,
    pub delegation_status: Vec<DelegationStatus>,
    pub open_blockers: Vec<Blocker>,
    pub decisions: Vec<Decision>,
    pub updated_at: DateTime<Utc>,
}
```

### 7.3 Governance Rules

**What belongs in SharedTaskState:**
- Accepted facts
- Accepted artifact refs
- Decision summaries
- Blocker summaries
- Progress summaries

**What does NOT belong:**
- Full member output bodies
- Raw tool outputs
- Member-private drafts
- Full conversation history

### 7.4 Publish Scopes

| Scope | Visibility |
|-------|------------|
| `private` | Only producing member and supervisor |
| `team_shared` | All team members, governed by shared_state_policy |
| `external_published` | Beyond team through upper-layer integration |

**Rule**: `team_shared` and `external_published` are separate governance actions.

---

## 8. Team Events

### 8.1 Purpose

Team events record collaboration facts, not every low-level agent event.

### 8.2 Event Types

| Category | Event Types |
|----------|-------------|
| Lifecycle | `team_task_received`, `team_completed`, `team_failed` |
| Triage | `triage_completed`, `mode_selected` |
| Delegation | `delegation_created`, `delegation_accepted`, `delegation_rejected` |
| Member | `member_activated`, `member_result_received`, `member_result_accepted`, `member_result_rejected` |
| State | `artifact_published`, `fact_published`, `blocker_added`, `blocker_resolved` |
| Governance | `approval_requested`, `team_blocked`, `team_unblocked` |

### 8.3 Event Schema

```rust
pub struct TeamEvent {
    pub team_event_id: Uuid,
    pub team_instance_id: Uuid,
    pub event_type: String,
    pub timestamp: DateTime<Utc>,
    pub actor_ref: String,           // "supervisor" or member role/id
    pub team_task_ref: Option<Uuid>,
    pub related_instance_refs: Vec<Uuid>,
    pub related_artifact_refs: Vec<Uuid>,
    pub payload: serde_json::Value,
    pub causal_event_refs: Vec<Uuid>,
}
```

---

## 9. Database Schema

### 9.1 New Tables

```sql
-- v1_team_tasks
CREATE TABLE v1_team_tasks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    team_instance_id UUID NOT NULL REFERENCES v1_team_instances(id),
    goal TEXT NOT NULL,
    instructions TEXT,
    status TEXT NOT NULL DEFAULT 'OPEN',
    triage_result JSONB,
    mode_selected TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);

-- v1_team_shared_state
CREATE TABLE v1_team_shared_state (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    team_instance_id UUID NOT NULL REFERENCES v1_team_instances(id) UNIQUE,
    accepted_artifact_refs JSONB NOT NULL DEFAULT '[]',
    published_facts JSONB NOT NULL DEFAULT '[]',
    delegation_status JSONB NOT NULL DEFAULT '[]',
    open_blockers JSONB NOT NULL DEFAULT '[]',
    decisions JSONB NOT NULL DEFAULT '[]',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- v1_team_events
CREATE TABLE v1_team_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    team_instance_id UUID NOT NULL REFERENCES v1_team_instances(id),
    event_type TEXT NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    actor_ref TEXT NOT NULL,
    team_task_ref UUID,
    related_instance_refs JSONB NOT NULL DEFAULT '[]',
    related_artifact_refs JSONB NOT NULL DEFAULT '[]',
    payload JSONB NOT NULL DEFAULT '{}',
    causal_event_refs JSONB NOT NULL DEFAULT '[]'
);
```

---

## 10. Team Task Status

```
OPEN
  │
  ▼
TRIAGED ──► (triage_result set, mode selected)
  │
  ▼
IN_PROGRESS
  │
  ├─────────────────────────────────────┐
  ▼                    ▼                ▼
WAITING_MEMBERS    BLOCKED         COMPLETED
  │                    │                ▲
  ▼                    ▼                │
RESULTS_RECEIVED ◄─────┴────── (accept/reject loop)
  │
  ▼
ACCEPTED / REJECTED ──► (can reroute back to IN_PROGRESS)
```

---

## 11. Implementation Structure

```
src/
├── service/
│   └── team/
│       ├── mod.rs              # TeamService (existing CRUD + new methods)
│       ├── supervisor.rs       # TeamSupervisor orchestration loop
│       ├── modes/
│       │   ├── mod.rs
│       │   ├── route.rs        # RouteMode handler
│       │   ├── broadcast.rs    # BroadcastMode handler
│       │   ├── coordinate.rs   # CoordinateMode handler
│       │   └── tasks.rs        # TasksMode handler
│       ├── selector.rs         # SelectorResolver
│       ├── shared_state.rs     # SharedTaskState management
│       └── events.rs           # TeamEvent emission
├── tools/
│   └── team_tools.rs           # Supervisor agent tools (delegate, publish, etc.)
├── models/v1/
│   └── team.rs                 # Add: TeamTask, SharedTaskState, TeamEvent, etc.
└── api/v1/
    └── teams.rs                # Add: POST /v1/team-instances/{id}/tasks
```

---

## 12. Invariants

1. **Supervisor is the control authority**: Non-supervisor modules may emit signals but do not make final team decisions
2. **Delegation is explicit**: Child completion does not imply parent acceptance
3. **Shared state is governance-filtered**: Not a transcript dump
4. **Team events are collaboration facts**: Not every low-level agent event
5. **Selector resolves to capability**: Not directly to AgentDefinition

---

## 13. Open Questions

- Should nested teams be explicit first-class harness objects or lowered through supervisor delegation only?
- How should team-level approval interact with kernel-level ApprovalRequest?
- Should supervisor tools be configurable per team definition?
