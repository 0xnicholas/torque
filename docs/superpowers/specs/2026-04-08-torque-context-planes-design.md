# Torque Context Planes Design

## Overview

This document defines the current design direction for the three major context-related planes in Torque:

- `ExternalContextRef`
- `Artifact`
- `Memory`

Torque should keep these planes explicitly separate.

They may reference one another and participate in the same execution lifecycle, but they should not collapse into one general-purpose knowledge bucket.

**Date**: 2026-04-08  
**Status**: Draft  
**Scope**: External context plane, artifact plane, memory plane, plane transitions, ownership boundaries

---

## 1. Design Goals

- Keep external references, execution outputs, and long-term semantic retention distinct
- Prevent one context plane from becoming a catch-all storage layer
- Make transitions between planes explicit and auditable
- Preserve lazy loading and token efficiency
- Support replay, recovery, and governance through clear plane ownership

## 2. Non-Goals

- Torque does not treat all context as one unified knowledge store
- Torque does not automatically persist all execution outputs as memory
- Torque does not copy all external context into internal state by default
- Torque does not use memory as an artifact archive
- Torque does not use artifacts as a replacement for external context references

---

## 3. Core Model

Torque should explicitly maintain three distinct planes:

### 3.1 External Context Plane

Represented primarily by `ExternalContextRef`.

This plane answers:

"What externally owned context can execution read from?"

### 3.2 Artifact Plane

Represented by `Artifact`.

This plane answers:

"What did this execution produce?"

### 3.3 Memory Plane

Represented by `Memory` and fed by `MemoryWriteCandidate`.

This plane answers:

"What durable semantic information should Torque retain for future recall?"

These planes are related, but they are not interchangeable.

---

## 4. ExternalContextRef

### 4.1 Purpose

`ExternalContextRef` is the reference object for externally owned context.

Examples include:

- repositories
- documents
- tickets
- logs
- file spaces
- knowledge bases
- conversation threads

### 4.2 Default Semantics

Recommended defaults:

- read-only by default
- stored as reference, not eagerly materialized content
- retrieved on demand
- not treated as an internal Torque-owned domain object

### 4.3 What It Is Not

`ExternalContextRef` is not:

- a memory record
- a published artifact
- a hot prompt blob
- an implicit copy of an external system into Torque state

Its job is to point at external context, not to absorb that context into all active execution by default.

---

## 5. Artifact

### 5.1 Purpose

`Artifact` should be understood first as an execution result object.

It is not only a storage blob.

Artifacts are important because they serve as:

- execution outputs
- traceable result records
- downstream input surfaces
- replay and review anchors

### 5.2 Typical Artifact Examples

Artifacts may include:

- structured result documents
- generated files
- plans
- review reports
- summaries
- intermediate products
- final deliverables

### 5.3 Why Artifact Is a First-Class Execution Object

Treating artifact as a first-class execution object helps preserve:

- auditable output history
- downstream reuse
- explicit publish behavior
- separation from memory retention

### 5.4 What Artifact Is Not

Artifact is not:

- the memory plane
- the shared-state plane
- the external reference plane

It is the result plane.

---

## 6. Memory

### 6.1 Purpose

`Memory` should be modeled as a semantic retention plane.

It is for durable recall-worthy knowledge, not for full output archiving.

### 6.2 Recommended Contents

Memory should favor:

- durable facts
- stable decisions worth recalling
- reusable lessons
- persistent semantic summaries

### 6.3 What Memory Should Not Become

Memory should not become:

- a full artifact archive
- a raw transcript store
- a dump of every result ever produced
- a replacement for external documents

### 6.4 Entry Path

All long-term writes should first become `MemoryWriteCandidate`.

Memory retention should therefore be policy-governed and selective.

---

## 7. Plane Transitions

Transitions between planes should be explicit and observable.

Recommended transition model:

### 7.1 External Context Read

`ExternalContextRef`
-> retrieved into local execution context

This is a read into execution, not a plane conversion.

### 7.2 Execution Output Creation

execution
-> `Artifact`

This is the normal path for result production.

### 7.3 Team Acceptance and Publish

`Artifact`
-> accepted and published into `SharedTaskState`

This is a governance action that promotes an execution result into team-shared coordination state.

It is not the same thing as memory retention.

### 7.4 Memory Nomination

`Artifact` or accepted content
-> `MemoryWriteCandidate`

This is a nomination step, not yet a memory write.

### 7.5 Memory Write

`MemoryWriteCandidate`
-> `Memory`

This requires separate policy-governed retention approval or evaluation.

---

## 8. Explicit Non-Automation Rules

Torque should keep the following rules explicit:

- external context does not automatically become artifact
- artifact does not automatically become memory
- team publish does not automatically become memory write
- memory does not replace artifact retention
- artifact does not replace external reference access

These rules are important because they prevent uncontrolled context growth and plane confusion.

---

## 9. Relationship with SharedTaskState

`SharedTaskState` is not one of the three major storage planes.

It is a governance-filtered coordination surface that may hold:

- accepted fact entries
- accepted artifact refs
- summaries
- blockers
- approvals

It should not absorb the responsibilities of:

- external references
- full execution artifacts
- long-term memory retention

So the correct relationship is:

- `ExternalContextRef`
  external input reference
- `Artifact`
  execution output object
- `SharedTaskState`
  accepted coordination layer
- `Memory`
  durable semantic retention layer

---

## 10. Why Separation Matters

Keeping the planes separate helps Torque preserve:

- token efficiency
- lazy loading
- clearer recovery boundaries
- clearer ownership
- better governance
- easier replay and audit

If the planes collapse together, several pathologies appear:

- everything gets stuffed into active context
- memory becomes archive
- artifacts become generic blobs
- external references lose meaning
- approval and publish semantics become ambiguous

---

## 11. Summary Rules

Recommended summary:

- `ExternalContextRef`
  is the external reference plane
- `Artifact`
  is the execution result plane
- `Memory`
  is the semantic retention plane

Recommended hard rule:

`transitions between planes must be explicit and policy-governed`

---

## 12. Open Questions

- Which artifact classes should be considered memory-eligible by default, if any?
- How much metadata should be attached to external context reads for replay and audit?
- Should published artifact refs always point to immutable artifact versions?
- How should retention and garbage-collection differ across the three planes?
