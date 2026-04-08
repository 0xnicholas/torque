# Session Agent MVP Plan

## Status

This document is an exploratory MVP plan for the existing `session-agent` crate.

It is **not** the authoritative architecture for Torque. The authoritative architecture is the runtime/harness design in [`../specs/2026-04-08-torque-agent-runtime-harness-design.md`](../specs/2026-04-08-torque-agent-runtime-harness-design.md).

The purpose of this MVP is to validate a session-oriented product surface on top of the current codebase without redefining Torque's kernel model around `Session`.

## Goal

Build a lightweight session-oriented agent surface that can validate:

- HTTP entrypoints
- streaming output
- tool-calling loops
- session persistence
- a simple bridge between user-facing sessions and Torque's longer-term runtime direction

## Architectural Position

`session-agent` should be treated as an **adapter-style MVP**, not as the long-term core abstraction.

Meaning:

- `Session` is a product/API surface
- `AgentInstance` remains the long-term execution center in Torque
- this crate is allowed to be simpler than the full runtime model
- the crate should not reintroduce old DAG/planner assumptions

## Scope

The MVP should focus on a narrow slice:

- one session receives user messages
- a runner maintains session context
- the runner can call tools through the LLM loop
- responses can be streamed back to the client
- session state is persisted enough to resume conversational continuity

## Non-Goals

This MVP should not try to solve the full Torque architecture.

Specifically out of scope:

- full team orchestration
- supervisor/subagent delegation runtime
- full event-sourced recovery
- generalized approval flow
- long-term memory pipeline
- full artifact publish/shared task state semantics

## Current Crate Shape

The current crate structure is already much smaller than the original scaffold plan, which is good for an MVP:

```text
crates/session-agent/
├── src/
│   ├── agent.rs
│   ├── api.rs
│   ├── db.rs
│   ├── lib.rs
│   ├── tools.rs
│   └── models/
│       ├── mod.rs
│       ├── message.rs
│       └── session.rs
```

The MVP plan should stay aligned with this simpler structure unless there is a strong reason to split further.

## Recommended MVP Responsibilities

### 1. API Surface

Expose a minimal HTTP interface for:

- creating sessions
- posting user messages
- streaming assistant output
- reading prior session state

### 2. Session State

Persist only what the MVP truly needs:

- session metadata
- ordered message history
- tool call metadata
- lightweight status transitions

Avoid over-modeling session state before the runtime kernel model is implemented underneath.

### 3. Runner Loop

Implement a constrained loop that can:

- build prompt context from session history
- call the LLM
- execute allowed tools
- append tool results
- stream response chunks

This loop should stay simple and predictable.

### 4. Future Compatibility

The MVP should avoid painting Torque into a corner.

So the crate should be written in a way that can later map:

- `Session` -> one or more `ExecutionRequest`s
- session runner state -> `AgentInstance` working state
- message/tool outputs -> future artifact/event structures

## Concrete Follow-Up Work

### Phase 1

- stabilize the current `session-agent` data model
- confirm the HTTP surface
- confirm the streaming model
- verify the tool loop against the current `llm` crate

### Phase 2

- separate product-facing session state from internal runner state
- introduce clearer tool execution boundaries
- add structured logging and error handling around the runner

### Phase 3

- define the migration path from session-local concepts to kernel runtime concepts
- decide which parts of session persistence later become artifacts, events, or checkpoints

## Implementation Constraints

- keep the MVP small
- avoid introducing product-wide architecture claims from this crate
- do not let `Session` replace `AgentInstance` as the kernel's conceptual center
- do not assume session persistence alone is equivalent to durable runtime recovery

## Success Criteria

- a client can create a session and continue it over multiple turns
- responses can stream incrementally
- tool calling works in a controlled loop
- session history persists correctly
- the crate remains conceptually compatible with Torque's longer-term runtime/harness architecture
