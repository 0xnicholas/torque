# Torque Platform API Design

## Overview

This document defines the external REST API specification for Torque as a full agent runtime and harness platform.

**Date**: 2026-04-16  
**Status**: Draft  
**Scope**: Product-facing HTTP REST API, OpenAPI 3.1 schema, SSE streaming contract, migration path from MVP session API

### Purpose

The existing MVP API (`/sessions`, `/chat`) was intentionally narrow: it demonstrated a single persistent agent session but did not expose the broader kernel/harness architecture. This design replaces the MVP interface with a unified, versioned, resource-oriented REST API that:

- exposes `AgentInstance` as a first-class execution container
- supports `TeamInstance` and `TeamTask` for supervisor-driven collaboration
- treats `Task`, `Artifact`, `Delegation`, `Approval`, `Checkpoint`, and `Event` as independent queryable resources
- preserves SSE streaming for real-time execution feedback
- uses `202 Accepted` for long-running team tasks
- provides a clean migration path from the MVP session-agent interface

### Design Decisions

- **Resource-oriented RESTful model**: Agent, Team, Task, Artifact, etc. are first-class resources with CRUD semantics
- **SSE for runs, 202 for team tasks**: `/agent-instances/{id}/runs` streams over SSE; `/team-instances/{id}/tasks` returns 202 with async task ID
- **Productized abstraction layer**: the API is a Harness-level product interface, not a 1:1 kernel object dump
- **OpenAPI 3.1 as source of truth**: all schemas, enums, and payloads are machine-readable
- **Complete replacement of MVP routes**: new API lives under `/v1/`; existing MVP routes are deprecated

---

## Resource Model and URL Topology

### Primary Resources

| Resource | Path | Description |
|----------|------|-------------|
| **Agent Definition** | `/v1/agent-definitions` | Static execution template (system prompt, policies, limits) |
| **Agent Instance** | `/v1/agent-instances` | Live execution owner with context and state |
| **Team Definition** | `/v1/team-definitions` | Static team template (supervisor, members, governance) |
| **Team Instance** | `/v1/team-instances` | Live team execution container |
| **Task** | `/v1/tasks` | Unified work-item view (agent task or team task) |
| **Artifact** | `/v1/artifacts` | Execution outputs (files, structured results, snapshots) |
| **Memory Write Candidate** | `/v1/memory-write-candidates` | Proposed memory before governance approval |
| **Memory Entry** | `/v1/memory-entries` | Durable semantic memory |
| **Capability Profile** | `/v1/capability-profiles` | Reusable ability package definitions |
| **Capability Registry Binding** | `/v1/capability-registry-bindings` | Links capability profiles to agent definitions |
| **Delegation** | `/v1/delegations` | Parent-child execution contracts |
| **Approval** | `/v1/approvals` | Governance pause requests |
| **Checkpoint** | `/v1/checkpoints` | Recovery acceleration snapshots |
| **Event** | `/v1/events` | Read-only event stream (truth source) |

### Sub-resources and Action Routes

```
/agent-instances/{id}/runs              → trigger a run (SSE stream)
/agent-instances/{id}/cancel            → cancel current run/task
/agent-instances/{id}/resume            → resume from SUSPENDED
/agent-instances/{id}/delegations       → delegations initiated by this instance
/agent-instances/{id}/artifacts         → instance-private artifacts
/agent-instances/{id}/checkpoints       → instance checkpoints
/agent-instances/{id}/events            → instance event history

/team-instances/{id}/tasks              → team tasks
/team-instances/{id}/members            → active member refs
/team-instances/{id}/shared-state       → SharedTaskState read-only view
/team-instances/{id}/artifacts          → team-published artifacts
/team-instances/{id}/events             → team collaboration events

/tasks/{id}/events                      → task-related events
/tasks/{id}/approvals                   → approvals raised by this task
/tasks/{id}/delegations                 → child delegations under this task

/artifacts/{id}/content                 → raw artifact content
/artifacts/{id}/publish                 → promote scope (private → team_shared → external)

/checkpoints/{id}/restore               → restore from checkpoint

/capability-profiles/{id}/bindings      → registry bindings for this profile
/capability-profiles/{id}/resolve       → selector resolution endpoint
```

---

## Core Endpoints

### Agent Definitions

```http
POST   /v1/agent-definitions
GET    /v1/agent-definitions
GET    /v1/agent-definitions/{id}
DELETE /v1/agent-definitions/{id}
```

**Create request** (`AgentDefinitionCreate`):
```json
{
  "name": "code-reviewer",
  "description": "Security-focused code review agent",
  "system_prompt": "You are a careful code reviewer...",
  "tool_policy": {
    "allowed_tools": ["read_file", "diff"],
    "default_deny": true
  },
  "memory_policy": {
    "recall_enabled": true,
    "write_enabled": false
  },
  "delegation_policy": {
    "max_depth": 1,
    "allow_child_delegation": false
  },
  "limits": {
    "max_turns": 20,
    "max_tokens_per_turn": 8000
  },
  "default_model_policy": {
    "provider": "openai",
    "model": "gpt-4o"
  }
}
```

### Agent Instances

```http
POST   /v1/agent-instances
GET    /v1/agent-instances
GET    /v1/agent-instances/{id}
POST   /v1/agent-instances/{id}/cancel
POST   /v1/agent-instances/{id}/resume
DELETE /v1/agent-instances/{id}
```

**Cancel response** (`200 OK`):
```json
{
  "instance_id": "inst_123",
  "previous_status": "RUNNING",
  "current_status": "CANCELLED"
}
```

**Resume response** (`200 OK`):
```json
{
  "instance_id": "inst_123",
  "previous_status": "SUSPENDED",
  "current_status": "READY"
}
```

**Create request** (`AgentInstanceCreate`):
```json
{
  "agent_definition_id": "def_xxx",
  "external_context_refs": [
    { "kind": "repo", "locator": "github.com/acme/app" }
  ]
}
```

**Instance statuses**: `CREATED`, `HYDRATING`, `READY`, `RUNNING`, `WAITING_TOOL`, `WAITING_SUBAGENT`, `WAITING_APPROVAL`, `SUSPENDED`, `COMPLETED`, `FAILED`, `CANCELLED`

### Agent Runs (SSE Streaming)

```http
POST /v1/agent-instances/{id}/runs
```

**Request** (`RunRequest`):
```json
{
  "goal": "Review this PR for security issues",
  "instructions": "Focus on SQL injection and auth bypass",
  "input_artifacts": ["art_123"],
  "expected_outputs": ["structured_review", "severity_list"],
  "execution_mode": "interactive",
  "idempotency_key": "run-2026-04-16-001"
}
```

**Response**: `Content-Type: text/event-stream`

#### SSE Event Types

| Event | Payload Fields | Description |
|-------|---------------|-------------|
| `run.started` | `run_id`, `task_id`, `instance_id`, `status` | Run has begun |
| `run.chunk` | `content` | Assistant content fragment |
| `run.tool_call` | `tool_call_id`, `name`, `arguments` | Tool invocation |
| `run.tool_result` | `tool_call_id`, `success`, `content`, `error` | Tool completion |
| `run.delegation_created` | `delegation_id`, `child_instance_id` | Child delegation spawned |
| `run.approval_requested` | `approval_id`, `reason` | Governance pause |
| `run.artifact_produced` | `artifact_id`, `kind` | New artifact created |
| `run.checkpoint_created` | `checkpoint_id` | Recovery snapshot saved |
| `run.completed` | `run_id`, `task_id`, `status`, `produced_artifacts[]`, `summary` | Terminal success |
| `run.error` | `code`, `message`, `recoverable` | Terminal failure |

**Terminal event contract**: every `/runs` stream ends with exactly one `run.completed` or `run.error`. It is always the last event before stream close. Clients must treat stream close without a terminal event as a transport failure.

### Tasks

```http
GET    /v1/tasks
GET    /v1/tasks/{id}
POST   /v1/tasks/{id}/cancel
GET    /v1/tasks/{id}/events
GET    /v1/tasks/{id}/approvals
GET    /v1/tasks/{id}/delegations
```

**Task statuses**: The `/v1/tasks` resource is polymorphic. The `status` field value depends on `task_type`:

- **Agent task statuses**: `OPEN`, `IN_PROGRESS`, `BLOCKED`, `DONE`, `FAILED`, `ABANDONED`
- **Team task statuses**: `OPEN`, `TRIAGED`, `ROUTED`, `IN_PROGRESS`, `WAITING_APPROVAL`, `BLOCKED`, `DONE`, `FAILED`, `CANCELLED`

> The kernel-level Task state machine and the harness-level TeamTask state machine remain separate as required by the architecture. The API unifies them under one queryable resource but uses a `task_type` discriminator and type-specific status values.

### Delegations

```http
POST   /v1/delegations
GET    /v1/delegations/{id}
POST   /v1/delegations/{id}/accept
POST   /v1/delegations/{id}/reject
```

**Create request**:
```json
{
  "parent_instance_id": "inst_parent",
  "child_agent_definition_selector": {
    "capability_profile_id": "specialist.analysis",
    "preferred_agent_definition_id": "def_child"
  },
  "task_goal": "Analyze the auth module",
  "return_contract": "structured_result",
  "visible_artifact_ids": ["art_1", "art_2"]
}
```

> The product API preserves capability-layer indirection by accepting a `child_agent_definition_selector` rather than a hard-bound ID. The server performs selector resolution (equivalent to `POST /v1/capability-profiles/{id}/resolve`) before creating the child instance. A `preferred_agent_definition_id` may be supplied as a hint, but the runtime may select a different compatible definition based on current policy and availability.

### Team Definitions

```http
POST   /v1/team-definitions
GET    /v1/team-definitions
GET    /v1/team-definitions/{id}
DELETE /v1/team-definitions/{id}
```

### Team Instances

```http
POST   /v1/team-instances
GET    /v1/team-instances
GET    /v1/team-instances/{id}
DELETE /v1/team-instances/{id}
```

### Team Tasks (Async)

```http
POST /v1/team-instances/{id}/tasks
```

**Request** (`TeamTaskCreate`):
```json
{
  "goal": "Build a login feature",
  "instructions": "Use JWT, keep it minimal",
  "input_artifacts": ["spec_001"],
  "requested_mode": "coordinate",
  "priority": "normal",
  "expected_outputs": ["design_doc", "implementation_plan"],
  "idempotency_key": "teamtask-2026-04-16-001"
}
```

**Response** (`202 Accepted`):
```json
{
  "task_id": "task_abc",
  "team_instance_id": "team_123",
  "status": "OPEN",
  "created_at": "2026-04-16T10:00:00Z"
}
```

Clients poll `GET /v1/tasks/{task_id}` or subscribe via `GET /v1/tasks/{task_id}/events`.

### Shared State & Team Artifacts

```http
GET /v1/team-instances/{id}/shared-state
GET /v1/team-instances/{id}/artifacts
```

### Artifacts

```http
POST   /v1/artifacts
GET    /v1/artifacts
GET    /v1/artifacts/{id}
GET    /v1/artifacts/{id}/content
POST   /v1/artifacts/{id}/publish
DELETE /v1/artifacts/{id}
```

**Artifact scopes**: `private`, `team_shared`, `external_published`

**Publish request**:
```json
{
  "target_scope": "team_shared",
  "publish_reason": "Supervisor accepted research findings",
  "team_instance_id": "team_123",
  "summary": "Concise summary of artifact contents"
}
```

**Publish response** (`200 OK`):
```json
{
  "artifact_id": "art_123",
  "previous_scope": "private",
  "current_scope": "team_shared",
  "published_by": "inst_supervisor",
  "published_at": "2026-04-16T10:00:00Z"
}
```

### Memory Write Candidates

```http
POST   /v1/memory-write-candidates
GET    /v1/memory-write-candidates
GET    /v1/memory-write-candidates/{id}
POST   /v1/memory-write-candidates/{id}/approve
POST   /v1/memory-write-candidates/{id}/reject
```

**Create request**:
```json
{
  "instance_id": "inst_123",
  "content": {
    "category": "user_preference",
    "key": "preferred_language",
    "value": "Rust"
  },
  "reasoning": "User explicitly stated preference for Rust in previous turn."
}
```

**Approve response** (`200 OK`):
```json
{
  "candidate_id": "cand_abc",
  "memory_entry_id": "mem_xyz",
  "status": "approved"
}
```

### Memory Entries

```http
GET /v1/memory-entries
GET /v1/memory-entries/{id}
GET /v1/memory-entries/search
```

**Search query parameters**: `query`, `category`, `instance_id`, `limit`

### Capability Profiles

```http
POST   /v1/capability-profiles
GET    /v1/capability-profiles
GET    /v1/capability-profiles/{id}
DELETE /v1/capability-profiles/{id}
```

**Create request**:
```json
{
  "name": "Research Specialist",
  "description": "Deep research and synthesis",
  "input_contract": { "schema": "..." },
  "output_contract": { "schema": "..." },
  "risk_level": "low",
  "default_agent_definition_id": "def_researcher"
}
```

> The server generates the `id` for capability profiles. Clients may propose an `id` in the create body, but the server reserves the right to normalize or reject it to ensure namespace consistency.

### Capability Registry Bindings

```http
POST   /v1/capability-registry-bindings
GET    /v1/capability-registry-bindings
GET    /v1/capability-registry-bindings/{id}
DELETE /v1/capability-registry-bindings/{id}
```

**Create request**:
```json
{
  "capability_profile_id": "specialist.research",
  "agent_definition_id": "def_researcher",
  "compatibility_score": 0.95,
  "quality_tier": "production"
}
```

### Capability Resolution

```http
POST /v1/capability-profiles/{id}/resolve
```

**Request**:
```json
{
  "team_instance_id": "team_123",
  "team_task_id": "task_abc",
  "selector_id": "analysis_pool",
  "constraints": { "max_active": 2 }
}
```

**Response**:
```json
{
  "candidates": [
    {
      "capability_profile_id": "specialist.research",
      "agent_definition_id": "def_researcher",
      "selection_rationale": "Highest compatibility with task constraints",
      "policy_check_summary": { "approved": true },
      "approval_requirement": "supervisor_local",
      "resource_estimate": { "token_budget": 4000 }
    }
  ]
}
```

### Approvals

```http
GET    /v1/approvals
GET    /v1/approvals/{id}
POST   /v1/approvals/{id}/resolve
```

**Resolve request**:
```json
{
  "resolution": "approved",
  "comment": "Proceed with caution"
}
```

### Checkpoints & Recovery

```http
GET  /v1/agent-instances/{id}/checkpoints
GET  /v1/checkpoints/{id}
POST /v1/checkpoints/{id}/restore
POST /v1/agent-instances/{id}/time-travel
```

**Time-travel request**:
```json
{
  "checkpoint_id": "cp_001",
  "branch_name": "experiment-auth-v2"
}
```

### Events (Read-only)

```http
GET /v1/events?resource_type=agent_instance&resource_id=inst_123
GET /v1/tasks/{id}/events
GET /v1/agent-instances/{id}/events
GET /v1/team-instances/{id}/events
```

**Query parameters**: `before_event_id`, `after_event_id`, `limit`, `event_types[]`

---

## Request/Response Schema Reference

### Common Base Schemas

#### Error
```yaml
Error:
  type: object
  required: [code, message]
  properties:
    code:
      type: string
      description: Machine-readable error code
    message:
      type: string
      description: Human-readable description
    details:
      type: object
    request_id:
      type: string
```

#### Pagination
```yaml
Pagination:
  type: object
  required: [has_more]
  properties:
    next_cursor:
      type: string
      nullable: true
    prev_cursor:
      type: string
      nullable: true
    has_more:
      type: boolean
```

#### Event
```yaml
Event:
  type: object
  required: [event_id, event_type, timestamp, resource_type, resource_id]
  properties:
    event_id: { type: string }
    event_type:
      type: string
      enum:
        - instance_created
        - instance_hydrated
        - execution_requested
        - run_started
        - run_chunk
        - run_tool_call
        - run_tool_result
        - run_delegation_created
        - run_approval_requested
        - run_artifact_produced
        - run_checkpoint_created
        - run_completed
        - run_error
        - task_created
        - task_status_changed
        - task_completed
        - task_failed
        - task_cancelled
        - delegation_created
        - delegation_completed
        - approval_requested
        - approval_resolved
        - artifact_created
        - artifact_published
        - memory_candidate_created
        - memory_candidate_approved
        - memory_candidate_rejected
        - checkpoint_created
        - instance_suspended
        - instance_resumed
        - instance_completed
        - instance_failed
        - instance_cancelled
        - team_created
        - team_task_received
        - triage_completed
        - mode_selected
        - lead_assigned
        - member_activated
        - member_result_accepted
        - member_result_rejected
        - fact_published
        - team_blocked
        - team_unblocked
        - team_completed
        - team_failed
    timestamp: { type: string, format: date-time }
    resource_type:
      type: string
      enum: [agent_instance, team_instance, task, delegation, approval, artifact, memory_write_candidate, memory_entry, capability_profile, capability_registry_binding, checkpoint]
    resource_id: { type: string }
    payload: { type: object }
```

> The `event_type` enum is extensible. Clients must gracefully handle unknown event types by treating them as opaque diagnostics unless explicitly documented above.

### Agent Schemas

#### AgentDefinition / AgentDefinitionCreate
See endpoint examples above. Key fields: `name`, `system_prompt`, `tool_policy`, `memory_policy`, `delegation_policy`, `limits`, `default_model_policy`.

#### AgentInstance / AgentInstanceCreate
Key fields: `agent_definition_id`, `status` (enum), `external_context_refs`, `current_task_id`, `checkpoint_id`.

### Execution Schemas

#### RunRequest
Key fields: `goal` (required), `instructions`, `input_artifacts`, `external_context_refs`, `constraints`, `execution_mode` (`interactive` | `batch` | `recovery`), `expected_outputs`, `idempotency_key`.

#### RunEvent
Envelope: `{ event: string, data: object }`. Event type determines data shape.

### Task Schema

Key fields:
- `id`, `parent_task_id`, `agent_instance_id`, `team_instance_id`
- `task_type`: `agent_task` | `team_task`
- `status`:
  - For `agent_task`: `OPEN` | `IN_PROGRESS` | `BLOCKED` | `DONE` | `FAILED` | `ABANDONED`
  - For `team_task`: `OPEN` | `TRIAGED` | `ROUTED` | `IN_PROGRESS` | `WAITING_APPROVAL` | `BLOCKED` | `DONE` | `FAILED` | `CANCELLED`
- `goal`, `instructions`, `input_artifacts`, `produced_artifacts`
- `delegation_ids`, `approval_ids`, `checkpoint_id`

### Team Schemas

#### TeamDefinition
Key fields:
- `id`, `name`, `description`
- `definition_mode`: `lightweight` | `role_based`
- `supervisor_spec`, `member_specs[]`, `dynamic_selectors[]`
- `available_modes[]`: `route` | `broadcast` | `coordinate` | `tasks`
- `policies`: leadership, delegation, shared_state, approval, resource, failure

#### TeamInstance
Key fields:
- `id`, `team_definition_id`, `supervisor_instance_id`
- `status`: `CREATED` | `TRIAGING` | `ROUTING` | `ORCHESTRATING` | `WAITING_MEMBERS` | `WAITING_APPROVAL` | `BLOCKED` | `SUSPENDED` | `COMPLETED` | `FAILED` | `CANCELLED`
- `active_member_refs`, `active_team_task_ids`, `shared_task_state_id`, `checkpoint_id`

#### TeamTask / TeamTaskCreate
Key fields: `goal`, `instructions`, `input_artifacts`, `external_context_refs`, `constraints`, `requested_mode`, `priority` (`low` | `normal` | `high` | `urgent`), `expected_outputs`.

Task statuses: `OPEN` | `TRIAGED` | `ROUTED` | `IN_PROGRESS` | `WAITING_APPROVAL` | `BLOCKED` | `DONE` | `FAILED` | `CANCELLED` (see Task Schema for `team_task` values).

### Artifact Schema

Key fields:
- `id`, `kind`: `text` | `json` | `file` | `structured_result` | `snapshot`
- `scope`: `private` | `team_shared` | `external_published`
- `source_instance_id`, `published_to_team_instance_id`
- `mime_type`, `size_bytes`, `summary`

### Approval Schema

Key fields:
- `id`, `task_id`, `instance_id`, `team_instance_id`
- `status`: `PENDING` | `APPROVED` | `REJECTED` | `ESCALATED` | `EXPIRED`
- `requested_action`, `reason_set[]`, `resolution`, `comment`, `resolved_by`

### ExternalContextRef Schema

```yaml
ExternalContextRef:
  type: object
  required: [kind, locator]
  properties:
    ref_id: { type: string }
    kind:
      type: string
      enum: [repo, knowledge_base, ticket, project, file_space, conversation_thread, workspace]
    locator: { type: string }
    access_mode:
      type: string
      enum: [read, read_write]
      default: read
    sync_policy: { type: object, nullable: true }
    metadata: { type: object, nullable: true }
```

### MemoryEntry Schema

```yaml
MemoryEntry:
  type: object
  required: [id, category, key, value, created_at]
  properties:
    id: { type: string }
    instance_id: { type: string, nullable: true }
    team_instance_id: { type: string, nullable: true }
    category:
      type: string
      enum: [agent_profile_memory, user_preference_memory, task_or_domain_memory, external_context_memory]
    key: { type: string }
    value: { type: object }
    source_candidate_id: { type: string }
    created_at: { type: string, format: date-time }
    updated_at: { type: string, format: date-time }
```

### MemoryWriteCandidate Schema

```yaml
MemoryWriteCandidate:
  type: object
  required: [id, instance_id, content, status, created_at]
  properties:
    id: { type: string }
    instance_id: { type: string }
    team_instance_id: { type: string, nullable: true }
    content:
      type: object
      required: [category, key, value]
      properties:
        category:
          type: string
          enum: [agent_profile_memory, user_preference_memory, task_or_domain_memory, external_context_memory]
        key: { type: string }
        value: { type: object }
    reasoning: { type: string }
    status:
      type: string
      enum: [PENDING, APPROVED, REJECTED]
    memory_entry_id: { type: string, nullable: true }
    reviewed_by: { type: string, nullable: true }
    created_at: { type: string, format: date-time }
    reviewed_at: { type: string, format: date-time, nullable: true }
```

### CapabilityProfile Schema

```yaml
CapabilityProfile:
  type: object
  required: [id, name, created_at]
  properties:
    id: { type: string }
    name: { type: string }
    description: { type: string, nullable: true }
    input_contract: { type: object, nullable: true }
    output_contract: { type: object, nullable: true }
    risk_level:
      type: string
      enum: [low, medium, high, critical]
    default_agent_definition_id: { type: string, nullable: true }
    created_at: { type: string, format: date-time }
    updated_at: { type: string, format: date-time }
```

### CapabilityProfileCreate Schema

```yaml
CapabilityProfileCreate:
  type: object
  required: [name]
  properties:
    id: { type: string, description: "Optional proposed ID; server may normalize or reject" }
    name: { type: string }
    description: { type: string, nullable: true }
    input_contract: { type: object, nullable: true }
    output_contract: { type: object, nullable: true }
    risk_level:
      type: string
      enum: [low, medium, high, critical]
    default_agent_definition_id: { type: string, nullable: true }
```

### CapabilityRegistryBinding Schema

```yaml
CapabilityRegistryBinding:
  type: object
  required: [id, capability_profile_id, agent_definition_id, created_at]
  properties:
    id: { type: string }
    capability_profile_id: { type: string }
    agent_definition_id: { type: string }
    compatibility_score:
      type: number
      minimum: 0
      maximum: 1
    quality_tier:
      type: string
      enum: [experimental, beta, production]
    metadata: { type: object, nullable: true }
    created_at: { type: string, format: date-time }
    updated_at: { type: string, format: date-time }
```

### Checkpoint Schema

Key fields: `id`, `instance_id` or `team_instance_id`, `status_snapshot`, `event_anchor_id`, `created_at`.

---

## OpenAPI 3.1 Document Structure

**File**: `docs/openapi/torque-v1.yaml`

```yaml
openapi: 3.1.0
info:
  title: Torque Platform API
  version: 1.0.0
  description: External REST API for Torque Agent Runtime and Harness

servers:
  - url: https://api.torque.dev/v1

security:
  - ApiKeyAuth: []

tags:
  - name: Agent Definitions
  - name: Agent Instances
  - name: Runs
  - name: Tasks
  - name: Delegations
  - name: Team Definitions
  - name: Team Instances
  - name: Team Tasks
  - name: Artifacts
  - name: Memory Write Candidates
  - name: Memory Entries
  - name: Capability Profiles
  - name: Capability Registry Bindings
  - name: Approvals
  - name: Checkpoints
  - name: Events

paths:
  /agent-definitions: { ... }
  /agent-instances: { ... }
  /agent-instances/{id}/runs: { ... }
  /tasks: { ... }
  /delegations: { ... }
  /team-definitions: { ... }
  /team-instances: { ... }
  /team-instances/{id}/tasks: { ... }
  /artifacts: { ... }
  /approvals: { ... }
  /checkpoints: { ... }
  /events: { ... }

components:
  securitySchemes:
    ApiKeyAuth:
      type: apiKey
      in: header
      name: X-API-Key
  schemas:
    # All schemas listed in the previous section
```

---

## List Endpoint Query Semantics

All `GET /v1/{resources}` collection endpoints support the following query parameters unless otherwise noted:

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `limit` | integer | 20 | Maximum items to return (max 100) |
| `cursor` | string | — | Pagination cursor from previous response |
| `sort` | string | `-created_at` | Sort field, prefix `-` for descending |
| `filter[status]` | string | — | Filter by exact status match (supports comma-separated OR) |
| `filter[created_after]` | ISO 8601 | — | Inclusive lower bound on `created_at` |
| `filter[created_before]` | ISO 8601 | — | Inclusive upper bound on `created_at` |

**Response envelope**:
```json
{
  "data": [ ... ],
  "pagination": {
    "next_cursor": "cursor_abc",
    "prev_cursor": null,
    "has_more": true
  }
}
```

Resource-specific filters:
- `GET /v1/tasks` — additionally supports `filter[agent_instance_id]`, `filter[team_instance_id]`, `filter[parent_task_id]`
- `GET /v1/agent-instances` — additionally supports `filter[agent_definition_id]`, `filter[current_task_id]`
- `GET /v1/events` — supports `resource_type`, `resource_id`, `before_event_id`, `after_event_id`, `event_types[]`
- `GET /v1/memory-entries` — supports `filter[category]`, `filter[instance_id]`
- `GET /v1/artifacts` — supports `filter[scope]`, `filter[source_instance_id]`, `filter[kind]`

---

## Lifecycle Transition Rules

### Agent Instance Lifecycle Guards

| Current Status | Allowed Actions | Forbidden Actions (return 409) |
|----------------|-----------------|--------------------------------|
| `CREATED` | `GET`, `DELETE` | `POST /runs` |
| `HYDRATING` | `GET` | `POST /runs`, `DELETE` |
| `READY` | `GET`, `POST /runs`, `DELETE` | — |
| `RUNNING` | `GET`, `POST /cancel` | `POST /runs`, `DELETE` |
| `WAITING_*` | `GET`, `POST /cancel` | `POST /runs`, `DELETE` |
| `SUSPENDED` | `GET`, `POST /resume` | `POST /runs`, `DELETE` |
| `COMPLETED` | `GET`, `DELETE` | `POST /runs` |
| `FAILED` | `GET`, `DELETE` | `POST /runs` |
| `CANCELLED` | `GET`, `DELETE` | `POST /runs` |

### Task Lifecycle Guards

- A task in `OPEN` or `IN_PROGRESS` may be canceled via `POST /v1/tasks/{id}/cancel`
- A task in `BLOCKED` may only be canceled if the blocker is resolved or the parent instance/team is being torn down
- A task in `DONE`, `FAILED`, `ABANDONED`, or `CANCELLED` is immutable; `cancel` returns `409 Conflict`

### Team Instance Lifecycle Guards

- `DELETE /v1/team-instances/{id}` is allowed only when `status` is `CREATED`, `COMPLETED`, `FAILED`, or `CANCELLED`
- Active team instances (`TRIAGING` through `WAITING_*`) must first be canceled or complete naturally

### Idempotency Rules

- `RunRequest.idempotency_key`: if a run with the same `idempotency_key` is already active for the instance, return `409` with `code: IDEMPOTENT_RUN_IN_PROGRESS`. If completed, return the cached result (or a reference to it).
- `TeamTaskCreate.idempotency_key`: **required**. A retry with the same key returns `202 Accepted` with the existing task reference, regardless of whether the task is still active or completed.
- `ArtifactCreate.idempotency_key`: same semantics as runs. Prevents duplicate artifact uploads.

---

## Authentication and Security

- **Mechanism**: API key passed in `X-API-Key` header
- **All endpoints require authentication** unless explicitly documented otherwise
- Future versions may add OAuth2 or mTLS for inter-service calls, but v1 keeps a single API key model for simplicity

---

## Error Handling

All errors return a JSON body matching the `Error` schema.

Common HTTP status codes:

| Status | Meaning |
|--------|---------|
| `400` | Invalid request parameters |
| `401` | Missing or invalid API key |
| `403` | Insufficient permissions |
| `404` | Resource not found |
| `409` | Resource conflict (e.g., concurrent run on same instance) |
| `422` | Semantic validation failure |
| `429` | Rate limited |
| `500` | Internal server error |

---

## Versioning Strategy

- **URL versioning**: all routes prefixed with `/v1/`
- **Breaking changes** require a new major version (`/v2/`)
- **Additive changes** (new optional fields, new event types, new endpoints) are allowed within `/v1/`
- **Extensible enums**: string enums in schemas are documented as extensible; clients must gracefully handle unknown values

---

## Migration from MVP API

| MVP Endpoint | v1 Replacement | Notes |
|--------------|----------------|-------|
| `POST /sessions` | `POST /v1/agent-instances` | Session becomes agent instance |
| `GET /sessions` | `GET /v1/agent-instances` | List instances instead of sessions |
| `GET /sessions/{id}` | `GET /v1/agent-instances/{id}` | Instance metadata |
| `GET /sessions/{id}/messages` | `GET /v1/tasks/{task_id}/events?event_types=run.chunk` | Message history as run events |
| `POST /sessions/{id}/chat` | `POST /v1/agent-instances/{id}/runs` | SSE semantics preserved; event model richer |
| `POST /sessions/{id}/memory/candidates` | `POST /v1/memory-write-candidates` | Nominations for durable memory |
| `POST /sessions/{id}/memory/candidates/{id}/accept` | `POST /v1/memory-write-candidates/{id}/approve` | Approved candidates become memory entries |
| `GET /sessions/{id}/memory` | `GET /v1/memory-entries` | Durable memory entries |
| `GET /metrics` | Service-level `/metrics` (Prometheus) | Metrics endpoint not under `/v1/` in v1 |

---

## Concurrency and Reliability Contracts

### Per-Instance Run Gate

- Only one `/runs` request may be active per `AgentInstance` at a time
- Concurrent request receives `409 Conflict`
- Same rule applies to `TeamInstance` tasks where a task is in `IN_PROGRESS`

### Idempotency

- `RunRequest.idempotency_key` allows safe retry of run initiation
- `TeamTaskCreate.idempotency_key` is required for safe retry of async 202 tasks

### SSE Reliability

- Terminal event (`run.completed` or `run.error`) is mandatory and unique
- Stream close without terminal event = transport failure; client should reconcile via `GET /v1/tasks/{task_id}`

---

## Relationship to Internal Architecture

This API sits **above** the Kernel/Harness boundary defined in the Torque architecture specs.

- **Kernel-layer objects** (`ExecutionRequest`, `AgentInstance`, `Task`, `DelegationRequest`) are mapped into REST resources but not exposed 1:1
- **Harness-layer objects** (`TeamDefinition`, `TeamInstance`, `TeamTask`, `SharedTaskState`) are exposed through product-friendly endpoints
- **Context planes** (`Artifact`, `Memory`, `ExternalContextRef`) remain separate in the API as they are in the architecture
- **Recovery** (`Checkpoint`, `Event`, time-travel) is exposed as read/query + restore actions

The API does not leak:
- internal prompt templates
- raw model token streams (only semantic chunks)
- kernel event log implementation details
- capability registry resolution internals

---

## Decisions on Previously Open Questions

### TeamTask Idempotency
**Decision**: `TeamTaskCreate` **must** include an `idempotency_key` field in v1. This is required for safe retry of async 202 tasks.

### Event Subscription Model
**Decision**: `GET /v1/events` is **polling-only** in v1. SSE or WebSocket real-time subscription may be added in a future version. Long-polling is not supported.

### Artifact Upload Mechanism
**Decision**: `POST /v1/artifacts` accepts **JSON-only** in v1, with file content base64-encoded inside the JSON payload. Multipart/form-data support is deferred to v1.1 or later.

### Metrics Surface
**Decision**: Prometheus exposition remains at `/metrics` (global service level) and is **not** namespaced under `/v1/`. A JSON summary endpoint `/v1/metrics` may be added later for API consumers, but is out of scope for v1.

---

## Summary

The Torque Platform API is a resource-oriented REST interface that replaces the MVP session-agent API with a full product contract for:

- agent definition and instance lifecycle
- streaming runs via SSE
- supervisor-driven team orchestration via async tasks
- artifact, memory, delegation, approval, checkpoint, and event management

It is versioned under `/v1/`, specified in OpenAPI 3.1, and aligned with the Kernel/Harness architecture without exposing internal implementation details.
