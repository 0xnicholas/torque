# Torque Runtime-Neutral Interfaces Design

> Date: 2026-04-26  
> Status: Draft  
> Scope: runtime-neutral ports for host extraction from `torque-harness` into `torque-runtime`  
> Goal: Define the minimum interface set needed to move the runtime host out of harness-specific types without reintroducing a second execution model.

---

## 1. Problem

The current `runtime::host::KernelRuntimeHandle` in `torque-harness` still depends directly on harness-owned types:

- `StreamEvent`
- `ToolService`
- `SessionRepository`
- concrete `Checkpointer`

This prevents the host from moving cleanly into `torque-runtime`, because the runtime layer would still need to depend on harness transport, repository, and service modules.

The issue is no longer file location. The issue is interface shape.

---

## 2. Design Goals

- Let the runtime host live in `torque-runtime`
- Keep `torque-kernel` unchanged as the execution-contract source
- Prevent `torque-runtime` from depending on harness API or repository modules
- Keep transport-specific event types such as `StreamEvent` out of runtime
- Keep product-specific services such as `ToolService` and `SessionService` out of runtime
- Minimize the number of new ports so the host does not become an abstract factory maze

## 3. Non-Goals

- This does not redesign the kernel object model
- This does not move session/run/team services into `torque-runtime`
- This does not define the final production persistence model for runtime state
- This does not replace all harness policies with runtime-owned policies

---

## 4. Options

### Option A: Keep the current host in harness and only move helpers

Pros:
- lowest short-term change cost
- keeps tests stable

Cons:
- leaves the main architectural ambiguity unresolved
- `torque-runtime` becomes a helper crate instead of the real runtime layer

### Option B: Move the host into `torque-runtime` now and pass harness types through traits

Pros:
- fastest path to a host in the new crate

Cons:
- easy to leak `StreamEvent`, `ToolService`, or repository concerns through trait signatures
- would encode harness concepts into the runtime layer

### Option C: Define a small runtime-neutral port set first, then move the host

Pros:
- preserves the intended dependency direction
- gives `torque-runtime` a clean contract surface
- keeps harness-specific types outside runtime

Cons:
- one extra design step before the host move

### Recommendation

Choose Option C.

The host should move only after the runtime layer speaks in environment contracts rather than harness implementation types.

---

## 5. Port Model

The runtime host should depend on five interfaces and one optional helper.

### 5.1 `RuntimeModelDriver`

Purpose:
- run a model turn
- return assistant content plus tool-call intents

Required because the host currently depends on `LlmClient` plus a harness-local streaming callback shape.

Suggested contract:

```rust
#[async_trait::async_trait]
pub trait RuntimeModelDriver: Send + Sync {
    async fn run_turn(
        &self,
        messages: Vec<RuntimeMessage>,
        tools: Vec<RuntimeToolDef>,
        sink: Option<&dyn RuntimeOutputSink>,
    ) -> anyhow::Result<ModelTurnResult>;
}
```

### 5.2 `RuntimeToolExecutor`

Purpose:
- execute a named tool with runtime execution context
- return a runtime-owned result shape

Suggested contract:

```rust
#[async_trait::async_trait]
pub trait RuntimeToolExecutor: Send + Sync {
    async fn execute(
        &self,
        ctx: RuntimeExecutionContext,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> anyhow::Result<RuntimeToolResult>;

    async fn tool_defs(&self) -> anyhow::Result<Vec<RuntimeToolDef>>;
}
```

This deliberately hides `ToolService` and the harness tool registry.

### 5.3 `RuntimeEventSink`

Purpose:
- persist execution-result events
- persist checkpoint-created events

Suggested contract:

```rust
#[async_trait::async_trait]
pub trait RuntimeEventSink: Send + Sync {
    async fn record_execution_result(
        &self,
        result: &torque_kernel::ExecutionResult,
    ) -> anyhow::Result<()>;

    async fn record_checkpoint_created(
        &self,
        checkpoint_id: uuid::Uuid,
        instance_id: torque_kernel::AgentInstanceId,
        reason: &str,
    ) -> anyhow::Result<()>;
}
```

This keeps runtime independent of harness DB event models.

### 5.4 `RuntimeCheckpointSink`

Purpose:
- materialize a checkpoint from kernel-owned state
- return a runtime-level checkpoint reference

Suggested contract:

```rust
#[async_trait::async_trait]
pub trait RuntimeCheckpointSink: Send + Sync {
    async fn save(
        &self,
        checkpoint: RuntimeCheckpointPayload,
    ) -> anyhow::Result<RuntimeCheckpointRef>;
}
```

The host should build `RuntimeCheckpointPayload` using kernel state and no longer depend on harness `Checkpointer` directly.

### 5.5 `RuntimeHydrationSource`

Purpose:
- recover enough state to rehydrate an in-memory reference runtime for session/run continuation

Suggested contract:

```rust
#[async_trait::async_trait]
pub trait RuntimeHydrationSource: Send + Sync {
    async fn load_instance_state(
        &self,
        instance_id: torque_kernel::AgentInstanceId,
    ) -> anyhow::Result<Option<HydrationState>>;
}
```

This hides `SessionRepository` from runtime.

### 5.6 `RuntimeOutputSink`

Purpose:
- emit transport-agnostic incremental output

Suggested contract:

```rust
pub trait RuntimeOutputSink: Send + Sync {
    fn on_text_chunk(&self, chunk: &str);
    fn on_tool_call(&self, tool_name: &str, arguments: &serde_json::Value);
    fn on_tool_result(&self, tool_name: &str, result: &RuntimeToolResult);
    fn on_checkpoint(&self, checkpoint_id: uuid::Uuid, reason: &str);
}
```

This replaces direct dependence on `StreamEvent`.

Important rule:
- runtime emits semantic output events
- harness translates them into SSE/websocket/API-specific transport events

---

## 6. Shared Runtime Types

These types belong in `torque-runtime`, not harness.

### 6.1 `RuntimeExecutionContext`

```rust
pub struct RuntimeExecutionContext {
    pub instance_id: uuid::Uuid,
    pub request_id: Option<uuid::Uuid>,
    pub source_task_id: Option<uuid::Uuid>,
}
```

### 6.2 `RuntimeToolResult`

The existing `torque-runtime::tools::ToolExecutionResult` should be expanded rather than creating another type.

Suggested shape:

```rust
pub struct RuntimeToolResult {
    pub success: bool,
    pub content: String,
    pub error: Option<String>,
    pub offload_ref: Option<RuntimeArtifactRef>,
}
```

### 6.3 `RuntimeMessage`

Runtime should not depend on harness `infra::llm::LlmMessage`.
It should carry its own normalized message type and let adapters translate at the edge.

### 6.4 `ModelTurnResult`

```rust
pub struct ModelTurnResult {
    pub finish_reason: RuntimeFinishReason,
    pub assistant_text: String,
    pub tool_calls: Vec<RuntimeToolCall>,
}
```

---

## 7. Ownership Rules

### 7.1 What stays in harness

- `StreamEvent`
- SSE/webhook transport logic
- repository implementations
- `ToolService`
- session/run/team orchestration
- policy enforcement that is product-facing

### 7.2 What moves to runtime

- host control flow
- model/tool/event/checkpoint/hydration port definitions
- runtime-owned message/tool result/checkpoint payload types
- transport-agnostic output sink contract

### 7.3 What stays in kernel

- `ExecutionRequest`
- `ExecutionResult`
- `Task`
- `AgentInstance`
- `RuntimeCommand`
- `ResumeSignal`
- `RuntimeStore`

No runtime-neutral interface in this document should compete with those kernel objects.

---

## 8. Migration Plan

### Phase 1: Introduce neutral types and adapters

In `torque-runtime`:
- add `RuntimeMessage`
- add `ModelTurnResult`
- add `RuntimeToolCall`
- add `RuntimeCheckpointPayload`
- add `RuntimeOutputSink`

In `torque-harness`:
- implement adapters from `StreamEvent` to `RuntimeOutputSink`
- implement adapters from `ToolService` to `RuntimeToolExecutor`
- implement adapters from repository/checkpointer services to `RuntimeEventSink`, `RuntimeCheckpointSink`, and `RuntimeHydrationSource`

### Phase 2: Move host into `torque-runtime`

- move `runtime::host::KernelRuntimeHandle` into `torque-runtime`
- change the host constructor so it accepts only runtime-neutral ports
- leave a harness re-export shim temporarily

### Phase 3: Remove harness-owned host implementation

- keep only the re-export in harness
- move remaining host-only helpers into `torque-runtime`

---

## 9. Acceptance Criteria

This design is successful when:

1. The runtime host constructor no longer mentions `ToolService`, `SessionRepository`, `StreamEvent`, or harness-local checkpointer wrappers.
2. `torque-runtime` can define and compile the host using only kernel contracts plus runtime-neutral ports.
3. Harness transports can still emit the same user-visible streaming behavior through an adapter.
4. Existing session/run tests keep passing after the adapters are introduced.

---

## 10. Immediate Next Step

The next implementation slice should not keep moving files.

It should:
- add the runtime-neutral types and traits in `torque-runtime`
- add harness adapters for those traits
- prove the host can be re-constructed from those adapters before moving the host itself

That is the narrowest path that reduces coupling instead of relocating it.
