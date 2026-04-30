# Test Infrastructure & E2E Tests Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish test infrastructure for torque-harness-lite E2E tests and API tests for core endpoints, achieving 60%+ test coverage on these subsystems.

**Architecture:** Create a test infrastructure layer with reusable TestApp, FakeLlm, and test helpers. Build E2E tests for torque-harness-lite (simple execution, tool execution, checkpoints) and API tests for Agent Definitions and Instances endpoints.

**Tech Stack:** Rust (tokio, serial_test, tower for HTTP testing), axum for API, tower::ServiceExt for test clients

---

## File Structure

```
crates/torque-harness-lite/tests/
├── common/
│   ├── mod.rs              # Re-exports fake implementations
│   └── fake_llm.rs        # Enhanced FakeLlm for lite tests
├── execution_tests.rs       # Simple execution flow tests
├── tool_execution_tests.rs # Tool call flow tests
├── checkpoint_tests.rs      # Checkpoint creation/structure tests
└── multi_turn_tests.rs    # Multi-turn conversation tests

crates/torque-harness/tests/api/
├── common/
│   ├── mod.rs              # TestApp builder, helpers
│   └── test_app.rs         # TestApp struct with HTTP helpers
├── agent_definitions_tests.rs
├── agent_instances_tests.rs
└── runs_tests.rs

crates/torque-harness/tests/common/fake_llm.rs  # Enhanced with more response types
```

---

## Task 1: Enhance FakeLlm for Testing

**Files:**
- Modify: `crates/torque-harness/tests/common/fake_llm.rs:1-151`
- Create: `crates/torque-harness-lite/tests/common/fake_llm.rs`

- [ ] **Step 1: Review existing FakeLlm implementation**

Read: `crates/torque-harness/tests/common/fake_llm.rs`
Understand: `single_text()`, `json_response()`, `tool_call_then_text()` patterns

- [ ] **Step 2: Add error_response() method**

Modify: `crates/torque-harness/tests/common/fake_llm.rs`

```rust
pub fn error_response(message: &str) -> Self {
    let message = message.to_string();
    Self {
        model: "fake-model".to_string(),
        scripted: Mutex::new(VecDeque::from([ScriptedResponse {
            chunks: vec![Chunk::error(message.clone())],
            finish_reason: FinishReason::Error,
            message_content: String::new(),
        }])),
        requests: Mutex::new(Vec::new()),
    }
}
```

- [ ] **Step 3: Add streaming_chunks() method**

Modify: `crates/torque-harness/tests/common/fake_llm.rs`

```rust
pub fn streaming_chunks(chunks: Vec<&str>) -> Self {
    let joined = chunks.join("");
    Self {
        model: "fake-model".to_string(),
        scripted: Mutex::new(VecDeque::from([ScriptedResponse {
            chunks: chunks.iter().map(|c| Chunk::content(c.to_string())).collect(),
            finish_reason: FinishReason::Stop,
            message_content: joined,
        }])),
        requests: Mutex::new(Vec::new()),
    }
}
```

- [ ] **Step 4: Add tool_call_then_error() method**

Modify: `crates/torque-harness/tests/common/fake_llm.rs`

```rust
pub fn tool_call_then_error(
    tool_name: &str,
    arguments: Value,
    error_message: &str,
) -> Self {
    let tool_call = ToolCall {
        id: "tool-call-1".to_string(),
        name: tool_name.to_string(),
        arguments,
    };

    Self {
        model: "fake-model".to_string(),
        scripted: Mutex::new(VecDeque::from([
            ScriptedResponse {
                chunks: vec![Chunk::with_tool_call(tool_call)],
                finish_reason: FinishReason::ToolCalls,
                message_content: String::new(),
            },
            ScriptedResponse {
                chunks: vec![Chunk::error(error_message.to_string())],
                finish_reason: FinishReason::Error,
                message_content: String::new(),
            },
        ])),
        requests: Mutex::new(Vec::new()),
    }
}
```

- [ ] **Step 5: Add approval_needed() method**

Modify: `crates/torque-harness/tests/common/fake_llm.rs`

```rust
pub fn approval_needed(reason: &str) -> Self {
    let reason = reason.to_string();
    Self {
        model: "fake-model".to_string(),
        scripted: Mutex::new(VecDeque::from([ScriptedResponse {
            chunks: vec![Chunk::approval_required(reason)],
            finish_reason: FinishReason::ApprovalRequired,
            message_content: String::new(),
        }])),
        requests: Mutex::new(Vec::new()),
    }
}
```

- [ ] **Step 6: Copy enhanced FakeLlm to torque-harness-lite tests**

Create: `crates/torque-harness-lite/tests/common/fake_llm.rs`
Copy entire implementation from step 2-5

- [ ] **Step 7: Add Chunk::error and Chunk::approval_required variants**

Modify: `crates/torque-harness/tests/common/fake_llm.rs`
Add to Chunk enum/builder:

```rust
impl Chunk {
    pub fn error(content: String) -> Self {
        Chunk::Content { content }
    }

    pub fn approval_required(reason: String) -> Self {
        Chunk::Content { content: format!("[APPROVAL_REQUIRED: {}]", reason) }
    }
}
```

- [ ] **Step 8: Run tests to verify FakeLlm compiles**

Run: `cargo test --package torque-harness --lib -- fake_llm`
Expected: PASS (no tests yet, just compilation)

- [ ] **Step 9: Commit**

```bash
git add crates/torque-harness/tests/common/fake_llm.rs crates/torque-harness-lite/tests/common/fake_llm.rs
git commit -m "test: enhance FakeLlm with error_response, streaming_chunks, tool_call_then_error, approval_needed"
```

---

## Task 2: Create torque-harness-lite Test Infrastructure

**Files:**
- Create: `crates/torque-harness-lite/tests/common/mod.rs`
- Create: `crates/torque-harness-lite/tests/common/helpers.rs`

- [ ] **Step 1: Create tests/common/mod.rs**

Create: `crates/torque-harness-lite/tests/common/mod.rs`

```rust
pub mod fake_llm;

pub use fake_llm::FakeLlm;

pub mod helpers {
    use super::fake_llm::FakeLlm;
    use std::sync::Arc;
    use torque_kernel::AgentDefinition;
    use torque_runtime::environment::{RuntimeCheckpointSink, RuntimeEventSink, RuntimeModelDriver, RuntimeToolExecutor};
    use torque_runtime::host::RuntimeHost;

    pub fn create_test_host(
        fake_llm: FakeLlm,
    ) -> (
        Arc<dyn RuntimeEventSink>,
        Arc<dyn RuntimeCheckpointSink>,
        RuntimeHost,
    ) {
        let event_sink = Arc::new(InMemoryEventSink::default()) as Arc<dyn RuntimeEventSink>;
        let checkpoint_sink = Arc::new(InMemoryCheckpointSink::default()) as Arc<dyn RuntimeCheckpointSink>;
        let agent_def = AgentDefinition::new("test-agent", "You are a helpful assistant.");

        let host = RuntimeHost::new(vec![agent_def], event_sink.clone(), checkpoint_sink.clone());

        (event_sink, checkpoint_sink, host)
    }
}

pub struct InMemoryEventSink {
    results: std::sync::Mutex<Vec<torque_kernel::ExecutionResult>>,
    checkpoint_count: std::sync::Mutex<usize>,
}

impl Default for InMemoryEventSink {
    fn default() -> Self {
        Self {
            results: std::sync::Mutex::new(Vec::new()),
            checkpoint_count: std::sync::Mutex::new(0),
        }
    }
}

impl InMemoryEventSink {
    pub fn execution_count(&self) -> usize {
        self.results.lock().unwrap().len()
    }
}

#[async_trait::async_trait]
impl RuntimeEventSink for InMemoryEventSink {
    async fn record_execution_result(&self, result: &torque_kernel::ExecutionResult) -> anyhow::Result<()> {
        self.results.lock().unwrap().push(result.clone());
        Ok(())
    }

    async fn record_checkpoint_created(
        &self,
        _checkpoint_id: uuid::Uuid,
        _instance_id: torque_kernel::AgentInstanceId,
        _reason: &str,
    ) -> anyhow::Result<()> {
        *self.checkpoint_count.lock().unwrap() += 1;
        Ok(())
    }
}

pub struct InMemoryCheckpointSink {
    payloads: std::sync::Mutex<Vec<torque_runtime::checkpoint::RuntimeCheckpointPayload>>,
    save_count: std::sync::Mutex<usize>,
}

impl Default for InMemoryCheckpointSink {
    fn default() -> Self {
        Self {
            payloads: std::sync::Mutex::new(Vec::new()),
            save_count: std::sync::Mutex::new(0),
        }
    }
}

impl InMemoryCheckpointSink {
    pub fn save_count(&self) -> usize {
        *self.save_count.lock().unwrap()
    }
}

#[async_trait::async_trait]
impl RuntimeCheckpointSink for InMemoryCheckpointSink {
    async fn save(
        &self,
        payload: torque_runtime::checkpoint::RuntimeCheckpointPayload,
    ) -> anyhow::Result<torque_runtime::checkpoint::RuntimeCheckpointRef> {
        *self.save_count.lock().unwrap() += 1;
        let checkpoint_id = uuid::Uuid::new_v4();
        let instance_id = payload.instance_id.as_uuid();
        self.payloads.lock().unwrap().push(payload);
        Ok(torque_runtime::checkpoint::RuntimeCheckpointRef {
            checkpoint_id,
            instance_id,
        })
    }
}
```

- [ ] **Step 2: Add required imports to Cargo.toml**

Modify: `crates/torque-harness-lite/Cargo.toml`

Add under `[dev-dependencies]`:

```toml
[dev-dependencies]
tempfile = "3"
serial_test = "3"
tokio-test = "0.4"
async-trait = "0.1"
uuid = { version = "1", features = ["v4", "serde"] }
```

- [ ] **Step 3: Verify compilation**

Run: `cargo build --package torque-harness-lite`
Expected: SUCCESS (with warnings about unused code)

- [ ] **Step 4: Commit**

```bash
git add crates/torque-harness-lite/tests/common/mod.rs crates/torque-harness-lite/Cargo.toml
git commit -m "test(torque-harness-lite): add test infrastructure with InMemoryEventSink and InMemoryCheckpointSink"
```

---

## Task 3: torque-harness-lite Execution Tests

**Files:**
- Create: `crates/torque-harness-lite/tests/execution_tests.rs`

> **Important:** This task depends on Task 1 (FakeLlm enhancements) and Task 2 (test infrastructure) being complete. Do NOT start Task 3 until Tasks 1 and 2 are committed.

- [ ] **Step 1: Define LiteModelDriver and LiteToolExecutor wrappers BEFORE writing tests**

Create: `crates/torque-harness-lite/tests/execution_tests.rs`

```rust
mod common;

use common::fake_llm::FakeLlm;
use torque_kernel::{AgentDefinition, ExecutionRequest};
use torque_runtime::host::RuntimeHost;
use torque_runtime::message::{RuntimeMessage, RuntimeMessageRole};
use torque_runtime::environment::{RuntimeModelDriver, RuntimeToolExecutor, RuntimeOutputSink};
use torque_runtime::tools::RuntimeToolDef;
use torque_runtime::events::ModelTurnResult;
use torque_runtime::checkpoint::RuntimeCheckpointRef;
use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use llm::{LlmClient, ChatRequest, Chunk, FinishReason, Message};

// Define LiteModelDriver BEFORE using it in tests
struct LiteModelDriver {
    llm: Arc<dyn LlmClient>,
}

impl LiteModelDriver {
    fn new(llm: Arc<dyn LlmClient>) -> Self {
        Self { llm }
    }
}

#[async_trait]
impl RuntimeModelDriver for LiteModelDriver {
    async fn run_turn(
        &self,
        messages: Vec<RuntimeMessage>,
        _tools: Vec<RuntimeToolDef>,
        sink: Option<&dyn RuntimeOutputSink>,
    ) -> anyhow::Result<ModelTurnResult> {
        let llm_messages: Vec<Message> = messages
            .into_iter()
            .map(|m| match m.role() {
                RuntimeMessageRole::User => Message::user(m.content().unwrap_or_default()),
                RuntimeMessageRole::Assistant => Message::assistant(m.content().unwrap_or_default()),
                RuntimeMessageRole::System => Message::system(m.content().unwrap_or_default()),
                _ => Message::user(m.content().unwrap_or_default()),
            })
            .collect();

        let text_chunks = Arc::new(Mutex::new(Vec::<String>::new()));
        let text_chunks_clone = text_chunks.clone();

        let callback = Box::new(move |chunk: Chunk| {
            if !chunk.content.is_empty() {
                text_chunks_clone.lock().unwrap().push(chunk.content.clone());
            }
        });

        let response = self.llm.chat_streaming(
            ChatRequest::new(self.llm.model().to_string(), llm_messages),
            callback,
        ).await?;

        let assistant_text = text_chunks.lock().unwrap().join("");

        if let Some(sink) = sink {
            if !assistant_text.is_empty() {
                sink.on_text_chunk(&assistant_text);
            }
        }

        Ok(ModelTurnResult {
            finish_reason: match response.finish_reason {
                FinishReason::Stop => torque_runtime::events::RuntimeFinishReason::Stop,
                FinishReason::Length => torque_runtime::events::RuntimeFinishReason::Length,
                FinishReason::ContentFilter => torque_runtime::events::RuntimeFinishReason::ContentFilter,
                FinishReason::ToolCalls => torque_runtime::events::RuntimeFinishReason::ToolCalls,
                FinishReason::Error => torque_runtime::events::RuntimeFinishReason::Stop,
            },
            assistant_text,
            tool_calls: vec![],
        })
    }
}

// Now write the test using the above definition
#[tokio::test]
async fn test_simple_execution_completes() {
    let fake_llm = FakeLlm::single_text("Task completed successfully");
    let llm = std::sync::Arc::new(fake_llm);
    let model_driver = LiteModelDriver::new(llm);
    let tool_executor = LiteToolExecutor::new();

    let event_sink = Arc::new(InMemoryEventSink::default());
    let checkpoint_sink = Arc::new(InMemoryCheckpointSink::default());
    let agent_def = AgentDefinition::new("test-agent", "You are helpful.");
    let agent_def_id = agent_def.id;

    let mut host = RuntimeHost::new(vec![agent_def], event_sink.clone(), checkpoint_sink.clone());

    let request = ExecutionRequest::new(agent_def_id, "Do something", vec![]);
    let messages = vec![RuntimeMessage::new(RuntimeMessageRole::User, "Do something".into())];

    let result = host
        .execute_v1(request, &model_driver, &tool_executor, None, messages)
        .await;

    assert!(result.is_ok(), "Execution should succeed: {:?}", result.err());
    let result = result.unwrap();
    assert!(result.summary.is_some(), "Result should have summary");
}
```

- [ ] **Step 2: Define LiteToolExecutor wrapper**

Add to `crates/torque-harness-lite/tests/execution_tests.rs` (after LiteModelDriver):

```rust
// LiteToolExecutor wraps the torque-harness-lite tool executor for tests
struct LiteToolExecutor;

impl LiteToolExecutor {
    fn new() -> Self {
        LiteToolExecutor
    }
}

#[async_trait]
impl RuntimeToolExecutor for LiteToolExecutor {
    async fn execute(
        &self,
        _ctx: torque_runtime::environment::RuntimeExecutionContext,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> anyhow::Result<torque_runtime::tools::RuntimeToolResult> {
        // Return success for known tools, failure for unknown
        match tool_name {
            "read_file" | "write_file" | "ls" | "edit_file" | "glob" | "grep" => {
                Ok(torque_runtime::tools::RuntimeToolResult::success("ok"))
            }
            _ => Ok(torque_runtime::tools::RuntimeToolResult::failure(format!("unknown tool: {}", tool_name))),
        }
    }

    async fn tool_defs(&self) -> anyhow::Result<Vec<RuntimeToolDef>> {
        Ok(vec![])
    }
}
```

- [ ] **Step 3: Run test to verify it compiles and passes**

Run: `cargo test --package torque-harness-lite --test execution_tests test_simple_execution_completes -- --nocapture`
Expected: PASS

- [ ] **Step 4: Add test for instance and task creation**

Add to `crates/torque-harness-lite/tests/execution_tests.rs`:

```rust
#[tokio::test]
async fn test_request_creates_instance_and_task() {
    let fake_llm = FakeLlm::single_text("Done");
    let llm = Arc::new(fake_llm);
    let model_driver = LiteModelDriver { llm };
    let tool_executor = LiteToolExecutor::new();

    let event_sink = Arc::new(InMemoryEventSink::default());
    let checkpoint_sink = Arc::new(InMemoryCheckpointSink::default());
    let agent_def = AgentDefinition::new("test-agent", "You are helpful.");
    let agent_def_id = agent_def.id;

    let mut host = RuntimeHost::new(vec![agent_def], event_sink.clone(), checkpoint_sink.clone());

    let request = ExecutionRequest::new(agent_def_id, "Do something", vec![]);
    let messages = vec![RuntimeMessage::new(RuntimeMessageRole::User, "Do something".into())];

    let result = host
        .execute_v1(request, &model_driver, &tool_executor, None, messages)
        .await
        .unwrap();

    assert!(result.instance_id.as_uuid() != uuid::Uuid::nil());
    assert!(result.task_id.as_uuid() != uuid::Uuid::nil());
}
```

- [ ] **Step 8: Run both tests**

Run: `cargo test --package torque-harness-lite --test execution_tests -- --nocapture`
Expected: Both PASS

- [ ] **Step 9: Add test for agent definition registration**

```rust
#[tokio::test]
async fn test_agent_definition_registered() {
    let fake_llm = FakeLlm::single_text("Done");
    let llm = Arc::new(fake_llm);
    let model_driver = LiteModelDriver::new(llm);
    let tool_executor = LiteToolExecutor::new();

    let event_sink = Arc::new(InMemoryEventSink::default());
    let checkpoint_sink = Arc::new(InMemoryCheckpointSink::default());
    let agent_def = AgentDefinition::new("test-agent", "You are helpful.");
    let agent_def_id = agent_def.id;

    let mut host = RuntimeHost::new(vec![agent_def.clone()], event_sink.clone(), checkpoint_sink.clone());

    let request = ExecutionRequest::new(agent_def_id, "Do something", vec![]);
    let messages = vec![RuntimeMessage::new(RuntimeMessageRole::User, "Do something".into())];

    host.execute_v1(request, &model_driver, &tool_executor, None, messages)
        .await
        .unwrap();

    let stored_def = host.runtime().store().agent_definition(agent_def_id);
    assert!(stored_def.is_some(), "Agent definition should be registered in store");
}
```

- [ ] **Step 10: Run all execution tests**

Run: `cargo test --package torque-harness-lite --test execution_tests -- --nocapture`
Expected: All PASS

- [ ] **Step 11: Commit**

```bash
git add crates/torque-harness-lite/tests/execution_tests.rs
git commit -m "test(torque-harness-lite): add execution flow tests"
```

---

## Task 4: torque-harness-lite Tool Execution Tests

**Files:**
- Create: `crates/torque-harness-lite/tests/tool_execution_tests.rs`

> **Prerequisites:** Tasks 1, 2, 3 must be complete before starting this task.

- [ ] **Step 1: Add simple_tool_call to FakeLlm**

Modify: `crates/torque-harness-lite/tests/common/fake_llm.rs`

Add this method to the `FakeLlm` impl:

```rust
pub fn simple_tool_call(tool_name: &str, args: &[(&str, &str)]) -> Self {
    let mut args_map = serde_json::Map::new();
    for (k, v) in args {
        args_map.insert(k.to_string(), serde_json::json!(v));
    }

    let tool_call = ToolCall {
        id: "tool-call-1".to_string(),
        name: tool_name.to_string(),
        arguments: serde_json::Value::Object(args_map),
    };

    Self {
        model: "fake-model".to_string(),
        scripted: Mutex::new(VecDeque::from([ScriptedResponse {
            chunks: vec![Chunk::with_tool_call(tool_call)],
            finish_reason: FinishReason::ToolCalls,
            message_content: String::new(),
        }])),
        requests: Mutex::new(Vec::new()),
    }
}
```

- [ ] **Step 2: Write test for tool executor rejects unknown tools**

Create: `crates/torque-harness-lite/tests/tool_execution_tests.rs`

```rust
mod common;

use torque_runtime::tools::RuntimeToolResult;
use torque_runtime::environment::RuntimeExecutionContext;
use uuid::Uuid;

#[tokio::test]
async fn test_unknown_tool_returns_failure() {
    let executor = LiteToolExecutor::new();
    let ctx = RuntimeExecutionContext {
        instance_id: Uuid::new_v4(),
        request_id: None,
        source_task_id: None,
    };

    let result = executor.execute(ctx, "nonexistent_tool", serde_json::json!({})).await
        .expect("execute should return Ok");

    // Unknown tool should return a failure result (not success)
    assert!(!result.success, "Unknown tool should return failure, got: {:?}", result);
    assert!(result.error.is_some(), "Unknown tool should have error message");
}
```

- [ ] **Step 3: Run test to verify it passes**

Run: `cargo test --package torque-harness-lite --test tool_execution_tests -- --nocapture`
Expected: PASS

- [ ] **Step 4: Write test for ls tool listing files**

```rust
#[tokio::test]
async fn test_ls_tool_lists_files() {
    let executor = LiteToolExecutor::new();
    let ctx = RuntimeExecutionContext {
        instance_id: Uuid::new_v4(),
        request_id: None,
        source_task_id: None,
    };

    let result = executor.execute(ctx, "ls", serde_json::json!({"path": "/scratch"})).await.unwrap();

    assert!(result.success, "ls should succeed");
    // Result should contain some listing
}
```

- [ ] **Step 5: Run test**

Run: `cargo test --package torque-harness-lite --test tool_execution_tests -- --nocapture`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/torque-harness-lite/tests/tool_execution_tests.rs crates/torque-harness-lite/tests/common/fake_llm.rs
git commit -m "test(torque-harness-lite): add tool execution tests"
```

> **Note:** Tests for max_tool_call_limit and consecutive failures require a FakeLlm that returns repeated tool calls. These are deferred to a follow-up task that implements a `looping_tool_calls()` FakeLlm method.

---

## Task 5: torque-harness-lite Checkpoint Tests

**Files:**
- Create: `crates/torque-harness-lite/tests/checkpoint_tests.rs`

> **Prerequisites:** Tasks 1, 2, 3, 4 must be complete before starting this task (uses `simple_tool_call` from Task 4).

- [ ] **Step 1: Write test for checkpoint created on tool call**

Create: `crates/torque-harness-lite/tests/checkpoint_tests.rs`

> **Important:** Use `simple_tool_call` (defined in Task 4 Step 1) NOT `tool_call` which doesn't exist.

```rust
mod common;

use common::fake_llm::FakeLlm;
use torque_kernel::{AgentDefinition, ExecutionRequest};
use torque_runtime::host::RuntimeHost;
use torque_runtime::message::{RuntimeMessage, RuntimeMessageRole};

#[tokio::test]
async fn test_checkpoint_created_on_tool_call() {
    // Use simple_tool_call (from Task 4) to trigger tool call
    let fake_llm = FakeLlm::simple_tool_call("read_file", &[("path", "/test.txt")]);
    let llm = Arc::new(fake_llm);
    let model_driver = LiteModelDriver::new(llm);
    let tool_executor = LiteToolExecutor::new();

    let event_sink = Arc::new(InMemoryEventSink::default());
    let checkpoint_sink = Arc::new(InMemoryCheckpointSink::default());
    let agent_def = AgentDefinition::new("test-agent", "You are helpful.");
    let agent_def_id = agent_def.id;

    let mut host = RuntimeHost::new(vec![agent_def], event_sink.clone(), checkpoint_sink.clone());

    let request = ExecutionRequest::new(agent_def_id, "Read a file", vec![]);
    let messages = vec![RuntimeMessage::new(RuntimeMessageRole::User, "Read a file".into())];

    let result = host
        .execute_v1(request, &model_driver, &tool_executor, None, messages)
        .await
        .unwrap();

    // After tool call, checkpoint should be created
    assert!(checkpoint_sink.save_count() >= 1, "Should create checkpoint after tool call");
}
```

- [ ] **Step 2: Run test**

Run: `cargo test --package torque-harness-lite --test checkpoint_tests -- --nocapture`
Expected: PASS

- [ ] **Step 3: Write test for checkpoint payload structure**

```rust
#[tokio::test]
async fn test_checkpoint_payload_structure() {
    let fake_llm = FakeLlm::simple_tool_call("ls", &[("path", "/scratch")]);
    let llm = Arc::new(fake_llm);
    let model_driver = LiteModelDriver::new(llm);
    let tool_executor = LiteToolExecutor::new();

    let event_sink = Arc::new(InMemoryEventSink::default());
    let checkpoint_sink = Arc::new(InMemoryCheckpointSink::default());
    let agent_def = AgentDefinition::new("test-agent", "You are helpful.");
    let agent_def_id = agent_def.id;

    let mut host = RuntimeHost::new(vec![agent_def], event_sink.clone(), checkpoint_sink.clone());

    let request = ExecutionRequest::new(agent_def_id, "List files", vec![]);
    let messages = vec![RuntimeMessage::new(RuntimeMessageRole::User, "List files".into())];

    let _ = host
        .execute_v1(request, &model_driver, &tool_executor, None, messages)
        .await
        .unwrap();

    // Verify checkpoint payloads exist and have required structure
    let payloads = &*checkpoint_sink.payloads.lock().unwrap();
    assert!(!payloads.is_empty(), "Should have at least one checkpoint");

    let payload = &payloads[0];
    assert!(payload.instance_id.as_uuid() != uuid::Uuid::nil());
    assert!(payload.reason.len() > 0);
    assert!(payload.state.is_object());
}
```

- [ ] **Step 4: Run test**

Run: `cargo test --package torque-harness-lite --test checkpoint_tests -- --nocapture`
Expected: PASS

- [ ] **Step 5: Add test for checkpoint on completion**

```rust
#[tokio::test]
async fn test_checkpoint_created_on_completion() {
    let fake_llm = FakeLlm::single_text("Task completed successfully");
    let llm = Arc::new(fake_llm);
    let model_driver = LiteModelDriver::new(llm);
    let tool_executor = LiteToolExecutor::new();

    let event_sink = Arc::new(InMemoryEventSink::default());
    let checkpoint_sink = Arc::new(InMemoryCheckpointSink::default());
    let agent_def = AgentDefinition::new("test-agent", "You are helpful.");
    let agent_def_id = agent_def.id;

    let mut host = RuntimeHost::new(vec![agent_def], event_sink.clone(), checkpoint_sink.clone());

    let request = ExecutionRequest::new(agent_def_id, "Do it", vec![]);
    let messages = vec![RuntimeMessage::new(RuntimeMessageRole::User, "Do it".into())];

    let _ = host
        .execute_v1(request, &model_driver, &tool_executor, None, messages)
        .await
        .unwrap();

    // checkpoint_on_task_complete should create a checkpoint
    let count = checkpoint_sink.save_count();
    assert!(count >= 1, "Should create checkpoint on task complete, got {}", count);
}
```

- [ ] **Step 6: Run all checkpoint tests**

Run: `cargo test --package torque-harness-lite --test checkpoint_tests -- --nocapture`
Expected: All PASS

- [ ] **Step 7: Commit**

```bash
git add crates/torque-harness-lite/tests/checkpoint_tests.rs
git commit -m "test(torque-harness-lite): add checkpoint tests"
```

---

## Task 6: Create API Model Tests Infrastructure

**Files:**
- Create: `crates/torque-harness/tests/api/common/mod.rs`
- Create: `crates/torque-harness/tests/api/common/helpers.rs`

> **Scope Note:** This task focuses on API model serialization/deserialization tests, NOT full HTTP endpoint tests. Full HTTP integration tests require database wiring and are deferred to a later phase.

- [ ] **Step 1: Create helpers.rs**

Create: `crates/torque-harness/tests/api/common/helpers.rs`

```rust
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListQuery {
    pub limit: Option<usize>,
    pub cursor: Option<String>,
    pub sort: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pagination {
    pub next_cursor: Option<String>,
    pub prev_cursor: Option<String>,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListResponse<T> {
    pub data: Vec<T>,
    pub pagination: Pagination,
}
```

- [ ] **Step 2: Create common/mod.rs**

Create: `crates/torque-harness/tests/api/common/mod.rs`

```rust
pub mod helpers;

pub use helpers::*;
```

- [ ] **Step 3: Verify compilation**

Run: `cargo build --package torque-harness --tests`
Expected: SUCCESS

- [ ] **Step 4: Commit**

```bash
git add crates/torque-harness/tests/api/common/mod.rs crates/torque-harness/tests/api/common/helpers.rs
git commit -m "test(api): add API test helpers"
```

---

## Task 7: Agent Definitions Model Tests

**Files:**
- Create: `crates/torque-harness/tests/api/agent_definitions_tests.rs`

> **Scope:** Model serialization/deserialization tests only. HTTP endpoint tests require database wiring and are deferred.

- [ ] **Step 1: Write model serialization tests**

Create: `crates/torque-harness/tests/api/agent_definitions_tests.rs`

```rust
mod common;

#[tokio::test]
async fn test_agent_definition_create_serialization() {
    use torque_harness::models::v1::agent_definition::AgentDefinitionCreate;

    let create = AgentDefinitionCreate {
        name: "Test Agent".into(),
        description: Some("A test agent".into()),
        system_prompt: Some("You are helpful.".into()),
        tool_policy: serde_json::json!({"allowed_tools": ["read_file"]}),
        memory_policy: serde_json::json!({"recall_enabled": true}),
        delegation_policy: serde_json::json!({"max_depth": 2}),
        limits: serde_json::json!({"max_turns": 20}),
        default_model_policy: serde_json::json!({"model": "gpt-4"}),
    };

    // Verify serialization
    let json = serde_json::to_value(&create).unwrap();
    assert_eq!(json["name"], "Test Agent");
    assert_eq!(json["description"], "A test agent");
    assert_eq!(json["system_prompt"], "You are helpful.");
    assert_eq!(json["tool_policy"]["allowed_tools"][0], "read_file");

    // Verify deserialization
    let deserialized: AgentDefinitionCreate = serde_json::from_value(json).unwrap();
    assert_eq!(deserialized.name, "Test Agent");
}

#[tokio::test]
async fn test_agent_definition_list_response() {
    use torque_harness::models::v1::common::{ListResponse, Pagination};

    let response = ListResponse::<serde_json::Value> {
        data: vec![],
        pagination: Pagination {
            next_cursor: None,
            prev_cursor: None,
            has_more: false,
        },
    };

    let json = serde_json::to_value(&response).unwrap();
    assert!(json["data"].is_array());
    assert!(!json["pagination"]["has_more"].as_bool().unwrap());
}

#[tokio::test]
async fn test_agent_definition_create_minimal_fields() {
    use torque_harness::models::v1::agent_definition::AgentDefinitionCreate;

    // Minimal create - only required fields
    let minimal = AgentDefinitionCreate {
        name: "Minimal Agent".into(),
        description: None,
        system_prompt: None,
        tool_policy: serde_json::json!({}),
        memory_policy: serde_json::json!({}),
        delegation_policy: serde_json::json!({}),
        limits: serde_json::json!({}),
        default_model_policy: serde_json::json!({}),
    };

    let json = serde_json::to_value(&minimal).unwrap();
    assert_eq!(json["name"], "Minimal Agent");
    assert!(json["description"].is_null());
    assert!(json["system_prompt"].is_null());
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --package torque-harness --test agent_definitions_tests -- --nocapture`
Expected: All PASS

- [ ] **Step 3: Commit**

```bash
git add crates/torque-harness/tests/api/agent_definitions_tests.rs
git commit -m "test(api): add agent definitions model tests"
```

---

## Task 8: Agent Instances Model Tests

**Files:**
- Create: `crates/torque-harness/tests/api/agent_instances_tests.rs`

> **Scope:** Model serialization/deserialization tests only.

- [ ] **Step 1: Write model serialization tests**

Create: `crates/torque-harness/tests/api/agent_instances_tests.rs`

```rust
mod common;

#[tokio::test]
async fn test_agent_instance_create_serialization() {
    use torque_harness::models::v1::agent_instance::AgentInstanceCreate;

    let create = AgentInstanceCreate {
        agent_definition_id: uuid::Uuid::new_v4(),
        external_context_refs: vec![],
    };

    let json = serde_json::to_value(&create).unwrap();
    assert!(json["agent_definition_id"].is_string());

    let deserialized: AgentInstanceCreate = serde_json::from_value(json).unwrap();
    assert!(deserialized.external_context_refs.is_empty());
}

#[tokio::test]
async fn test_agent_instance_with_external_context() {
    use torque_harness::models::v1::agent_instance::AgentInstanceCreate;
    use torque_kernel::{ExternalContextRef, ExternalContextKind, AccessMode, SyncPolicy};

    let create = AgentInstanceCreate {
        agent_definition_id: uuid::Uuid::new_v4(),
        external_context_refs: vec![
            ExternalContextRef {
                id: torque_kernel::ExternalContextRefId::new(),
                kind: ExternalContextKind::Repository,
                locator: "github.com/test/repo".into(),
                access_mode: AccessMode::ReadOnly,
                sync_policy: SyncPolicy::LazyFetch,
                metadata: vec![],
            }
        ],
    };

    let json = serde_json::to_value(&create).unwrap();
    assert_eq!(json["external_context_refs"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_agent_instance_status_serialization() {
    use torque_harness::models::v1::agent_instance::AgentInstanceStatus;

    let statuses = vec![
        (AgentInstanceStatus::Created, "CREATED"),
        (AgentInstanceStatus::Running, "RUNNING"),
        (AgentInstanceStatus::Completed, "COMPLETED"),
    ];

    for (status, expected_str) in statuses {
        let json = serde_json::to_value(&status).unwrap();
        let json_str = json.as_str().unwrap();
        assert!(json_str.contains(expected_str), "Expected {} in {}", json_str, expected_str);

        let deserialized: AgentInstanceStatus = serde_json::from_value(json).unwrap();
        assert!(matches!(deserialized, s if std::mem::discriminant(&s) == std::mem::discriminant(&status)));
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --package torque-harness --test agent_instances_tests -- --nocapture`
Expected: All PASS

- [ ] **Step 3: Commit**

```bash
git add crates/torque-harness/tests/api/agent_instances_tests.rs
git commit -m "test(api): add agent instances model tests"
```

---

## Task 9: Runs Model Tests

**Files:**
- Create: `crates/torque-harness/tests/api/runs_tests.rs`

> **Scope:** Model serialization/deserialization tests only.

- [ ] **Step 1: Write model serialization tests**

Create: `crates/torque-harness/tests/api/runs_tests.rs`

```rust
mod common;

#[tokio::test]
async fn test_run_request_serialization() {
    use torque_harness::models::v1::run::RunRequest;

    let request = RunRequest {
        goal: "Build a REST API".to_string(),
        instructions: Some("Use Rust and Axum".to_string()),
        input_artifacts: Some(vec!["artifact-1".to_string()]),
        external_context_refs: None,
        constraints: Some(vec!["keep it simple".to_string()]),
        expected_outputs: Some(vec!["api_design".to_string()]),
        execution_mode: Some("interactive".to_string()),
        agent_instance_id: Some(uuid::Uuid::new_v4()),
        webhook_url: Some("https://example.com/webhook".to_string()),
        idempotency_key: Some("run-001".to_string()),
        async_execution: false,
    };

    let json = serde_json::to_value(&request).unwrap();
    assert_eq!(json["goal"], "Build a REST API");
    assert_eq!(json["instructions"], "Use Rust and Axum");
    assert!(json["input_artifacts"].is_array());
    assert!(json["async_execution"].as_bool().unwrap() == false);

    // Verify roundtrip
    let deserialized: RunRequest = serde_json::from_value(json).unwrap();
    assert_eq!(deserialized.goal, "Build a REST API");
}

#[tokio::test]
async fn test_run_request_async_validation_logic() {
    use torque_harness::models::v1::run::RunRequest;

    // Async execution without instance_id should be flagged as invalid
    // (actual validation happens in handler, this tests the data structure)
    let async_request = RunRequest {
        goal: "Test".to_string(),
        instructions: None,
        input_artifacts: None,
        external_context_refs: None,
        constraints: None,
        expected_outputs: None,
        execution_mode: None,
        agent_instance_id: None,  // Missing for async
        webhook_url: None,
        idempotency_key: None,
        async_execution: true,
    };

    // The handler should reject this combination
    let should_be_rejected = async_request.async_execution && async_request.agent_instance_id.is_none();
    assert!(should_be_rejected, "async_execution=true without agent_instance_id should be flagged");
}

#[tokio::test]
async fn test_run_status_serialization() {
    use torque_harness::models::v1::run::RunStatus;

    let statuses = vec![
        (RunStatus::Queued, "QUEUED"),
        (RunStatus::Running, "RUNNING"),
        (RunStatus::Completed, "COMPLETED"),
        (RunStatus::Failed, "FAILED"),
        (RunStatus::Cancelled, "CANCELLED"),
    ];

    for (status, expected) in statuses {
        let json = serde_json::to_value(&status).unwrap();
        let json_str = json.as_str().unwrap();
        assert!(json_str.contains(expected), "Expected {} in {}", json_str, expected);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --package torque-harness --test runs_tests -- --nocapture`
Expected: All PASS

- [ ] **Step 3: Commit**

```bash
git add crates/torque-harness/tests/api/runs_tests.rs
git commit -m "test(api): add runs model tests"
```

---

## Task 10: Integration Test - Full Execution Flow

**Files:**
- Create: `crates/torque-harness/tests/integration/full_execution_tests.rs`

> **Prerequisites:** Tasks 1, 2, 3 must be complete. This task uses FakeLlm from torque-harness tests.

- [ ] **Step 1: Create complete integration test**

Create: `crates/torque-harness/tests/integration/full_execution_tests.rs`

```rust
mod common;

#[tokio::test]
async fn test_execution_flow_with_fake_llm() {
    use torque_harness::tests::common::fake_llm::FakeLlm;
    use torque_kernel::{AgentDefinition, ExecutionRequest};
    use torque_runtime::host::RuntimeHost;
    use torque_runtime::message::{RuntimeMessage, RuntimeMessageRole};
    use torque_runtime::environment::{RuntimeModelDriver, RuntimeToolExecutor, RuntimeEventSink, RuntimeCheckpointSink};
    use torque_runtime::tools::RuntimeToolDef;
    use torque_runtime::events::ModelTurnResult;
    use torque_runtime::checkpoint::{RuntimeCheckpointPayload, RuntimeCheckpointRef};
    use async_trait::async_trait;
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    // Define FakeModelDriver inline
    struct FakeModelDriver {
        response: String,
    }

    #[async_trait]
    impl RuntimeModelDriver for FakeModelDriver {
        async fn run_turn(
            &self,
            _messages: Vec<RuntimeMessage>,
            _tools: Vec<RuntimeToolDef>,
            _sink: Option<&dyn torque_runtime::environment::RuntimeOutputSink>,
        ) -> anyhow::Result<ModelTurnResult> {
            Ok(ModelTurnResult {
                finish_reason: torque_runtime::events::RuntimeFinishReason::Stop,
                assistant_text: self.response.clone(),
                tool_calls: vec![],
            })
        }
    }

    // Define minimal FakeToolExecutor inline
    struct FakeToolExecutor;

    #[async_trait]
    impl RuntimeToolExecutor for FakeToolExecutor {
        async fn execute(
            &self,
            _ctx: torque_runtime::environment::RuntimeExecutionContext,
            _tool_name: &str,
            _arguments: serde_json::Value,
        ) -> anyhow::Result<torque_runtime::tools::RuntimeToolResult> {
            Ok(torque_runtime::tools::RuntimeToolResult::success("ok"))
        }

        async fn tool_defs(&self) -> anyhow::Result<Vec<RuntimeToolDef>> {
            Ok(vec![])
        }
    }

    // Define minimal test sinks
    struct TestEventSink(Arc<Mutex<Vec<torque_kernel::ExecutionResult>>>);

    impl Default for TestEventSink {
        fn default() -> Self { Self(Arc::new(Mutex::new(vec![]))) }
    }

    #[async_trait]
    impl RuntimeEventSink for TestEventSink {
        async fn record_execution_result(&self, result: &torque_kernel::ExecutionResult) -> anyhow::Result<()> {
            self.0.lock().unwrap().push(result.clone());
            Ok(())
        }

        async fn record_checkpoint_created(
            &self,
            _checkpoint_id: Uuid,
            _instance_id: torque_kernel::AgentInstanceId,
            _reason: &str,
        ) -> anyhow::Result<()> {
            Ok(())
        }
    }

    struct TestCheckpointSink(Arc<Mutex<Vec<RuntimeCheckpointPayload>>>, Arc<Mutex<usize>>);

    impl Default for TestCheckpointSink {
        fn default() -> Self { Self(Arc::new(Mutex::new(vec![])), Arc::new(Mutex::new(0))) }
    }

    #[async_trait]
    impl RuntimeCheckpointSink for TestCheckpointSink {
        async fn save(&self, payload: RuntimeCheckpointPayload) -> anyhow::Result<RuntimeCheckpointRef> {
            *self.1.lock().unwrap() += 1;
            self.0.lock().unwrap().push(payload);
            Ok(RuntimeCheckpointRef {
                checkpoint_id: Uuid::new_v4(),
                instance_id: Uuid::new_v4(),
            })
        }
    }

    // Setup
    let model_driver = FakeModelDriver { response: "Task completed".to_string() };
    let tool_executor = FakeToolExecutor;

    let event_sink = Arc::new(TestEventSink::default()) as Arc<dyn RuntimeEventSink>;
    let checkpoint_sink = Arc::new(TestCheckpointSink::default()) as Arc<dyn RuntimeCheckpointSink>;
    let agent_def = AgentDefinition::new("test-agent", "You are helpful.");
    let agent_def_id = agent_def.id;

    let mut host = RuntimeHost::new(vec![agent_def], event_sink.clone(), checkpoint_sink.clone());

    // Execute
    let request = ExecutionRequest::new(agent_def_id, "Do something", vec![]);
    let messages = vec![RuntimeMessage::new(RuntimeMessageRole::User, "Do something".into())];

    let result = host
        .execute_v1(request, &model_driver, &tool_executor, None, messages)
        .await;

    // Verify
    assert!(result.is_ok(), "Execution should succeed: {:?}", result.err());
    let result = result.unwrap();
    assert!(result.summary.is_some(), "Result should have summary");

    // Verify event sink recorded results
    let events = event_sink.0.lock().unwrap();
    assert!(!events.is_empty(), "Should have recorded execution results");
}
```

- [ ] **Step 2: Run test**

Run: `cargo test --package torque-harness --test full_execution_tests -- --nocapture`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/torque-harness/tests/integration/full_execution_tests.rs
git commit -m "test(integration): add full execution flow test"
```

---

## Summary

After completing all tasks:

| Metric | Before | After |
|--------|--------|-------|
| torque-harness-lite tests | 0 | ~10 |
| API model tests | 0 | ~12 |
| Integration tests | ~5 | ~6 |
| FakeLlm capabilities | 3 methods | 7 methods |
| Test infrastructure | minimal | structured |

**Total new test functions:** ~30
**Total files created/modified:** ~12
**Estimated time to implement:** 2-3 days

---

## Dependencies Between Tasks

```
Task 1 (FakeLlm) → Task 2 (Test Infra) → Task 3 (Execution Tests)
                                              ↓
Task 4 (Tool Tests) ←←←←←←←←←←←←←←←←←←←←←←←←←←
Task 5 (Checkpoint Tests) depends on Task 2 & 3 & 4

Task 6 (API Infra) → Task 7 (Agent Def Tests)
                    → Task 8 (Agent Inst Tests)
                    → Task 9 (Runs Tests)

Task 10 (Integration) depends on Task 1, 2, 3
```

**Critical path:** Task 1 → Task 2 → Task 3 → Task 10

Tasks 4, 5, 7, 8, 9 can be done in parallel after their prerequisites.
