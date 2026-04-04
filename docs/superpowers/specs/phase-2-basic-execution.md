# Phase 2: Basic Execution

## Overview

**Goal**: Single node can execute from start to finish, with LLM calls and tool execution.

**Prerequisites**: Phase 1 (types, dag, db, queue)

---

## Success Criteria

- [ ] ContextStore can route reads/writes to Redis or S3 based on size
- [ ] Tool Executor can execute registered tools with permission checks
- [ ] Agent Runtime can complete a simple node execution
- [ ] Executor can drive a single-layer DAG to completion

---

## Components

### 1. context-store crate

**Purpose**: Unified read/write interface, transparent routing to Redis or S3.

**Routing Rules**:
| Condition | Storage |
|-----------|---------|
| Size < 256KB, JSON | Redis direct |
| Size 256KB ~ 10MB | S3 + Redis Pointer |
| Size > 10MB | S3 direct |
| Binary files | S3 direct |

**Trait**:
```rust
#[async_trait]
pub trait ContextStore: Send + Sync {
    async fn write(&self, data: &[u8]) -> Result<ArtifactPointer>;
    async fn read(&self, pointer: &ArtifactPointer) -> Result<Vec<u8>>;
    async fn delete(&self, pointer: &ArtifactPointer) -> Result<()>;
}
```

**ArtifactPointer Structure**:
```rust
pub struct ArtifactPointer {
    pub task_id: String,
    pub storage: StorageType, // Redis or S3
    pub location: String,   // Redis key or S3 path
    pub size_bytes: i64,
    pub content_type: String,
}
```

**Files**:
- `crates/context-store/src/lib.rs`
- `crates/context-store/src/store.rs` - Main implementation
- `crates/context-store/src/redis_impl.rs` - Redis backend
- `crates/context-store/src/s3_impl.rs` - S3 backend
- `crates/context-store/src/error.rs`
- `crates/context-store/Cargo.toml`

**Tests**:
- `crates/context-store/tests/routing.rs` - Test size-based routing
- `crates/context-store/tests/redis_impl.rs`
- `crates/context-store/tests/s3_impl.rs`

---

### 2. tool-executor crate

**Purpose**: Tool call execution with permission verification.

**Permission Flow**:
1. Get `agent_type` and tool name from call context
2. Query AgentType registry, verify tool is in allowed set
3. Check node definition's tool subset (can narrow, cannot expand)
4. If allowed → execute; if not → return error

**Tool Call Record**:
```rust
pub struct ToolCallRecord {
    pub tool: String,
    pub args_summary: String,  // First 200 chars only
    pub duration_ms: u64,
    pub status: ToolStatus,    // Success or Failed
    pub error: Option<String>,
}
```

**Files**:
- `crates/tool-executor/src/lib.rs`
- `crates/tool-executor/src/execute.rs` - Main execution logic
- `crates/tool-executor/src/permission.rs` - Permission checks
- `crates/tool-executor/src/registry.rs` - Tool registry
- `crates/tool-executor/src/error.rs`
- `crates/tool-executor/Cargo.toml`

**Built-in Tools** (minimal set for testing):
- `echo` - Returns input unchanged
- `sleep` - Sleeps for specified duration
- `write_temp` - Writes to temp storage

**Tests**:
- `crates/tool-executor/tests/execution.rs`
- `crates/tool-executor/tests/permission.rs`

---

### 3. agent-runtime crate

**Purpose**: Single node complete execution lifecycle.

**Execution Flow**:
1. Read node definition from PostgreSQL
2. Pull upstream results via ContextStore
3. Build LLM prompt (system + instruction + context)
4. Tool call loop:
   - LLM returns tool call → ToolExecutor → result → continue
   - LLM returns final output → validate format → exit loop
5. Write output to ContextStore
6. Update node status → DONE
7. Enqueue downstream nodes

**LLM Interface**:
```rust
pub trait LlmClient: Send + Sync {
    async fn chat(&self, messages: Vec<Message>) -> Result<Message>;
    async fn chat_streaming(&self, messages: Vec<Message>, callback: impl Fn(String)) -> Result<Message>;
}
```

**Files**:
- `crates/agent-runtime/src/lib.rs`
- `crates/agent-runtime/src/runtime.rs` - Main AgentRuntime
- `crates/agent-runtime/src/llm.rs` - LLM client trait
- `crates/agent-runtime/src/prompt.rs` - Prompt building
- `crates/agent-runtime/src/error.rs`
- `crates/agent-runtime/Cargo.toml`

**Tests**:
- `crates/agent-runtime/tests/execution.rs` - Mock LLM, verify flow

---

### 4. executor crate

**Purpose**: Executor Service entry point, Scheduler + Worker Pool.

**Scheduler Behavior**:
1. Weighted round-robin through tenant list
2. Check tenant concurrency quota (Redis counter)
3. Check tenant token quota (Redis cached from PostgreSQL)
4. Dequeue node (SKIP LOCKED)
5. Verify upstream dependencies DONE
6. Dispatch to Worker Pool

**Worker Pool**:
- Fixed size async task pool
- Each worker runs AgentRuntime

**Crash Recovery** (basic):
- On startup, scan for LOCKED entries > 10 minutes
- If node has Artifact → mark DONE
- If no Artifact → reset to PENDING

**Files**:
- `crates/executor/src/lib.rs`
- `crates/executor/src/scheduler.rs` - Scheduling logic
- `crates/executor/src/worker.rs` - Worker pool
- `crates/executor/src/crash_recovery.rs`
- `crates/executor/src/error.rs`
- `Cargo.toml` - Binary entry point
- `src/main.rs`

**Config**:
```rust
pub struct ExecutorConfig {
    pub executor_id: String,
    pub worker_pool_size: usize,
    pub lock_timeout_secs: u64,
    pub crash_recovery_interval_secs: u64,
}
```

**Tests**:
- `crates/executor/tests/scheduler.rs`
- `crates/executor/tests/crash_recovery.rs`

---

## Architecture

```
Phase 1: types → dag → db → queue
                            ↓
Phase 2: context-store → tool-executor → agent-runtime → executor
```

**Dependency Chain**:
```
executor
  └→ agent-runtime
       ├→ tool-executor
       │    └→ db
       ├→ context-store
       │    └→ db
       └→ llm (trait only)
```

---

## Dependencies

**New external crates**:
- `redis-rs` - Redis client
- `aws-sdk-s3` - S3 client
- `async-trait` - Async trait methods
- `tokio` - Async runtime
- `reqwest` - HTTP client for LLM calls

---

## Implementation Order

1. **context-store** - Storage foundation
2. **tool-executor** - Tool execution + permissions
3. **agent-runtime** - Single node execution
4. **executor** - Scheduler + Worker Pool

---

## Files to Create/Modify

```
crates/
├── context-store/           # NEW
│   ├── src/
│   │   ├── lib.rs
│   │   ├── store.rs
│   │   ├── redis_impl.rs
│   │   ├── s3_impl.rs
│   │   └── error.rs
│   ├── tests/
│   │   ├── routing.rs
│   │   ├── redis_impl.rs
│   │   └── s3_impl.rs
│   └── Cargo.toml
├── tool-executor/           # NEW
│   ├── src/
│   │   ├── lib.rs
│   │   ├── execute.rs
│   │   ├── permission.rs
│   │   ├── registry.rs
│   │   └── error.rs
│   ├── tests/
│   │   ├── execution.rs
│   │   └── permission.rs
│   └── Cargo.toml
├── agent-runtime/           # NEW
│   ├── src/
│   │   ├── lib.rs
│   │   ├── runtime.rs
│   │   ├── llm.rs
│   │   ├── prompt.rs
│   │   └── error.rs
│   ├── tests/
│   │   └── execution.rs
│   └── Cargo.toml
└── executor/              # Modify existing workspace
    ├── src/
    │   ├── lib.rs
    │   ├── scheduler.rs
    │   ├── worker.rs
    │   ├── crash_recovery.rs
    │   └── error.rs
    ├── src/main.rs         # NEW
    ├── tests/
    │   ├── scheduler.rs
    │   └── crash_recovery.rs
    └── Cargo.toml
```

---

## Next Phase

Phase 3: Core Enhancements - Checkpointer, VFS, ContextManager
