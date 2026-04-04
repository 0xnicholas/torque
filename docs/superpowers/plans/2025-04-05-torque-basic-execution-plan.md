# Torque Basic Execution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enable single node execution from start to finish, with LLM calls and tool execution.

**Architecture:** 
- llm (done) → context-store → tool-executor → agent-runtime → executor
- All depend on Phase 1 crates (types, dag, db, queue)

**Tech Stack:** Rust, tokio, redis-rs, aws-sdk-s3, async-trait, reqwest

---

## File Structure Overview

```
crates/
├── llm/                        # EXISTING (completed)
│   ├── src/
│   │   ├── lib.rs
│   │   ├── client.rs
│   │   ├── openai.rs
│   │   ├── streaming.rs
│   │   ├── tools.rs
│   │   └── error.rs
│   └── tests/
├── context-store/              # NEW
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
├── tool-executor/              # NEW
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
├── agent-runtime/              # NEW
│   ├── src/
│   │   ├── lib.rs
│   │   ├── runtime.rs
│   │   ├── prompt.rs
│   │   └── error.rs
│   ├── tests/
│   │   └── execution.rs
│   └── Cargo.toml
└── executor/                   # MODIFY
    ├── src/
    │   ├── lib.rs
    │   ├── scheduler.rs
    │   ├── worker.rs
    │   ├── crash_recovery.rs
    │   └── error.rs
    ├── src/main.rs            # NEW
    ├── tests/
    │   ├── scheduler.rs
    │   └── crash_recovery.rs
    └── Cargo.toml
```

---

## Phase 1: Context Store (Day 1)

### Task 1: Create context-store crate scaffold

**Files:**
- Create: `crates/context-store/Cargo.toml`
- Create: `crates/context-store/src/lib.rs`

- [ ] **Step 1: Create Cargo.toml**

```toml
[package]
name = "context-store"
version = "0.1.0"
edition = "2021"

[dependencies]
types = { path = "../types" }
redis = { version = "0.25", features = ["tokio-comp", "connection-manager"] }
aws-sdk-s3 = "1"
async-trait = "0.1"
thiserror = "1"
tokio = { version = "1", features = ["full"] }
serde_json = "1"
```

- [ ] **Step 2: Create lib.rs with public exports**

```rust
pub mod store;
pub mod redis_impl;
pub mod s3_impl;
pub mod error;

pub use store::{ContextStore, ArtifactPointer, StorageType};
pub use error::{ContextStoreError, ContextStoreErrorKind};
```

- [ ] **Step 3: Commit**

```bash
git add crates/context-store/
git commit -m "feat(context-store): create context-store crate scaffold"
```

---

### Task 2: Define ContextStore trait and ArtifactPointer

**Files:**
- Create: `crates/context-store/src/store.rs`
- Create: `crates/context-store/src/error.rs`

- [ ] **Step 1: Create error.rs**

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContextStoreError {
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),
    
    #[error("S3 error: {0}")]
    S3(String),
    
    #[error("Serialization error: {0}")]
    Serialization(String),
    
    #[error("Not found: {0}")]
    NotFound(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextStoreErrorKind {
    Redis,
    S3,
    Serialization,
    NotFound,
}
```

- [ ] **Step 2: Create store.rs with trait and routing logic**

```rust
use async_trait::async_trait;
use types::Artifact;
use crate::error::ContextStoreError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageType {
    Redis,
    S3,
}

#[derive(Debug, Clone)]
pub struct ArtifactPointer {
    pub task_id: String,
    pub storage: StorageType,
    pub location: String,
    pub size_bytes: i64,
    pub content_type: String,
}

impl From<Artifact> for ArtifactPointer {
    fn from(a: Artifact) -> Self {
        Self {
            task_id: a.id.to_string(),
            storage: match a.storage {
                types::StorageType::Redis => StorageType::Redis,
                types::StorageType::S3 => StorageType::S3,
            },
            location: a.location,
            size_bytes: a.size_bytes,
            content_type: a.content_type,
        }
    }
}

const SMALL_THRESHOLD: usize = 256 * 1024;
const LARGE_THRESHOLD: usize = 10 * 1024 * 1024;

#[async_trait]
pub trait ContextStore: Send + Sync {
    async fn write(&self, data: &[u8], content_type: &str) -> Result<ArtifactPointer, ContextStoreError>;
    async fn read(&self, pointer: &ArtifactPointer) -> Result<Vec<u8>, ContextStoreError>;
    async fn delete(&self, pointer: &ArtifactPointer) -> Result<(), ContextStoreError>;
}

pub fn route_storage(size_bytes: usize, content_type: &str) -> StorageType {
    if size_bytes < SMALL_THRESHOLD && content_type.contains("json") {
        StorageType::Redis
    } else if size_bytes < LARGE_THRESHOLD {
        StorageType::S3
    } else {
        StorageType::S3
    }
}
```

- [ ] **Step 3: Run cargo check**

```bash
cd crates/context-store && cargo check
```

- [ ] **Step 4: Commit**

```bash
git add crates/context-store/src/store.rs crates/context-store/src/error.rs
git commit -m "feat(context-store): define ContextStore trait and routing logic"
```

---

### Task 3: Implement Redis backend

**Files:**
- Create: `crates/context-store/src/redis_impl.rs`

- [ ] **Step 1: Create redis_impl.rs**

```rust
use async_trait::async_trait;
use redis::aio::ConnectionManager;
use crate::error::ContextStoreError;
use crate::store::{ArtifactPointer, ContextStore, StorageType};

pub struct RedisContextStore {
    conn: ConnectionManager,
    tenant_id: uuid::Uuid,
    ttl_secs: u64,
}

impl RedisContextStore {
    pub fn new(conn: ConnectionManager, tenant_id: uuid::Uuid, ttl_secs: u64) -> Self {
        Self { conn, tenant_id, ttl_secs }
    }
    
    fn make_key(&self, pointer: &ArtifactPointer) -> String {
        format!("{}:node:{}:artifact", self.tenant_id, pointer.task_id)
    }
}

#[async_trait]
impl ContextStore for RedisContextStore {
    async fn write(&self, data: &[u8], content_type: &str) -> Result<ArtifactPointer, ContextStoreError> {
        let task_id = uuid::Uuid::new_v4().to_string();
        let key = format!("{}:node:{}:artifact", self.tenant_id, task_id);
        
        let mut conn = self.conn.clone();
        redis::cmd("SETEX")
            .arg(&key)
            .arg(self.ttl_secs)
            .arg(data)
            .query_async(&mut conn)
            .await?;
        
        Ok(ArtifactPointer {
            task_id,
            storage: StorageType::Redis,
            location: key,
            size_bytes: data.len() as i64,
            content_type: content_type.to_string(),
        })
    }
    
    async fn read(&self, pointer: &ArtifactPointer) -> Result<Vec<u8>, ContextStoreError> {
        let mut conn = self.conn.clone();
        let data: Vec<u8> = redis::cmd("GET")
            .arg(&pointer.location)
            .query_async(&mut conn)
            .await?;
        Ok(data)
    }
    
    async fn delete(&self, pointer: &ArtifactPointer) -> Result<(), ContextStoreError> {
        let mut conn = self.conn.clone();
        redis::cmd("DEL")
            .arg(&pointer.location)
            .query_async(&mut conn)
            .await?;
        Ok(())
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/context-store/src/redis_impl.rs
git commit -m "feat(context-store): implement Redis backend"
```

---

### Task 4: Implement S3 backend

**Files:**
- Create: `crates/context-store/src/s3_impl.rs`

- [ ] **Step 1: Create s3_impl.rs**

```rust
use async_trait::async_trait;
use aws_sdk_s3::Client;
use crate::error::ContextStoreError;
use crate::store::{ArtifactPointer, ContextStore, StorageType};

pub struct S3ContextStore {
    client: Client,
    bucket: String,
    tenant_id: uuid::Uuid,
}

impl S3ContextStore {
    pub fn new(client: Client, bucket: String, tenant_id: uuid::Uuid) -> Self {
        Self { client, bucket, tenant_id }
    }
    
    fn make_key(&self, pointer: &ArtifactPointer) -> String {
        format!("{}/{}/{}", self.tenant_id, pointer.task_id, "artifact")
    }
}

#[async_trait]
impl ContextStore for S3ContextStore {
    async fn write(&self, data: &[u8], content_type: &str) -> Result<ArtifactPointer, ContextStoreError> {
        let task_id = uuid::Uuid::new_v4().to_string();
        let key = format!("{}/{}/artifact", self.tenant_id, task_id);
        
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(&key)
            .body(aws_sdk_s3::primitives::ByteStream::from(data.to_vec()))
            .content_type(content_type)
            .send()
            .await
            .map_err(|e| ContextStoreError::S3(e.to_string()))?;
        
        Ok(ArtifactPointer {
            task_id,
            storage: StorageType::S3,
            location: key,
            size_bytes: data.len() as i64,
            content_type: content_type.to_string(),
        })
    }
    
    async fn read(&self, pointer: &ArtifactPointer) -> Result<Vec<u8>, ContextStoreError> {
        let output = self.client
            .get_object()
            .bucket(&self.bucket)
            .key(&pointer.location)
            .send()
            .await
            .map_err(|e| ContextStoreError::S3(e.to_string()))?;
        
        let bytes = output
            .body
            .collect()
            .await
            .map_err(|e| ContextStoreError::S3(e.to_string()))?
            .into_bytes();
        
        Ok(bytes.to_vec())
    }
    
    async fn delete(&self, pointer: &ArtifactPointer) -> Result<(), ContextStoreError> {
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(&pointer.location)
            .send()
            .await
            .map_err(|e| ContextStoreError::S3(e.to_string()))?;
        Ok(())
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/context-store/src/s3_impl.rs
git commit -m "feat(context-store): implement S3 backend"
```

---

## Phase 2: Tool Executor (Day 2)

### Task 5: Create tool-executor crate scaffold

**Files:**
- Create: `crates/tool-executor/Cargo.toml`
- Create: `crates/tool-executor/src/lib.rs`

- [ ] **Step 1: Create Cargo.toml**

```toml
[package]
name = "tool-executor"
version = "0.1.0"
edition = "2021"

[dependencies]
types = { path = "../types" }
db = { path = "../db" }
async-trait = "0.1"
thiserror = "1"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
```

- [ ] **Step 2: Create lib.rs**

```rust
pub mod execute;
pub mod permission;
pub mod registry;
pub mod error;

pub use error::{ToolError, ToolErrorKind};
pub use execute::ToolExecutor;
pub use registry::{ToolRegistry, ToolHandler};
```

- [ ] **Step 3: Create error.rs**

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ToolError {
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    
    #[error("Tool not found: {0}")]
    ToolNotFound(String),
    
    #[error("Execution error: {0}")]
    Execution(String),
    
    #[error("Timeout: {0}")]
    Timeout(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolErrorKind {
    PermissionDenied,
    ToolNotFound,
    Execution,
    Timeout,
}
```

- [ ] **Step 4: Commit**

```bash
git add crates/tool-executor/
git commit -m "feat(tool-executor): create tool-executor crate scaffold"
```

---

### Task 6: Implement ToolRegistry

**Files:**
- Create: `crates/tool-executor/src/registry.rs`

- [ ] **Step 1: Create registry.rs**

```rust
use std::collections::HashMap;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub output: String,
    pub error: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[async_trait]
pub trait ToolHandler: Send + Sync {
    async fn call(&self, args: serde_json::Value) -> Result<ToolResult, String>;
}

pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn ToolHandler>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: HashMap::new() }
    }
    
    pub fn register<F, Fut>(&mut self, name: &str, handler: F)
    where
        F: Fn(serde_json::Value) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<ToolResult, String>> + Send,
    {
        self.tools.insert(name.to_string(), Box::new(move |args| {
            let f = handler(args);
            Box::pin(async move { f.await }) as _
        }));
    }
    
    pub async fn execute(&self, call: ToolCall) -> Result<ToolResult, String> {
        let handler = self.tools
            .get(&call.name)
            .ok_or_else(|| format!("Tool not found: {}", call.name))?;
        
        handler.call(call.arguments).await
    }
    
    pub fn get_tool_names(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/tool-executor/src/registry.rs
git commit -m "feat(tool-executor): implement ToolRegistry"
```

---

### Task 7: Implement permission checks and execution

**Files:**
- Create: `crates/tool-executor/src/permission.rs`
- Create: `crates/tool-executor/src/execute.rs`

- [ ] **Step 1: Create permission.rs**

```rust
use types::AgentType;
use crate::error::ToolError;

pub struct PermissionChecker;

impl PermissionChecker {
    pub fn check(agent_type: &AgentType, tool_name: &str, allowed_tools: Option<&[String]>) -> Result<(), ToolError> {
        let tools = allowed_tools.unwrap_or(&agent_type.tools);
        
        if !tools.contains(&tool_name.to_string()) {
            return Err(ToolError::PermissionDenied(format!(
                "Tool '{}' not allowed for agent type '{}'",
                tool_name, agent_type.name
            )));
        }
        
        Ok(())
    }
}
```

- [ ] **Step 2: Create execute.rs**

```rust
use std::sync::Arc;
use crate::error::ToolError;
use crate::registry::{ToolRegistry, ToolCall, ToolResult};
use types::AgentType;

pub struct ToolExecutor {
    registry: Arc<ToolRegistry>,
}

impl ToolExecutor {
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        Self { registry }
    }
    
    pub async fn execute(
        &self,
        agent_type: &AgentType,
        call: ToolCall,
        allowed_tools: Option<&[String]>,
    ) -> Result<ToolResult, ToolError> {
        crate::permission::PermissionChecker::check(agent_type, &call.name, allowed_tools)
            .map_err(|e| ToolError::PermissionDenied(e.to_string()))?;
        
        let result = self.registry.execute(call).await
            .map_err(|e| ToolError::Execution(e))?;
        
        Ok(result)
    }
}
```

- [ ] **Step 3: Commit**

```bash
git add crates/tool-executor/src/permission.rs crates/tool-executor/src/execute.rs
git commit -m "feat(tool-executor): implement permission checks and execution"
```

---

## Phase 3: Agent Runtime (Day 3)

### Task 8: Create agent-runtime crate scaffold

**Files:**
- Create: `crates/agent-runtime/Cargo.toml`
- Create: `crates/agent-runtime/src/lib.rs`

- [ ] **Step 1: Create Cargo.toml**

```toml
[package]
name = "agent-runtime"
version = "0.1.0"
edition = "2021"

[dependencies]
types = { path = "../types" }
db = { path = "../db" }
llm = { path = "../llm" }
context-store = { path = "../context-store" }
tool-executor = { path = "../tool-executor" }
async-trait = "0.1"
thiserror = "1"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
```

- [ ] **Step 2: Create lib.rs**

```rust
pub mod runtime;
pub mod prompt;
pub mod error;

pub use error::{AgentError, AgentErrorKind};
pub use runtime::AgentRuntime;
```

- [ ] **Step 3: Create error.rs**

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AgentError {
    #[error("LLM error: {0}")]
    Llm(String),
    
    #[error("Tool execution error: {0}")]
    Tool(String),
    
    #[error("Context error: {0}")]
    Context(String),
    
    #[error("Max iterations exceeded")]
    MaxIterations,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentErrorKind {
    Llm,
    Tool,
    Context,
    MaxIterations,
}
```

- [ ] **Step 4: Commit**

```bash
git add crates/agent-runtime/
git commit -m "feat(agent-runtime): create agent-runtime crate scaffold"
```

---

### Task 9: Implement AgentRuntime

**Files:**
- Create: `crates/agent-runtime/src/runtime.rs`
- Create: `crates/agent-runtime/src/prompt.rs`

- [ ] **Step 1: Create prompt.rs**

```rust
use types::Message;

pub struct PromptBuilder;

impl PromptBuilder {
    pub fn build_system_prompt(system_prompt: &str, tools: &[String]) -> String {
        let tools_json = tools.iter()
            .map(|t| format!("  - {}", t))
            .collect::<Vec<_>>()
            .join("\n");
        
        format!(
            "{}\n\nAvailable tools:\n{}\n\nOutput format: {{\"output\": \"...\", \"__requires_approval\": false, \"__metadata\": {{}}}}",
            system_prompt,
            tools_json
        )
    }
    
    pub fn build_initial_message(instruction: &str, context: &[Message]) -> Vec<Message> {
        let mut messages = vec![
            Message {
                role: "system".to_string(),
                content: instruction.to_string(),
            }
        ];
        messages.extend(context.iter().cloned());
        messages
    }
}
```

- [ ] **Step 2: Create runtime.rs**

```rust
use std::sync::Arc;
use types::{Node, NodeStatus};
use llm::LlmClient;
use context_store::ContextStore;
use tool_executor::{ToolExecutor, ToolRegistry, ToolCall, ToolResult};
use crate::error::AgentError;
use crate::prompt::PromptBuilder;

const MAX_TOOL_CALLS: u32 = 20;

pub struct AgentRuntime {
    llm: Arc<dyn LlmClient>,
    context_store: Arc<dyn ContextStore>,
    tool_executor: Arc<ToolExecutor>,
}

impl AgentRuntime {
    pub fn new(
        llm: Arc<dyn LlmClient>,
        context_store: Arc<dyn ContextStore>,
        tool_registry: Arc<ToolRegistry>,
    ) -> Self {
        Self {
            llm,
            context_store,
            tool_executor: Arc::new(ToolExecutor::new(tool_registry)),
        }
    }
    
    pub async fn execute(&self, node: &mut Node) -> Result<String, AgentError> {
        let system_prompt = PromptBuilder::build_system_prompt(
            &node.agent_type,
            &node.tools.clone().unwrap_or_default(),
        );
        
        let mut messages = vec![
            types::Message {
                role: "system".to_string(),
                content: system_prompt,
            },
            types::Message {
                role: "user".to_string(),
                content: node.instruction.clone(),
            }
        ];
        
        let mut tool_call_count = 0;
        
        loop {
            if tool_call_count >= MAX_TOOL_CALLS {
                return Err(AgentError::MaxIterations);
            }
            
            let response = self.llm.chat(messages.clone())
                .await
                .map_err(|e| AgentError::Llm(e.to_string()))?;
            
            messages.push(response.message.clone());
            
            if let Some(tool_call) = response.message.tool_call {
                tool_call_count += 1;
                
                let result = self.tool_executor
                    .execute(&types::AgentType::new(node.agent_type.clone(), "".to_string(), vec![]), tool_call.clone(), node.tools.as_ref())
                    .await
                    .map_err(|e| AgentError::Tool(e.to_string()))?;
                
                messages.push(types::Message {
                    role: "tool".to_string(),
                    content: result.output,
                });
            } else {
                let output = &response.message.content;
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(output) {
                    if let Some(o) = parsed.get("output") {
                        return Ok(o.to_string());
                    }
                }
                return Ok(output.clone());
            }
        }
    }
}
```

- [ ] **Step 3: Run cargo check**

```bash
cd crates/agent-runtime && cargo check
```

- [ ] **Step 4: Commit**

```bash
git add crates/agent-runtime/src/runtime.rs crates/agent-runtime/src/prompt.rs
git commit -m "feat(agent-runtime): implement AgentRuntime with tool call loop"
```

---

## Phase 4: Executor (Day 4)

### Task 10: Create executor crate main entry

**Files:**
- Create: `crates/executor/Cargo.toml` (modify)
- Create: `crates/executor/src/main.rs`

- [ ] **Step 1: Update Cargo.toml**

```toml
[package]
name = "executor"
version = "0.1.0"
edition = "2021"

[dependencies]
types = { path = "../types" }
db = { path = "../db" }
queue = { path = "../queue" }
llm = { path = "../llm" }
context-store = { path = "../context-store" }
tool-executor = { path = "../tool-executor" }
agent-runtime = { path = "../agent-runtime" }
tokio = { version = "1", features = ["full"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["json"] }
```

- [ ] **Step 2: Create main.rs**

```rust
use std::sync::Arc;
use executor::{Executor, ExecutorConfig, ExecutorError};

#[tokio::main]
async fn main() -> Result<(), ExecutorError> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .json()
        .init();
    
    let config = ExecutorConfig {
        executor_id: std::env::var("EXECUTOR_ID")
            .unwrap_or_else(|_| format!("executor-{}", uuid::Uuid::new_v4())),
        worker_pool_size: std::env::var("EXECUTOR_WORKER_POOL_SIZE")
            .unwrap_or_else(|_| "10".to_string())
            .parse()
            .unwrap_or(10),
        lock_timeout_secs: std::env::var("EXECUTOR_LOCK_TIMEOUT_SECS")
            .unwrap_or_else(|_| "600".to_string())
            .parse()
            .unwrap_or(600),
        crash_recovery_interval_secs: 30,
    };
    
    let executor = Executor::new(config).await?;
    executor.run().await
}
```

- [ ] **Step 3: Commit**

```bash
git add crates/executor/src/main.rs
git commit -m "feat(executor): add main entry point"
```

---

### Task 11: Implement Executor with Scheduler and Worker Pool

**Files:**
- Modify: `crates/executor/src/lib.rs`
- Create: `crates/executor/src/scheduler.rs`
- Create: `crates/executor/src/worker.rs`

- [ ] **Step 1: Create scheduler.rs**

```rust
use std::collections::VecDeque;
use types::Tenant;

pub struct Scheduler {
    tenants: VecDeque<uuid::Uuid>,
    current_index: usize,
}

impl Scheduler {
    pub fn new(tenants: Vec<Tenant>) -> Self {
        let mut ids: VecDeque<_> = tenants.into_iter()
            .flat_map(|t| std::iter::repeat(t.id).take(t.weight as usize))
            .collect();
        ids.make_contiguous().sort();
        ids.dedup();
        let ids = VecDeque::from(ids);
        Self { tenants: ids, current_index: 0 }
    }
    
    pub fn next(&mut self) -> Option<uuid::Uuid> {
        if self.tenants.is_empty() {
            return None;
        }
        let len = self.tenants.len();
        for _ in 0..len {
            let id = self.tenants[self.current_index];
            self.current_index = (self.current_index + 1) % len;
            return Some(id);
        }
        None
    }
}
```

- [ ] **Step 2: Create worker.rs**

```rust
use std::sync::Arc;
use types::{Node, NodeStatus};
use agent_runtime::AgentRuntime;
use db::PgPool;

pub struct Worker {
    id: usize,
    runtime: Arc<AgentRuntime>,
    pool: PgPool,
}

impl Worker {
    pub fn new(id: usize, runtime: Arc<AgentRuntime>, pool: PgPool) -> Self {
        Self { id, runtime, pool }
    }
    
    pub async fn run_node(&self, mut node: Node) -> Result<(), String> {
        let output = self.runtime.execute(&mut node).await
            .map_err(|e| e.to_string())?;
        
        db::nodes::update_status(&self.pool, node.id, NodeStatus::Done)
            .await
            .map_err(|e| e.to_string())?;
        
        tracing::info!(node_id = %node.id, "Node completed");
        Ok(())
    }
}
```

- [ ] **Step 3: Update lib.rs with Executor struct**

```rust
pub mod scheduler;
pub mod worker;
pub mod crash_recovery;
pub mod error;

pub use error::ExecutorError;
pub use scheduler::Scheduler;
pub use worker::Worker;

use std::sync::Arc;
use tokio::sync::Semaphore;
use db::PgPool;
use crate::scheduler::Scheduler;

pub struct ExecutorConfig {
    pub executor_id: String,
    pub worker_pool_size: usize,
    pub lock_timeout_secs: u64,
    pub crash_recovery_interval_secs: u64,
}

pub struct Executor {
    config: ExecutorConfig,
    pool: PgPool,
    scheduler: Scheduler,
    worker_semaphore: Arc<Semaphore>,
}

impl Executor {
    pub async fn new(config: ExecutorConfig) -> Result<Self, ExecutorError> {
        let database_url = std::env::var("DATABASE_URL")
            .map_err(|_| ExecutorError::Config("DATABASE_URL not set".to_string()))?;
        
        let pool = PgPool::connect(&database_url)
            .await
            .map_err(|e| ExecutorError::Database(e.to_string()))?;
        
        let tenants = db::tenants::list_all(&pool)
            .await
            .map_err(|e| ExecutorError::Database(e.to_string()))?;
        
        let scheduler = Scheduler::new(tenants);
        
        Ok(Self {
            config,
            pool,
            scheduler,
            worker_semaphore: Arc::new(Semaphore::new(config.worker_pool_size)),
        })
    }
    
    pub async fn run(&self) -> Result<(), ExecutorError> {
        tracing::info!(executor_id = %self.config.executor_id, "Starting executor");
        
        loop {
            if let Some(tenant_id) = self.scheduler.next() {
                let permit = self.worker_semaphore.clone().acquire_owned().await;
                
                let entry = queue::dequeue(&self.pool, tenant_id, &self.config.executor_id)
                    .await
                    .map_err(|e| ExecutorError::Queue(e.to_string()))?;
                
                if let Some(e) = entry {
                    let _permit = permit;
                    let node = db::nodes::get(&self.pool, e.node_id)
                        .await
                        .map_err(|e| ExecutorError::Database(e.to_string()))?;
                    
                    if let Some(mut node) = node {
                        let _ = crash_recovery::execute_node(&self.pool, &node).await;
                    }
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }
}
```

- [ ] **Step 4: Run cargo check**

```bash
cd crates/executor && cargo check
```

- [ ] **Step 5: Commit**

```bash
git add crates/executor/src/lib.rs crates/executor/src/scheduler.rs crates/executor/src/worker.rs
git commit -m "feat(executor): implement Executor with Scheduler and Worker Pool"
```

---

## Phase 5: Integration (Day 5)

### Task 12: Workspace verification

- [ ] **Step 1: Run cargo check --workspace**

```bash
cargo check --workspace
```

- [ ] **Step 2: Run cargo test --workspace**

```bash
cargo test --workspace
```

- [ ] **Step 3: Commit integration**

```bash
git add -A
git commit -m "feat: integrate all Phase 2 crates"
```

---

## Summary

| Phase | Tasks | Duration |
|-------|-------|----------|
| Phase 1: ContextStore | Tasks 1-4 | Day 1 |
| Phase 2: ToolExecutor | Tasks 5-7 | Day 2 |
| Phase 3: AgentRuntime | Tasks 8-9 | Day 3 |
| Phase 4: Executor | Tasks 10-11 | Day 4 |
| Phase 5: Integration | Task 12 | Day 5 |

**Total Estimated Time:** 5 days

---

**Plan complete and saved to** `docs/superpowers/plans/2025-04-05-torque-basic-execution-plan.md`

**Two execution options:**

**1. Subagent-Driven (recommended)** - Dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints
