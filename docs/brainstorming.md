# Brainstorming Notes

## Multi-Agent Productivity

Current conclusion: Torque should default to a **supervisor/subagent** collaboration model. Fully peer-to-peer multi-agent coordination should exist only as an advanced, explicitly controlled mechanism.

This is not because peer collaboration is impossible. It is because the default should optimize for:

- task convergence
- quality stability
- controllable cost
- observability
- governance
- sustainable iteration

One useful mental model is:

`effective output = specialization gain - coordination cost - rework cost - loss-of-control cost`

From that perspective, supervisor-first orchestration is usually the stronger default.

## Why Supervisor/Subagent Should Be the Default

### 1. Faster Convergence

When a supervisor holds the task spine, boundaries are clearer:

- who is responsible for what
- when work is handed off
- when results must return
- when a branch of work should stop

This reduces drift and duplicated effort.

### 2. Better Context Control

One of the main benefits of multi-agent systems is context engineering. Isolating prompts, tools, memory, and working state per agent reduces noise and improves specialization.

Supervisor/subagent structures encourage:

- narrower context windows
- explicit input contracts
- controlled output contracts
- less accidental state sharing

### 3. Better Observability and Recovery

A controlled delegation model is easier to:

- trace
- audit
- replay
- checkpoint
- resume
- route through HITL gates

This aligns with Torque's goal as a runtime/harness platform rather than an open-ended agent society simulator.

### 4. Healthier Cost Structure

Peer-to-peer systems tend to increase communication rounds. Supervisor/subagent systems usually compress collaboration into a narrower interaction surface:

1. supervisor delegates
2. specialist executes
3. result returns
4. supervisor synthesizes or reviews

This is easier to budget and reason about.

## Where Peer Collaboration Still Helps

Peer-style or handoff-heavy collaboration still has value in some cases:

- more autonomous open-ended exploration
- environments where control should shift between specialists
- network-like collaboration models with less centralized planning

But in Torque, those should be explicit exceptions, not the default.

## Implication for Torque

These notes support the current architecture direction:

- Kernel remains agent-centric
- Harness exposes team orchestration
- default team collaboration is `Supervisor -> Subagent`
- peer handoff is explicit, auditable, and constrained
