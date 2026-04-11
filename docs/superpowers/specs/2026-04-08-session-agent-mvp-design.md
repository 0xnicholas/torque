# Session Agent MVP Design

## Purpose

This document defines the earliest product-facing MVP for Torque.

The MVP is intentionally narrow:

- it demonstrates a usable **single-agent session experience**
- it proves Torque can support **stateful conversations**, not just one-shot prompting
- it avoids prematurely exposing the full kernel / harness / team architecture

This is a **demo-oriented product slice**, not a reduced version of the full multi-agent platform.

---

## Goals

The MVP should demonstrate that Torque can already behave like a reliable session-based agent.

The user-facing goals are:

- create a session
- chat with the same agent across multiple turns
- receive streaming responses
- preserve conversation history across reloads
- optionally trigger a small number of safe, demo-friendly tools

The architecture goals are:

- reuse the current `agent-runtime-service` path instead of building a second throwaway demo stack
- keep implementation boundaries compatible with later kernel evolution
- avoid introducing concepts that imply team orchestration, capability resolution, or policy UI before they are needed

This MVP is not trying to prove:

- multi-agent execution
- supervisor/subagent orchestration
- approval workflows
- recovery UI
- capability registry UX
- full policy-system exposure

---

## Product Shape

The MVP is a small product-facing session agent service.

Its visible shape is:

- a user can create a session
- a user can send messages to that session
- the assistant replies over SSE
- the full message history for the session can be fetched later

The product surface should feel like:

- a persistent agent conversation
- with lightweight runtime credibility
- without exposing the full platform control plane

The first impression should be:

`Torque already runs a stateful agent`

not:

`Torque has a speculative architecture that is not yet demonstrable`

---

## In Scope

The MVP includes the following capabilities.

### 1. Session Lifecycle

- create session
- get session metadata
- list sessions
- persist session records in PostgreSQL

### 2. Message Lifecycle

- persist user messages
- persist assistant messages
- fetch message history for a session
- continue an existing session across multiple turns

### 3. Single-Agent Runtime Loop

- one fixed agent persona / system prompt
- LLM call per turn
- optional bounded tool loop
- final assistant message persisted after completion

### 4. Streaming

- `POST /sessions/{id}/chat` responds as SSE
- streaming is first-class in the experience, not an afterthought

### 5. Basic Context Management

- recent-window context inclusion
- simple truncation and/or summary compaction
- no unbounded prompt growth

This is explicitly a simplified precursor to the broader context-state model.

### 6. Minimal Tooling

- a very small whitelist of safe, stable, demo-friendly tools
- tool execution visible as lightweight streaming events
- tool failure handled gracefully

Tool support may be shipped as:

- one safe tool
- or temporarily zero tools with a preserved interface seam

The exact tool count is secondary to demo stability.

---

## Out of Scope

The MVP does not include:

- multi-agent or team orchestration
- supervisor / subagent execution
- approval requests or approval UI
- checkpoint browsing UI
- recovery controls
- capability registry UX
- selector resolution
- explicit policy engine UI
- memory write pipeline
- artifact browsing as a user-facing feature
- multi-tenant quotas and scheduling control plane
- admin configuration surface beyond what is minimally required to run locally

These may be represented as future-facing seams in code structure, but they are not part of the deliverable product slice.

---

## Primary User Flows

### 1. Start a New Conversation

1. user creates a session
2. service returns `session_id`
3. user sends first message to `POST /sessions/{id}/chat`
4. assistant streams its response over SSE
5. user and assistant messages are persisted

This is the most important flow in the MVP.

### 2. Continue an Existing Conversation

1. user opens an existing session
2. client loads prior messages from `GET /sessions/{id}/messages`
3. user sends another message
4. assistant responds using prior conversation context
5. new messages are appended to the same session history

This flow proves the product is stateful rather than stateless.

### 3. Tool-Assisted Turn

1. agent determines a supported tool is needed
2. streaming emits a lightweight tool event
3. tool executes
4. tool result is folded into the current turn
5. assistant completes the streamed reply
6. final assistant message is stored like any other turn

This flow is valuable but secondary to the first two.

---

## API Surface

The MVP API should remain intentionally small.

### Required Endpoints

- `POST /sessions`
  create a session

- `GET /sessions`
  list sessions

- `GET /sessions/{id}`
  fetch session metadata

- `GET /sessions/{id}/messages`
  fetch session message history

- `POST /sessions/{id}/chat`
  submit a user message and receive SSE

### Chat Request

The initial request body should remain minimal:

```json
{
  "message": "Hello, agent"
}
```

No per-request capability selection, policy override, task metadata, or execution hints should be exposed in the MVP API.

### Streaming Events

The streaming protocol should support at least:

- `start`
- `chunk`
- `tool_call`
- `done`
- `error`

The exact payload shape may evolve, but the event model should stay simple and product-facing.

---

## Internal Architecture

The MVP should reuse the current `crates/agent-runtime-service` direction and tighten it into a clean demo path.

Recommended internal layers:

### 1. HTTP API Layer

Responsible for:

- session endpoints
- message history endpoints
- chat SSE endpoint

### 2. Session Store Layer

Responsible for:

- session persistence
- message persistence
- history reads

PostgreSQL is sufficient for the MVP.

### 3. Agent Runner Layer

Responsible for:

- receiving the current user turn
- loading recent message context
- applying basic context truncation / compaction
- invoking the LLM
- coordinating tool calls if enabled
- emitting streaming events
- returning a final assistant message

This layer should be kept conceptually close to later runtime evolution, even if the concrete types remain MVP-specific.

### 4. Tool Layer

Responsible for:

- small whitelist of safe tools
- deterministic tool registration
- graceful tool errors

The tool layer should not evolve into a general plugin platform in this MVP.

### 5. LLM Client Layer

Responsible for:

- OpenAI-compatible requests
- streaming token handling
- tool-call parsing support

This should reuse `crates/llm` rather than duplicating client logic.

---

## Alignment With Future Architecture

Although this MVP is intentionally product-first, it should not actively fight the future kernel/harness direction.

Implementation should preserve these future-friendly seams:

- clear separation between session persistence and agent execution
- clear “current turn input” boundary
- explicit streaming event model
- bounded tool loop
- room for future checkpoint integration
- room for future `TaskPacket`-style context shaping

What the MVP should not do:

- encode full future architecture into public APIs
- introduce fake team abstractions
- expose raw internal object models before they are stable

The rule is:

`keep the internal seams clean without making the MVP feel like an architecture demo`

---

## Context Strategy For MVP

The MVP should use a simplified context strategy.

Recommended behavior:

- include recent conversation turns directly
- cap total context size
- if needed, summarize older turns into a short session summary
- do not attempt the full future layered context system yet

The important product property is:

- the session feels continuous

The important technical property is:

- context size stays bounded

---

## Error Handling

The MVP should optimize for graceful degradation.

### LLM Errors

- emit a terminal `error` stream event
- do not corrupt the session
- preserve already-stored user messages

### Tool Errors

- keep the current turn recoverable where possible
- either continue with a fallback assistant response or fail the turn cleanly
- do not expose stack-trace-style internals to the end user

### Persistence Errors

- fail the request clearly
- avoid partial assistant-message persistence when a turn has not completed successfully

Demo reliability matters more than squeezing in every feature.

---

## Non-Functional Priorities

For this MVP, priorities should be:

1. reliability of the primary demo flow
2. simplicity of the API
3. streaming responsiveness
4. persistence correctness
5. limited but credible context continuity

Not priorities:

- perfect long-horizon memory
- platform completeness
- generalized orchestration
- full operational tooling

---

## Success Criteria

The MVP is successful if all of the following are true:

1. a new session can be created successfully
2. a user can chat with the same session across multiple turns
3. assistant replies stream over SSE
4. reloading a session still shows prior messages and supports continued conversation
5. the agent feels like a stateful assistant rather than a one-shot wrapper

Optional but strong enhancement:

6. at least one tool-assisted turn can be demonstrated reliably

The most important demo test is qualitative:

- a first-time viewer should understand within a few minutes that Torque already supports a persistent agent experience

---

## Suggested Implementation Boundary

This spec is intended to drive an MVP implementation that primarily evolves:

- `crates/agent-runtime-service`
- `crates/llm`

with minimal new surface area unless needed for clarity.

If additional crates are introduced, they should serve obvious separations such as:

- persistence
- runtime loop
- tool execution

not speculative platform decomposition.

---

## Summary

The Session Agent MVP is a narrow, product-facing slice:

- one session
- one agent
- multi-turn persistence
- streaming replies
- basic context continuity
- optional minimal tools

It should be good enough to demo confidently, while still leaving the codebase able to evolve toward the broader Torque runtime and harness architecture.
