# Torque Recovery Core Design

## Overview

This document defines the current design direction for the **event, checkpoint, and recovery core model** in Torque.

Torque should treat recovery as a layered system built from:

- `Event`
- `Checkpoint`
- recovery-time reconciliation

These are related, but they are not interchangeable.

**Date**: 2026-04-08  
**Status**: Draft  
**Scope**: Event truth model, checkpoint scope, replay, reconciliation, recovery flow

---

## 1. Design Goals

- Make event history the primary truth source
- Use checkpoints only as recovery acceleration, not as competing truth
- Define recovery as restore + replay + reconcile
- Keep checkpoint scope focused on efficient recovery state
- Preserve correctness when runtime reality diverges from stored snapshots

## 2. Non-Goals

- Torque does not treat checkpoints as the authoritative historical record
- Torque does not assume snapshot restore is sufficient without replay
- Torque does not assume replay alone is always the cheapest recovery path
- Torque does not require exact in-place restoration of every internal detail before progress can continue
- Torque does not collapse recovery into a single monolithic snapshot model

---

## 3. Core Relationship

The recommended recovery relationship is:

- `Event`
  is the truth source
- `Checkpoint`
  is the recovery acceleration layer
- `Recovery`
  is the process that restores snapshot state, replays tail events, and reconciles against current runtime reality

Recommended summary:

`Event = truth`
`Checkpoint = acceleration`
`Recovery = restoration + replay + reconciliation`

---

## 4. Event

### 4.1 Purpose

`Event` records what actually happened in the system.

Examples include:

- instance created
- task assigned
- model response received
- tool call started or completed
- artifact created
- artifact published
- memory candidate created
- delegation requested
- approval requested
- checkpoint created
- state transitioned

### 4.2 Truth Role

Event history should be treated as the factual source of truth.

If there is tension between:

- event history
- checkpoint contents
- derived summaries

the event record is the source that explains what actually happened over time.

### 4.3 Why Event Truth Matters

Without event truth:

- recovery cannot be trusted
- replay becomes impossible or ambiguous
- audit trails weaken
- time-travel lineage becomes unclear

---

## 5. Checkpoint

### 5.1 Purpose

`Checkpoint` is a recovery acceleration snapshot.

It exists to make recovery cheaper and faster, not to become a second historical truth system.

### 5.2 Snapshot Scope

Recommended checkpoint contents should focus on the minimum useful running state needed for efficient recovery, such as:

- instance or team status
- current task references
- pending approvals
- active delegation references
- context anchors
- relevant shared-state anchors
- event anchor or offset

### 5.3 What Checkpoint Should Not Be

Checkpoint should not try to become:

- a full event replacement
- a complete long-term archive
- a second copy of all raw execution data
- a universal dump of every internal detail

### 5.4 Design Rule

Checkpoint should snapshot what is useful to restart efficiently, not everything that ever mattered historically.

---

## 6. Recovery

### 6.1 Purpose

`Recovery` is the process of bringing the system back to a usable, policy-governed, fact-consistent state.

It is not a static object.

### 6.2 Recommended Flow

Recommended default recovery flow:

1. load latest checkpoint
2. restore checkpoint state
3. replay tail events after the checkpoint anchor
4. inspect current runtime and storage reality
5. reconcile restored state against actual reality
6. continue, replace, fail, or escalate according to policy

### 6.3 Why Replay Is Required

Checkpoint restore alone is not enough, because:

- events may have occurred after the snapshot
- artifact creation may have succeeded after checkpoint
- state transitions may not yet be reflected in the snapshot
- pending approvals or delegations may have changed

Replay is how recovery re-approaches truth after using a cheaper starting point.

---

## 7. Reconciliation

### 7.1 Purpose

Reconciliation is the step that compares restored state with current runtime/storage reality and resolves inconsistencies.

This is a first-class recovery step, not an optional cleanup task.

### 7.2 Typical Inconsistencies

Examples include:

- checkpoint says a child instance is still active, but the instance has failed or disappeared
- event history implies an artifact was created, but status did not update cleanly
- approval request is still pending, but parent state has already advanced or failed
- delegation state and child reality are no longer aligned

### 7.3 Recovery Without Reconciliation Is Unsafe

Blindly restoring from checkpoint can be wrong because the world may have changed since the snapshot.

Without reconciliation:

- stale state may be treated as current truth
- completed work may be repeated
- failed work may look resumable
- approval state may become inconsistent

### 7.4 Recovery Outcomes

After reconciliation, recovery should be able to select from policy-governed outcomes such as:

- resume current execution
- accept already completed output
- replace missing or failed child execution
- reissue a task or delegation
- escalate to approval or operator
- fail or cancel when no valid continuation remains

---

## 8. Event, Checkpoint, and Recovery Together

These three should be understood as complementary, not competing:

- `Event`
  tells the history
- `Checkpoint`
  shortens the path back into execution
- `Recovery`
  turns stored history plus current reality back into a coherent running state

One useful mental model is:

- without `Event`, you lose truth
- without `Checkpoint`, recovery becomes expensive
- without `Recovery` reconciliation, restored state may be wrong

---

## 9. Design Rules

Recommended hard rules:

- event log is the historical truth source
- checkpoint is a recovery acceleration layer
- recovery must include replay after checkpoint
- recovery must include reconciliation against current runtime or storage reality
- snapshot restore alone is not sufficient for correctness

---

## 10. Relationship to Higher Layers

This core recovery model should apply consistently across:

- kernel instance recovery
- team-level recovery
- context/state recovery
- approval and delegation recovery

Higher layers may add their own objects and policies, but they should not violate the same underlying recovery philosophy.

---

## 11. Open Questions

- Which event classes must always be persisted synchronously before acknowledging completion-sensitive actions?
- How often should checkpoints be created relative to event volume and recovery cost?
- Which recovered inconsistencies should be auto-healed versus escalated?
- How should retention policies differ for event logs and checkpoints?
