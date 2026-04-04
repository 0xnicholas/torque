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

### 1. llm crate (NEW - Foundation)

**Purpose**: Unified LLM client for both planner and agent-runtime.

**Trait**:
```rust
#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse>;
    
    async fn chat_streaming(
        &self,
        request: ChatRequest,
        callback: impl Fn(Chunk) + Send + 'static,
    ) -> Result<ChatResponse>;
    
    fn max_tokens(&self) -> usize;
    
    fn count_tokens(&self, text: &str) -> usize;
    
    fn model(&self) -> &str;
}
```

**Types**:
```rust
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub tools: Option<Vec<ToolDef>>,
    pub max_tokens: Option<usize>,
    pub temperature: Option<f32>,
}

pub struct ChatResponse {
    pub message: Message,
    pub usage: TokenUsage,
    pub finish_reason: FinishReason,
}

pub struct Message {
    pub role: String,
    pub content: String,
}

pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

pub struct Chunk {
    pub content: String,
    pub tool_call: Option<ToolCall>,
    pub is_final: bool,
}
```

**OpenAI Implementation**:
```rust
pub struct OpenAiClient {
    http_client: reqwest::Client,
    base_url: String,
    api_key: String,
    default_model: String,
}

impl OpenAiClient {
    pub fn new(base_url: String, api_key: String, default_model: String) -> Self;
    
    pub fn from_env() -> Result<Self> {
        Ok(Self::new(
            std::env::var("LLM_BASE_URL")?,
            std::env::var("LLM_API_KEY")?,
            std::env::var("LLM_AGENT_MODEL")?,
        ))
    }
}
```

**Files**:
- `crates/llm/src/lib.rs`
- `crates/llm/src/client.rs` - LlmClient trait + types
- `crates/llm/src/openai.rs` - OpenAI compatible implementation
- `crates/llm/src/streaming.rs` - Streaming SSE support
- `crates/llm/src/tools.rs` - Tool calling formats
- `crates/llm/src/error.rs`
- `crates/llm/Cargo.toml`

**Dependencies**:
- `reqwest` - HTTP client
- `async-trait` - Async trait methods
- `serde` + `serde_json` - Serialization

**Tests**:
- `crates/llm/tests/client_tests.rs`
- `crates/llm/tests/openai_tests.rs`

---

### 2. context-store crate

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

### 3. tool-executor crate

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

### 4. agent-runtime crate

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
- `crates/agent-runtime/src/prompt.rs` - Prompt building
- `crates/agent-runtime/src/error.rs`
- `crates/agent-runtime/Cargo.toml`

**Note**: `agent-runtime` uses the shared `llm` crate for LLM client, not a local `llm.rs`.

**Tests**:
- `crates/agent-runtime/tests/execution.rs` - Mock LLM, verify flow

---

### 5. executor crate

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
Phase 2: llm → context-store → tool-executor → agent-runtime → executor
```

**Dependency Chain**:
```
executor
  └→ agent-runtime
       ├→ tool-executor
       │    └→ db
       ├→ context-store
       │    └→ db
       └→ llm ← (shared between planner and agent-runtime)
```

---

## Dependencies

**New external crates**:
- `redis-rs` - Redis client
- `aws-sdk-s3` - S3 client
- `async-trait` - Async trait methods
- `tokio` - Async runtime
- `reqwest` - HTTP client (for llm crate)
- `tokio-util` - Streaming utilities (for llm streaming)

---

## Implementation Order

1. **llm** - LLM client foundation (used by both planner and agent-runtime)
2. **context-store** - Storage foundation
3. **tool-executor** - Tool execution + permissions
4. **agent-runtime** - Single node execution
5. **executor** - Scheduler + Worker Pool

---

## Files to Create/Modify

```
crates/
├── llm/                      # NEW - Foundation (Phase 2)
│   ├── src/
│   │   ├── lib.rs
│   │   ├── client.rs
│   │   ├── openai.rs
│   │   ├── streaming.rs
│   │   ├── tools.rs
│   │   └── error.rs
│   ├── tests/
│   │   ├── client_tests.rs
│   │   └── openai_tests.rs
│   └── Cargo.toml
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

**Note**: `agent-runtime` no longer contains `llm.rs` - LLM client is now in the shared `llm` crate.

---

## Next Phase

Phase 3: Core Enhancements - Checkpointer, VFS, ContextManager
