# Torque Architecture

## Overview

Torque is a general-purpose **Agent Runtime / Harness** platform implemented in Rust.

The project is intentionally split into two layers:

- **Kernel**: the execution substrate for long-running, stateful, recoverable agents
- **Harness**: higher-level orchestration and batteries-included agent/team capabilities built on top of the kernel

Torque is **agent-centric**, not DAG-centric. It does not require a built-in planner, workflow DSL, or workspace domain model.

The current authoritative design document is [`docs/superpowers/specs/2026-04-08-torque-agent-runtime-harness-design.md`](./superpowers/specs/2026-04-08-torque-agent-runtime-harness-design.md).

## Layered Architecture

```text
Upper-layer orchestration systems
  -> compile to standard runtime requests

Torque Harness
  -> TeamDefinition
  -> TeamInstance
  -> TeamTask
  -> orchestration modes
  -> built-in prompts, routines, planning, collaboration features

Torque Kernel
  -> AgentDefinition
  -> AgentInstance
  -> ExecutionRequest
  -> Task
  -> Artifact
  -> Event
  -> Checkpoint
  -> ApprovalRequest
  -> MemoryWriteCandidate
  -> ExternalContextRef

Storage / Adapters
  -> PostgreSQL / Redis
  -> object storage
  -> vector memory backends
  -> external context systems
```

## Kernel Model

The kernel treats `AgentInstance` as the execution center.

- `AgentDefinition` defines identity, policies, limits, and tool boundaries
- `AgentInstance` is a live execution with its own working state, checkpoints, approvals, and child delegations
- `Task` is a runtime-level work item delegated to an instance
- `ExecutionRequest` is the standard runtime entrypoint
- `ExternalContextRef` represents mounted external context without making Torque own that domain model

Execution is instance-centric:

1. `Instantiate`
2. `Hydrate`
3. `Deliberate`
4. `Act`
5. `Checkpoint`
6. `Publish`
7. `Suspend / Resume / Complete / Fail`

## Data Planes

Torque keeps three planes separate:

- **Artifact Plane**: precise outputs and execution results
- **Memory Plane**: semantic recall derived from selected content
- **External Context Plane**: references to external repos, knowledge bases, file spaces, tickets, or workspace-like systems

Key constraints:

- artifacts default to private scope and become shared only through explicit publish/promote
- memory is derived and retrievable, but is not the source of truth
- external context is referenced, not owned

## Team Model

`Team` is a first-class object in the **Harness** layer, not in the **Kernel** layer.

Harness-level team concepts:

- `TeamDefinition`
- `TeamInstance`
- `TeamTask`
- team modes such as `coordinate`, `route`, `broadcast`, and `tasks`

Default collaboration is strongly biased toward:

`Supervisor -> Subagent`

This is a deliberate constraint for:

- context isolation
- predictable delegation
- better recovery
- cleaner observability

Peer handoff is an explicit advanced action, not the default.

## Delegation Model

Delegation is a standard runtime contract, not an informal chat between agents.

The core pattern is:

`Supervisor AgentInstance`
-> `DelegationRequest`
-> `Child AgentInstance`
-> `DelegationResult`

Default delegation is conservative:

- child instances do not inherit the parent's full history
- child instances receive only explicitly passed artifacts and visible external context refs
- child outputs remain private until accepted and published

## Recovery Model

Torque uses an **event-sourced, snapshot-assisted** recovery model.

- `Event Log` is the factual source of truth
- `Checkpoint` is the fast recovery layer
- recovery hydrates from the latest checkpoint and replays only the tail events after it
- time travel creates a new lineage branch instead of mutating history

External side effects must be tracked with effect metadata and idempotency information so recovery does not blindly repeat committed actions.

## Current Documentation Direction

Torque no longer documents itself as:

- a planner-led DAG execution system
- a built-in workspace platform
- a product-specific playbook runtime

Instead, current docs should assume:

- a neutral runtime kernel
- a harness layer for built-in team and agent capabilities
- standard runtime interfaces for integration with upper-layer systems
