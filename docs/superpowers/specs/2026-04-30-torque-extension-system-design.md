# Torque Extension System Design

## Overview

This document defines the Extension System for Torque, enabling third-party extensions to extend Torque's functionality through an event-driven, Actor-based architecture.

**Date**: 2026-04-30  
**Status**: Spec (评审通过)  
**Scope**: Extension architecture, Hook system, communication channels, configuration management

---

## 1. Design Goals

- Make Torque **extensible** without modifying core runtime
- Provide **event-driven interception** for extending Torque's execution flow
- Support **isolated Extension execution** with independent state
- Enable **Extension-to-Extension communication** through well-defined channels
- Keep **Harness independent** - Torque can run without Extension feature

---

## 2. Non-Goals

- Torque Extension System does not provide automatic version upgrades
- Torque Extension System does not enforce resource quotas (users are responsible)
- Torque Extension System does not provide sandbox isolation (Phase 6+)
- Torque Extension System does not support patch-in-place for structural changes

---

## 3. Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    Torque Runtime                             │
│                                                             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐   │
│  │   Agent     │  │    Tool     │  │ Delegation  │   │
│  │   Engine    │  │   Runner    │  │   Manager   │   │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘   │
└─────────┼────────────────┼────────────────┼─────────────┘
          │ emit events     │                │
          ▼                 ▼                ▼
┌─────────────────────────────────────────────────────────────┐
│              Hook Executor (顺序执行, first-block-wins)      │
│                                                             │
│  Handler₁ ──→ Handler₂ ──→ Handler₃ (sequential, no concurrency)
└─────────────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────┐
│              Extension Runtime                               │
│                                                             │
│  ┌─────────────────────┐    ┌─────────────────────┐   │
│  │    Actor Channel     │    │     EventBus        │   │
│  │  (point-to-point)   │    │  (publish/subscribe)│   │
│  └─────────────────────┘    └─────────────────────┘   │
│                                                             │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐                  │
│  │   Ext   │  │   Ext   │  │   Ext   │                  │
│  │    A    │  │    B    │  │    C    │                  │
│  └─────────┘  └─────────┘  └─────────┘                  │
└─────────────────────────────────────────────────────────────┘
```

---

## 4. Crate Structure

### 4.1 Dependency Graph

```
torque-kernel (no dependencies)
       │
       ▼
torque-runtime (depends on kernel)
       │
       ▼
torque-harness ───────────────┐
       │                       │ optional (feature-gated)
       │                       ▼
       │              torque-extension
       │                 (independent crate)
       │
       ▼
  [Can run independently]
```

### 4.2 Feature-Gated Design

```toml
# torque-harness/Cargo.toml
[features]
default = []
extension = ["torque-extension"]
```

---

## 5. Core Types

### 5.1 ID Types

```rust
/// Extension 唯一标识
pub struct ExtensionId(AgentInstanceId);

/// Extension 语义化版本
pub struct ExtensionVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}
```

### 5.2 Error Types

```rust
pub enum ExtensionError {
    NotFound(ExtensionId),
    AlreadyRegistered(ExtensionId),
    Timeout(ExtensionId),
    HookRejected { hook: &'static str, reason: String },
    LifecycleError(String),
    RuntimeError(String),
    SerializationError(String),
    SubscriptionNotFound(String),
    Panicked(String),
}

pub type Result<T> = std::result::Result<T, ExtensionError>;
```

### 5.3 Lifecycle States

```
Loaded ──► Registered ──► Initialized ──► Running
  │            │               │             │
  │            │               │             ▼
  │            │               │         Suspended
  │            │               │             │
  │            │               │             ▼
  │            │               │          Resumed
  │            │               │             │
  │            │               │             ▼
  │            │               │          Stopped
  │            │               │             │
  ▼            ▼               ▼             ▼
Unloaded    Unregistered    Failed       Cleanup
```

| State | Description | Transitions To |
|-------|-------------|---------------|
| Loaded | Loaded but not registered | Registered |
| Registered | Registered to Runtime | Initialized, Unregistered |
| Initialized | Initialized | Running, Failed |
| Running | Running | Suspended, Stopped, Failed |
| Suspended | Paused | Running |
| Stopped | Stopped | Cleanup |
| Failed | Error state | Cleanup |
| Cleanup | Cleanup complete | - |

---

## 6. Extension Model

### 6.1 ExtensionActor Trait

```rust
#[async_trait]
pub trait ExtensionActor: Send + Sync {
    fn id(&self) -> ExtensionId;
    fn name(&self) -> &'static str;
    fn version(&self) -> ExtensionVersion;
    
    async fn on_start(&self, ctx: &ExtensionContext) -> Result<()>;
    async fn on_stop(&self, ctx: &ExtensionContext) -> Result<()>;
    
    async fn handle(
        &self,
        ctx: &ExtensionContext,
        msg: ExtensionMessage,
    ) -> Result<ExtensionResponse>;
}
```

### 6.2 ExtensionContext

```rust
impl ExtensionContext {
    // ========== Hook ==========
    async fn register_hook(&self, hook: &'static str, handler: Arc<dyn HookHandler>) -> Result<()>;
    async fn unregister_hook(&self, hook: &'static str) -> Result<()>;
    
    // ========== Actor Channel (point-to-point) ==========
    async fn send(&self, target: ExtensionId, action: ExtensionAction) -> Result<()>;
    async fn call(&self, target: ExtensionId, action: ExtensionAction) -> Result<ExtensionResponse>;
    
    // ========== EventBus (publish/subscribe) ==========
    async fn publish(&self, topic: BusTopic, payload: serde_json::Value) -> Result<()>;
    async fn subscribe(&self, topic: BusTopic, handler: Arc<dyn BusEventHandler>) -> Result<SubscriptionId>;
    async fn unsubscribe(&self, subscription_id: SubscriptionId) -> Result<()>;
    
    // ========== Configuration ==========
    async fn update_config(&self, patch: ExtensionConfigPatch) -> Result<()>;
    async fn reload(&self, new_extension: Arc<dyn ExtensionActor>) -> Result<ExtensionVersion>;
    fn version(&self) -> ExtensionVersion;
    
    // ========== State ==========
    fn get_state(&self, key: &str) -> Option<serde_json::Value>;
    fn set_state(&self, key: &str, value: serde_json::Value);
}
```

---

## 7. Hook System

### 7.1 Hook Classification

#### Intercept Type (Can Modify, Can Reject)

| Hook | Description |
|------|-------------|
| `tool_call` | Before tool execution, can modify args or reject |
| `tool_result` | After tool execution, can modify result |
| `context` | Before context processing, can modify context |

#### Observational Type (Read-only)

| Hook | Description |
|------|-------------|
| `turn_start` | Turn start, for logging/metrics |
| `turn_end` | Turn end, for logging/metrics |
| `agent_start` | Agent start execution |
| `agent_end` | Agent execution completed |
| `execution_start` | Execution start |
| `execution_end` | Execution completed |
| `error` | Error occurred |
| `checkpoint` | Checkpoint created |

### 7.2 Execution Rules

| Rule | Description |
|------|-------------|
| **Sequential Execution** | Handlers execute in registration order, no concurrency |
| **first-block-wins** | First Rejected stops subsequent Handlers |
| **AbortSignal** | Handlers can check `ctx.signal` to cancel processing |

### 7.3 Hook Handler

```rust
pub struct HookContext {
    pub extension_id: ExtensionId,
    pub hook_name: &'static str,
    pub agent_id: Option<AgentInstanceId>,
    pub signal: AbortSignal,
}

#[async_trait]
pub trait HookHandler: Send + Sync {
    async fn handle(&self, ctx: &HookContext, input: &HookInput) -> HookResult;
}

pub enum HookResult {
    Continue,                      // Continue to next Handler
    Rejected { reason: String }, // Block, subsequent Handlers not executed
    Modified(HookInput),          // Modify, pass to next Handler
    ShortCircuit { value: serde_json::Value },
}
```

### 7.4 Hook Input

```rust
pub enum HookInput {
    ToolCall { tool: ToolDefinition, args: serde_json::Value },
    ToolResult { tool: ToolDefinition, result: ToolResult },
    Context { content: ContextContent },
    TurnStart { turn_number: u32 },
    TurnEnd { turn_number: u32, response: Response },
    AgentStart { request: ExecutionRequest },
    AgentEnd { result: ExecutionResult },
    ExecutionStart { request: ExecutionRequest },
    ExecutionEnd { result: ExecutionResult },
    Error { error: ErrorInfo },
    Checkpoint { checkpoint: CheckpointData },
}
```

### 7.5 Execution Flow

```
HookExecutor::execute(hook_name, input)
    │
    ├── 1. Get Hook definition (Intercept / Observational)
    │
    ├── 2. Get Handler list (registration order)
    │
    ├── 3. Create AbortSignal
    │
    └── 4. Execute Handlers sequentially:

┌─────────────────────────────────────────────────────────────┐
│                    Handler Execution                          │
│                                                             │
│  Handler₁ ──→ Continue ──→ Handler₂ ──→ Rejected ◄── STOP │
│                              │                      │         │
│                              ▼                      ▼         │
│                      Handler₃ not executed    Return Rejected │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## 8. Extension Communication

### 8.1 Dual-Channel Model

| Channel | Mode | Use Case | API |
|---------|------|----------|-----|
| **Actor Channel** | Point-to-point | RPC-style calls requiring response | `send()`, `call()` |
| **EventBus** | Publish/Subscribe | Notifications to multiple Extensions | `publish()`, `subscribe()` |

### 8.2 Actor Channel (Point-to-Point)

```rust
// ========== Message Types ==========

pub enum ExtensionMessage {
    Command { target: ExtensionId, action: ExtensionAction },
    Request { request_id, target, action, timeout_ms },
    Response { request_id, status, result },
}

pub enum ExtensionAction {
    Execute { goal: String, instructions: Vec<String> },
    Query { key: String },
    SetState { key: String, value: serde_json::Value },
    Custom { namespace: String, name: String, payload: serde_json::Value },
}

pub struct ExtensionResponse {
    pub request_id: Uuid,
    pub status: ResponseStatus,
    pub result: Option<serde_json::Value>,
}

pub enum ResponseStatus {
    Success,
    Failure,
    Timeout,
    NotFound,
}

// ========== API ==========

impl ExtensionContext {
    // Fire-and-Forget
    pub async fn send(&self, target: ExtensionId, action: ExtensionAction) -> Result<()>;
    
    // Request-Reply
    pub async fn call(&self, target: ExtensionId, action: ExtensionAction) -> Result<ExtensionResponse>;
}
```

### 8.3 EventBus (Publish/Subscribe)

```rust
// ========== Event Types ==========

pub struct BusEvent {
    pub id: Uuid,
    pub topic: BusTopic,
    pub source: ExtensionId,
    pub timestamp: DateTime<Utc>,
    pub payload: serde_json::Value,
}

#[derive(Clone)]
pub struct BusTopic(String);

impl BusTopic {
    pub const EXT_REGISTERED: BusTopic = BusTopic("ext:registered".into());
    pub const EXT_UNREGISTERED: BusTopic = BusTopic("ext:unregistered".into());
    pub const EXT_ERROR: BusTopic = BusTopic("ext:error".into());
    
    pub fn custom(ns: &str, name: &str) -> Self {
        BusTopic(format!("ext:{}.{}", ns, name).into())
    }
}

#[async_trait]
pub trait BusEventHandler: Send + Sync {
    async fn handle(&self, event: &BusEvent);
}

// ========== API ==========

impl ExtensionContext {
    pub async fn publish(&self, topic: BusTopic, payload: serde_json::Value) -> Result<()>;
    pub async fn subscribe(&self, topic: BusTopic, handler: Arc<dyn BusEventHandler>) -> Result<SubscriptionId>;
    pub async fn unsubscribe(&self, subscription_id: SubscriptionId) -> Result<()>;
}

pub struct SubscriptionId(Uuid);
```

---

## 9. Configuration & Hot Update

### 9.1 Layered Update Strategy

| Layer | Granularity | Content | Update Method |
|-------|-------------|---------|---------------|
| Layer 1 | Coarse | Extension logic, config files | /reload (full replacement) |
| Layer 2 | Fine | Tools, model params, runtime state | API (immediate) |
| Layer 3 | Config source | Settings files | File monitoring → reload |

### 9.2 Configuration Types

```rust
pub struct ExtensionConfig {
    pub settings: serde_json::Value,
    pub tools: HashMap<String, ToolConfig>,
    pub model: Option<ModelConfig>,
}

pub struct ExtensionConfigPatch {
    pub settings: Option<serde_json::Value>,
    pub tools: Option<HashMap<String, ToolConfig>>,
    pub model: Option<ModelConfig>,
}

pub struct ToolConfig {
    pub timeout_ms: Option<u64>,
    pub retries: Option<u32>,
    pub enabled: Option<bool>,
}

pub struct ModelConfig {
    pub provider: String,
    pub model: String,
    pub parameters: serde_json::Value,
}
```

---

## 10. Version Upgrade

**Completely delegated to user. No automatic updates. /reload required for new version to take effect.**

### Upgrade Flow

```
1. User deploys new Extension version
2. User calls POST /v1/extensions/{id}/reload
3. Extension hot-reloads with new version
4. New version takes effect
```

---

## 11. Resource & Security

**No resource limits enforced. Users are responsible for their Extensions.**

```rust
/// Resource usage (informational only, no limits)
pub struct ResourceUsage {
    pub memory_bytes: u64,
    pub cpu_time_ms: u64,
    pub message_count: u64,
}
```

Future extensions (Phase 6+): Process Isolation, Permission Control, Resource Quotas.

---

## 12. REST API

```http
### Extension Management
POST   /v1/extensions              # Register
GET    /v1/extensions              # List
GET    /v1/extensions/{id}         # Details
DELETE /v1/extensions/{id}         # Unregister

### Hook
POST   /v1/extensions/{id}/hooks           # Subscribe Hook
DELETE /v1/extensions/{id}/hooks/{hook}   # Unsubscribe

### Actor Channel
POST   /v1/extensions/{id}/messages       # Send message

### EventBus
POST   /v1/extensions/{id}/events        # Publish event
GET    /v1/extensions/{id}/subscriptions # List subscriptions
POST   /v1/extensions/{id}/subscriptions # Subscribe
DELETE /v1/extensions/{id}/subscriptions/{topic} # Unsubscribe

### Configuration
PATCH  /v1/extensions/{id}/config       # Runtime update (immediate)
POST   /v1/extensions/{id}/reload       # Hot reload (full)
GET    /v1/extensions/{id}/version      # Get version
```

---

## 13. File Structure

```
crates/torque-extension/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── id.rs                    # ExtensionId, ExtensionVersion
    ├── error.rs                 # ExtensionError
    ├── lifecycle.rs             # ExtensionLifecycle
    ├── actor.rs                 # ExtensionActor trait
    ├── context.rs               # ExtensionContext
    ├── message.rs               # ExtensionMessage, ExtensionAction
    ├── config.rs                # ExtensionConfig, ExtensionConfigPatch
    │
    ├── hook/
    │   ├── mod.rs
    │   ├── definition.rs        # HookDef, HookMode, predefined hooks
    │   ├── handler.rs           # HookHandler trait, HookResult
    │   ├── input.rs             # HookInput
    │   ├── context.rs           # HookContext, AbortSignal
    │   ├── registry.rs          # HookRegistry
    │   └── executor.rs           # HookExecutor
    │
    ├── bus/
    │   ├── mod.rs
    │   ├── event.rs             # BusEvent
    │   ├── topic.rs             # BusTopic
    │   ├── handler.rs           # BusEventHandler trait
    │   └── registry.rs          # TopicRegistry
    │
    └── runtime/
        ├── mod.rs
        ├── trait.rs              # ExtensionRuntime trait
        ├── mailbox.rs            # Mailbox
        └── in_memory.rs          # InMemoryExtensionRuntime
```

---

## 14. Implementation Phases

| Phase | Description | Duration | Deliverables |
|-------|-------------|----------|--------------|
| Phase 0 | Technical review | 1 week | This spec |
| Phase 1 | Core abstraction | 2-3 weeks | Trait, types, Hook executor |
| Phase 2 | Runtime | 2-3 weeks | InMemory Runtime |
| Phase 3 | Integration | 2-3 weeks | Hook integration, API |
| Phase 4 | Examples | 1-2 weeks | Logging, Metrics Extensions |
| Phase 5 | Persistence | 1-2 weeks | Snapshot, Recovery |
| Phase 6 | Distributed (optional) | 3-4 weeks | Remote Runtime |

**Total**: 10-17 weeks (excluding Phase 6)

---

## 15. Related Specs

- [Torque Agent Runtime & Harness Design](./2026-04-08-torque-agent-runtime-harness-design.md)
- [Torque Kernel Execution Contract Design](./2026-04-08-torque-kernel-execution-contract-design.md)
