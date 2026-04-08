# Torque Policy Model Design

## Overview

This document defines the current design direction for the **policy model** in Torque.

Torque should treat policy as a composable governance rules system, not merely as scattered static config blocks.

The purpose of this model is to make policy:

- composable
- inspectable
- auditable
- conservative by default
- reusable across runtime, team, capability, and context layers

**Date**: 2026-04-08  
**Status**: Draft  
**Scope**: Policy sources, evaluation model, merge semantics, PolicyDecision shape, dimensional boundaries

---

## 1. Design Goals

- Define policy as a first-class governance layer
- Support policy composition across multiple object layers
- Avoid ambiguous "last writer wins" behavior
- Keep policy evaluation separate from runtime action execution
- Make policy outcomes explicit and structured
- Reuse one policy model across approval, delegation, visibility, resource, memory, and tool control

## 2. Non-Goals

- Torque does not model policy as only boolean allow/deny flags
- Torque does not treat policy as a bag of unrelated JSON blobs
- Torque does not let policy evaluation directly perform orchestration actions
- Torque does not collapse all policy dimensions into one flat override chain
- Torque does not assume every policy source can affect every decision dimension

---

## 3. Core Principle

The recommended policy model is:

`policy inputs -> dimensional evaluation -> conservative merge -> PolicyDecision`

This means:

- policy is not merely stored
- policy is evaluated
- evaluation happens per decision dimension
- merged results are returned as structured governance outcomes

---

## 4. Policy as a Governance Rules System

### 4.1 Not Just Static Config

Policy may be authored and stored as configuration, but the system should not conceptually stop there.

In Torque, policy should be understood as:

- rules
- constraints
- limits
- gates
- decision modifiers

that are evaluated against a subject and execution context.

### 4.2 Why This Matters

This is necessary because Torque already has policy interactions that cannot be expressed well by simple static override:

- approval requirements
- delegation constraints
- visibility narrowing
- resource admission
- memory write control
- tool risk controls

These require evaluation and merge semantics, not only configuration lookup.

---

## 5. Policy Dimensions

Policy should be evaluated per dimension rather than as one undifferentiated bundle.

Recommended initial dimensions:

- `approval`
- `visibility`
- `delegation`
- `resource`
- `memory`
- `tool`

Each dimension should be allowed to define its own evaluation semantics and result shape.

Important rule:

- different dimensions should not silently override one another

For example:

- visibility policy should not directly change approval outcome
- memory policy should not directly become publish policy
- resource policy should not directly redefine delegation authority

---

## 6. Policy Sources

Torque should support multiple policy sources, but they should not be treated as one universal override stack.

Recommended source layers:

### 6.1 System or Global Policy

System-wide hard boundaries and baseline governance, such as:

- globally prohibited tools
- mandatory audit requirements
- minimum safety floors

### 6.2 Capability Policy

Capability-level expectations and risk rules, such as:

- review requirements
- execution risk constraints
- quality floors
- output handling expectations

### 6.3 AgentDefinition Policy

Execution-template-specific constraints, such as:

- tool permissions
- runtime limits
- default memory behavior
- delegation limits

### 6.4 Team Policy

Collaboration governance, such as:

- who may delegate
- who may publish
- approval routing
- shared visibility boundaries
- resource sharing within team execution

### 6.5 Selector or Binding Policy

Local constraints attached to selection or resolution, such as:

- allowed candidate classes
- local approval gates
- environment-specific availability
- local risk restrictions

### 6.6 Runtime Signal or Local Override

Current execution-context facts, such as:

- budget pressure
- observed side-effect risk
- member requesting approval
- transient execution constraints

These are still policy inputs, but they should usually act as local tightening signals, not broad semantic redefinitions.

---

## 7. Source Hierarchy Is Not Universal Override Hierarchy

Torque should not model policy as:

`system -> capability -> agent -> team -> selector -> runtime`, where lower layers simply override everything above.

That model is too blunt and will produce incorrect behavior.

Recommended rule:

- first determine which policy sources are allowed to affect a given dimension
- then merge those sources conservatively within that dimension

This means:

- not every source may speak on every dimension
- lower-level locality does not automatically grant broad override power

---

## 8. Merge Semantics

### 8.1 Dimensional Evaluation

Policy evaluation should happen per dimension.

For example:

- approval sources are evaluated together
- visibility sources are evaluated together
- resource sources are evaluated together

Then the dimensional outputs are assembled into one `PolicyDecision`.

### 8.2 Conservative Merge

Within a dimension, the default merge rule should be conservative.

Recommended default:

- the more restrictive applicable rule wins

Examples:

- if any approval source requires external approval, local approval is insufficient
- if any tool policy denies a tool, the tool is not allowed
- if any visibility rule narrows the visible scope, broader visibility should not silently prevail

### 8.3 Not Last Writer Wins

Torque should explicitly avoid simple "last writer wins" merge semantics for governance-critical decisions.

That model is too opaque and too easy to misuse.

---

## 9. PolicyDecision

### 9.1 Purpose

`PolicyDecision` is the structured result of policy evaluation.

It is the object that runtime, team, resolver, and execution layers consume after policy rules have been applied.

### 9.2 Not an Action Executor

`PolicyDecision` is an evaluated governance result, not an action executor.

It should not directly:

- create approval requests
- instantiate members
- publish artifacts
- retry tasks
- perform side effects

Those actions remain the responsibility of runtime or orchestration layers.

### 9.3 Suggested Shape

Conceptually, a policy decision should be close to:

```json
{
  "decision_id": "policy-decision-123",
  "subject_ref": "delegation://abc",
  "applicable_sources": ["system", "team", "capability"],
  "overall": {
    "allowed": true,
    "requires_followup": true
  },
  "dimensions": {
    "approval": {},
    "visibility": {},
    "delegation": {},
    "resource": {},
    "memory": {},
    "tool": {}
  },
  "reason_set": ["capability_risk_policy", "tool_side_effect"],
  "effective_at": "2026-04-08T00:00:00Z"
}
```

Field names may evolve, but the semantic structure should remain:

- one evaluated decision object
- shared metadata
- per-dimension results
- explicit reasons

### 9.4 Overall vs Dimensional Results

`overall` may help callers quickly determine whether execution is generally allowed, but the dimensional decisions are the real substance.

This matters because many valid outcomes are not reducible to simple allow/deny:

- allow, but require approval
- allow, but narrow visibility
- allow, but prohibit child delegation
- allow, but deny memory write

---

## 10. Dimension Result Examples

Each dimension may expose its own structured result shape.

Examples:

### 10.1 Approval

- `none`
- `supervisor_local`
- `external_required`
- reason set

### 10.2 Visibility

- allowed scopes
- denied scopes
- slice narrowing instructions

### 10.3 Delegation

- allowed
- max depth
- child delegation allowed
- handoff allowed

### 10.4 Resource

- allowed
- budget guard
- concurrency cap
- timeout class
- defer or deny hints

### 10.5 Memory

- write allowed
- candidate only
- denied

### 10.6 Tool

- allowlist or denylist effects
- privileged action gates
- side-effect approval requirements

---

## 11. Recommended Evaluation Flow

The recommended policy evaluation flow is:

1. identify subject and decision context
2. identify relevant dimensions
3. collect applicable policy sources per dimension
4. evaluate each dimension separately
5. apply conservative merge within each dimension
6. assemble one `PolicyDecision`
7. hand the decision to runtime or orchestration for action

This keeps policy evaluation explicit and debuggable.

---

## 12. Policy Subjects

The policy system should be usable against multiple subjects, including:

- execution request
- task
- delegation request
- delegation result handling
- tool call
- artifact publish
- memory write candidate
- selector resolution
- team action

This is one reason the policy model should remain generic and composable.

---

## 13. Summary Rules

Recommended summary:

- policy is a composable governance rules system
- policy is evaluated per dimension
- merge is conservative within each dimension
- not every source can affect every dimension
- evaluation produces `PolicyDecision`
- runtime consumes the decision but does not delegate action execution to the policy layer

In short:

`dimensioned evaluation + conservative merge + structured decision object`

---

## 14. Open Questions

- Which dimensions should be mandatory in the initial implementation, and which can remain optional?
- Should system/global policy be hard-coded, config-driven, or registry-managed?
- How much of policy evaluation should be cached versus recomputed each time?
- Should tenant-specific policy be modeled as part of team policy, system policy, or a separate source layer?
