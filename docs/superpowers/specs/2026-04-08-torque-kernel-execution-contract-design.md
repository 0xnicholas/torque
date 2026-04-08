# Torque Kernel Execution Contract Design

## Overview

This document defines the current design direction for the **kernel execution contract** in Torque.

The goal is to make the kernel execution model explicit without importing upper-layer workflow, playbook, or team semantics into the kernel itself.

The kernel should revolve around a small set of runtime objects:

- `ExecutionRequest`
- `AgentDefinition`
- `AgentInstance`
- `Task`
- `ExecutionResult`
- `DelegationRequest`
- `DelegationResult`
- `ApprovalRequest`

**Date**: 2026-04-08  
**Status**: Draft  
**Scope**: ExecutionRequest, AgentInstance, Task, ExecutionResult, delegation relationship, lifecycle boundaries

---

## 1. Design Goals

- Define a clean kernel-level execution entry contract
- Keep execution instance ownership explicit
- Keep work-item semantics separate from execution-entry semantics
- Make delegation a first-class kernel contract
- Return progression results rather than only final answers
- Prevent workflow/playbook semantics from leaking into the kernel

## 2. Non-Goals

- The kernel does not treat `ExecutionRequest` as a workflow definition
- The kernel does not treat `Task` as a playbook step or graph node
- The kernel does not let one `ExecutionRequest` carry a whole orchestration plan
- The kernel does not turn one `AgentInstance` into an internal multi-task scheduler
- The kernel does not require team semantics to exist in order to execute work

---

## 3. Core Execution Chain

The recommended kernel execution chain is:

`ExecutionRequest`
-> create or continue `AgentInstance`
-> assign or continue `Task`
-> instance executes
-> may produce `Artifact`, `DelegationRequest`, `ApprovalRequest`, `MemoryWriteCandidate`, `Checkpoint`
-> runtime returns `ExecutionResult`

This chain should remain stable across:

- direct user or API calls
- team-supervisor initiated execution
- scheduler-driven execution
- replay or resume paths

---

## 4. ExecutionRequest

### 4.1 Purpose

`ExecutionRequest` is the standard kernel-level intent object for entering the runtime.

It answers:

"What execution should the kernel drive right now?"

It is not merely an HTTP payload or SDK transport shape.

### 4.2 Multiple Sources, One Kernel Shape

An execution request may originate from:

- external API
- scheduler
- team supervisor
- replay or resume path
- another orchestration layer

But once it reaches the kernel, it should be represented as the same class of execution-intent object.

### 4.3 One Request, One Current Task Intent

Recommended rule:

- one `ExecutionRequest` should submit one current task intent

If a higher layer needs multiple work items, branching, or decomposition, that should happen above the kernel and eventually lower into multiple requests or delegations.

This keeps the kernel contract simple and avoids recreating a workflow engine inside the execution core.

### 4.4 Suggested Minimum Fields

Conceptually, `ExecutionRequest` should include:

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

Field names may evolve, but the semantic role should remain stable.

---

## 5. AgentDefinition

### 5.1 Purpose

`AgentDefinition` is the static execution template.

It defines:

- identity
- system prompt
- tool policy
- memory policy
- delegation policy
- limits
- default model policy

It does not represent a live execution session.

---

## 6. AgentInstance

### 6.1 Purpose

`AgentInstance` is the kernel's execution center.

It is the live execution owner, not merely a temporary wrapper around a task.

### 6.2 Owned State

An agent instance owns:

- message and working context
- tool loop state
- checkpoint lineage
- active task references
- private scratch state
- pending approvals
- child delegation references

### 6.3 Lifecycle Independence

An `AgentInstance` may outlive an individual `Task`.

This allows:

- continuity of private execution context
- checkpoint continuity
- stable policy and tool environment
- multi-step work over time without forcing a brand-new instance each time

### 6.4 Single Active Primary Task Rule

Even though an instance may outlive multiple tasks over time, it should have only one active primary task at a time.

If true concurrency is needed, the system should prefer:

- multiple instances
- or explicit delegation to child instances

This prevents the instance from turning into an internal task scheduler.

---

## 7. Task

### 7.1 Purpose

`Task` is the runtime-level work item assigned to an `AgentInstance`.

It answers:

"What work is this instance currently trying to complete?"

### 7.2 Minimum Semantic Contents

A task should express:

- goal
- instructions
- input references
- constraints
- expected outputs

### 7.3 What Task Is Not

`Task` is not:

- a playbook step
- a graph node
- a whole session
- a delegation policy object

### 7.4 Relationship to ExecutionRequest

`ExecutionRequest` and `Task` must remain distinct:

- `ExecutionRequest`
  is the runtime entry intent
- `Task`
  is the current work item the instance executes
- `AgentInstance`
  is the execution owner

The kernel should therefore interpret a request as:

- create or continue an instance
- then assign, replace, or continue one current task context

---

## 8. ExecutionResult

### 8.1 Purpose

`ExecutionResult` should default to being a progression result, not merely a final answer blob.

It answers:

"How far did this execution progress, and what objects or state transitions did it produce?"

### 8.2 Suggested Contents

Conceptually, an execution result may include:

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

### 8.3 Why Progression Matters

Returning progression rather than only final text makes the runtime compatible with:

- agentic execution
- approval gates
- delegation
- replay and resume
- structured artifact production
- downstream orchestration

---

## 9. Delegation in the Kernel Contract

### 9.1 Purpose

`DelegationRequest` and `DelegationResult` belong to the kernel execution contract, not only to team semantics.

They express how one instance can explicitly and audibly delegate constrained work to another instance.

### 9.2 Core Pattern

The default delegation chain is:

`Parent AgentInstance`
-> creates `DelegationRequest`
-> runtime creates or continues `Child AgentInstance`
-> child works on constrained `Task`
-> child returns `DelegationResult`

### 9.3 Design Expectations

Kernel-level delegation should remain:

- explicit
- auditable
- conservative by default
- parent-controlled

Child instances should not be modeled as free-form peer chats.

---

## 10. Lifecycle Boundaries

### 10.1 Instance States

Suggested `AgentInstance` states:

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

### 10.2 Task States

Suggested `Task` states:

- `OPEN`
- `IN_PROGRESS`
- `BLOCKED`
- `DONE`
- `FAILED`
- `ABANDONED`

These state machines must remain separate.

Important rule:

- instance state describes execution lifecycle
- task state describes work-item lifecycle

Neither should be inferred naively from the other.

---

## 11. Summary Rules

Recommended kernel summary:

- `ExecutionRequest`
  is the kernel entry intent
- `AgentInstance`
  is the execution owner
- `Task`
  is the current work item
- `ExecutionResult`
  is the progression result
- `DelegationRequest` and `DelegationResult`
  are first-class kernel control contracts

Recommended hard rules:

- one `ExecutionRequest` carries one current task intent
- one `AgentInstance` may outlive a task
- one `AgentInstance` should have only one active primary task at a time
- workflow and playbook semantics stay above the kernel

---

## 12. Open Questions

- Should the kernel expose a first-class "continue task" action distinct from generic execution request continuation?
- How should idempotency be enforced across repeated `ExecutionRequest`s that target the same instance and logical work?
- Which portions of `ExecutionResult` should be required synchronously versus made available through later event/query APIs?
- How much task replacement or task supersession should the kernel support within one long-lived instance?
