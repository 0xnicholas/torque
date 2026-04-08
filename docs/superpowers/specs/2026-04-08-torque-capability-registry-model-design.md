# Torque Capability Registry Model Design

## Overview

This document defines the current design direction for **capability identity, capability registration, implementation binding, and runtime capability resolution** in Torque.

Torque should distinguish:

- the thing upper layers ask for
- the capability contract being referenced
- the concrete implementations available
- the runtime candidate set selected under current constraints

These are related, but they are not the same object.

**Date**: 2026-04-08  
**Status**: Draft  
**Scope**: CapabilityRef, CapabilityProfile, CapabilityRegistryBinding, CapabilityResolution, registry responsibilities

---

## 1. Design Goals

- Separate capability reference from capability definition
- Separate capability definition from concrete implementation binding
- Support stable authoring references without coupling upper layers to specific agents
- Allow runtime candidate resolution under policy and resource constraints
- Keep capability identity usable across team, workflow, and orchestration layers
- Avoid collapsing capability, implementation, and runtime choice into one object

## 2. Non-Goals

- Torque does not treat `CapabilityRef` as a direct agent call
- Torque does not treat `CapabilityProfile` as a runtime member instance
- Torque does not make `CapabilityRegistry` a full orchestration control plane
- Torque does not allow fuzzy unrestricted capability discovery by default
- Torque does not require playbook or workflow semantics to exist inside the kernel

---

## 3. Core Layering

The recommended layering is:

`CapabilityRef`
-> resolves to canonical `CapabilityProfile`
-> registry binding maps profile to candidate `AgentDefinition`s
-> runtime produces `CapabilityResolution`
-> upper layer selects a candidate
-> runtime creates `AgentInstance` or `MemberInstance`

These layers answer different questions:

- `CapabilityRef`
  what ability is being requested?
- `CapabilityProfile`
  what does that ability mean?
- `CapabilityRegistryBinding`
  which implementations can satisfy it?
- `CapabilityResolution`
  which candidates are valid for this run right now?

---

## 4. CapabilityRef

### 4.1 Purpose

`CapabilityRef` is the lightweight capability reference used by upper layers.

It answers:

"What ability is needed here?"

Examples of where it may appear:

- playbook-like authoring
- workflow-like authoring
- team definition
- selector definition
- planning or routing outputs

### 4.2 Initial Shape

In the initial design, `CapabilityRef` may be authoring-friendly and string-shaped, for example:

```txt
capability.analysis.summarize_findings
```

However, it should be treated semantically as a lightweight reference object, not merely as an arbitrary string literal.

### 4.3 Future Evolution

The model should leave room for future evolution such as:

- alias resolution
- version range resolution
- qualifier fields
- environment-specific lookup hints

This means:

- authoring may remain string-friendly
- the semantic model should still be richer than "raw string forever"

### 4.4 Hard Boundary

`CapabilityRef` should not directly identify a concrete `AgentDefinition`.

It identifies requested ability, not execution implementation.

---

## 5. CapabilityProfile

### 5.1 Purpose

`CapabilityProfile` is the canonical ability contract stored in the registry.

It answers:

"What does this capability actually mean?"

### 5.2 Minimum Semantic Contents

A capability profile should define at least:

- purpose
- input contract
- output contract
- routine or tool expectations
- quality expectations
- risk or execution policy

It should be a real contract object, not just an abstract label.

### 5.3 What It Is Not

`CapabilityProfile` is not:

- a direct alias for one agent implementation
- a runtime instance
- a full workflow definition
- a team or role by itself

It is the reusable ability definition layer.

### 5.4 Relationship to Team

For team-facing design, `CapabilityProfile` is the preferred ability layer.

This keeps team definitions stable while allowing implementations to change over time.

---

## 6. CapabilityRegistryBinding

### 6.1 Purpose

`CapabilityRegistryBinding` maps a canonical capability profile to one or more candidate execution implementations.

It answers:

"Which implementations can currently satisfy this capability?"

### 6.2 Why It Must Be Separate

This binding should remain separate from `CapabilityProfile` so that:

- capability contracts stay stable
- implementation choice can evolve
- environment-specific bindings are possible
- version and deprecation metadata can be managed without redefining the capability itself

### 6.3 Typical Binding Contents

Binding data may include:

- candidate `AgentDefinition` references
- compatibility metadata
- availability metadata
- alias and deprecation metadata
- version mapping information

It should not itself become a runtime orchestration decision object.

### 6.4 Relationship to Registry

`CapabilityRegistry` owns and exposes these bindings as part of its directory function.

---

## 7. CapabilityResolution

### 7.1 Purpose

`CapabilityResolution` is the runtime candidate resolution result.

It answers:

"Under current constraints, which candidates are valid for this capability right now?"

### 7.2 Runtime Inputs

Resolution may depend on:

- canonical capability profile
- registry bindings
- current policy constraints
- selector or team constraints
- approval context
- resource or concurrency limits
- task-local intent

### 7.3 Output Shape

`CapabilityResolution` should return an ordered candidate set, not a final runtime member instance.

Each candidate should conceptually include:

- `capability_profile_ref`
- `agent_definition_ref`
- `match_rationale`
- `policy_check_summary`
- `risk_level`
- `cost_or_latency_estimate`

### 7.4 Hard Boundary

`CapabilityResolution` does not itself:

- create runtime instances
- finalize team-level selection
- bypass approval or selector governance

It returns candidates for an upper layer to choose from.

---

## 8. CapabilityRegistry

### 8.1 Purpose

`CapabilityRegistry` should be a lightweight governed directory system.

It should not be merely a dumb lookup table, but it also should not become a full orchestration control plane.

### 8.2 Recommended Responsibilities

The registry should be responsible for:

- registering canonical `CapabilityProfile`s
- resolving `CapabilityRef` to canonical profiles
- maintaining alias metadata
- maintaining version and deprecation metadata
- maintaining profile-to-implementation bindings
- exposing compatibility metadata needed for runtime resolution

### 8.3 Non-Responsibilities

The registry should not directly own:

- runtime member creation
- final selector choice
- team supervision
- approval outcome decisions
- workflow or playbook semantics

These belong to upper orchestration and runtime layers.

### 8.4 Design Rule

`CapabilityRegistry` is a capability directory and resolution boundary, not an orchestration brain.

---

## 9. Alias and Version Resolution

### 9.1 Stable Indirection

`CapabilityRef` should support stable indirection.

Recommended supported behaviors:

- alias -> canonical profile
- deprecated ref -> replacement canonical profile
- version-aware lookup when needed

### 9.2 What To Avoid

Torque should avoid defaulting to fuzzy capability-family search or open-ended semantic discovery.

Recommended rule:

`CapabilityRef` should support stable indirection, not fuzzy capability discovery.

This keeps authoring stable and governance inspectable.

---

## 10. Relationship with Playbook and Workflow Layers

In upper-layer systems, a `CapabilityRef` may appear inside workflow-like or playbook-like authoring.

That is compatible with Torque, as long as the object layers remain distinct.

Important distinction:

- upper layers may say:
  this step or task needs capability X
- Torque should interpret that as:
  resolve `CapabilityRef` to a capability contract, then resolve candidate implementations

So:

- `CapabilityRef` is the upper-layer reference surface
- `CapabilityProfile` is the Torque-side ability contract
- `CapabilityResolution` is the runtime-side candidate evaluation result

This avoids forcing playbook/workflow semantics into the runtime kernel while still making capability references usable from above.

---

## 11. Summary Rule

Recommended summary:

- `CapabilityRef`
  identifies requested ability
- `CapabilityProfile`
  defines the ability contract
- `CapabilityRegistryBinding`
  enumerates viable implementations
- `CapabilityResolution`
  filters and ranks candidates for the current run

In short:

`ref identifies intent, profile defines contract, binding enumerates implementations, resolution filters candidates for the current run`

---

## 12. Open Questions

- Should `CapabilityRef` become a first-class structured type early, or remain authoring-string-first for now?
- How much environment-specific binding should the registry support in its first implementation?
- Should capability deprecation be soft-warning-only at first, or allow automatic redirect to successor profiles?
- How should capability versioning interact with tenant-specific registry configuration?
