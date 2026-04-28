# LLM Crate Bug Fixes and Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix critical bugs (streaming TokenUsage zeros, missing tool_calls, missing 401/429 streaming handling), remove dead code, and harden the llm crate for production use.

**Architecture:** All changes are within `crates/llm/`. The `streaming.rs` module (unused, duplicative) is deleted. `Message` gains an optional `tool_calls` field. `OpenAiClient::chat_streaming()` gains proper error handling, usage parsing, timeout, and tracing. Mockito integration tests verify the fixes. One downstream fix in `torque-runtime/src/message.rs` for the new `Message` field.

**Tech Stack:** Rust, reqwest 0.12, serde/serde_json, mockito 1 (async), tracing, thiserror, tokio.

---

## File Map

| Action | File | Purpose |
|--------|------|---------|
| Delete | `crates/llm/src/streaming.rs` | Unused module, duplicative of openai.rs SSE parser |
| Modify | `crates/llm/Cargo.toml` | Remove unused deps `anyhow`, `tokio-util` |
| Modify | `crates/llm/src/tools.rs` | Remove unused `Function`, `StreamingChunk` structs |
| Modify | `crates/llm/src/lib.rs` | Remove `streaming` module, update re-exports |
| Modify | `crates/llm/src/client.rs` | Add `tool_calls: Option<Vec<ToolCall>>` to `Message` |
| Modify | `crates/llm/src/openai.rs` | Populate tool_calls in `chat()`, fix streaming 401/429, fix streaming TokenUsage, add timeout, add tracing |
| Modify | `crates/llm/tests/client_tests.rs` | Add `Message.tool_calls` serialization tests |
| Create | `crates/llm/tests/openai_integration_tests.rs` | Mockito integration tests for streaming error handling, TokenUsage, tool call accumulation |
| Modify | `crates/torque-runtime/src/message.rs` | Add `tool_calls: None` to `LlmMessage` struct construction (downstream fix) |

---

### Task 1: Remove Dead Code

**Files:**
- Delete: `crates/llm/src/streaming.rs`
- Modify: `crates/llm/Cargo.toml`
- Modify: `crates/llm/src/tools.rs`
- Modify: `crates/llm/src/lib.rs`

- [ ] **Step 1: Delete `crates/llm/src/streaming.rs`**

Run: `rm crates/llm/src/streaming.rs`

- [ ] **Step 2: Remove `Function` and `StreamingChunk` from `crates/llm/src/tools.rs`**

Remove lines 3-8 and 45-50. After removal, the file should contain only `ToolDef` and `ToolCall`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

impl ToolDef {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters: serde_json::Value::Object(Default::default()),
        }
    }

    pub fn with_parameters(mut self, parameters: serde_json::Value) -> Self {
        self.parameters = parameters;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

impl ToolCall {
    pub fn arguments_as<T: for<'de> Deserialize<'de>>(&self) -> Option<T> {
        serde_json::from_value(self.arguments.clone()).ok()
    }
}
```

- [ ] **Step 3: Remove `streaming` module from `crates/llm/src/lib.rs`**

Change:
```rust
pub mod client;
pub mod error;
pub mod openai;
pub mod streaming;
pub mod tools;

pub use client::{ChatRequest, ChatResponse, Chunk, FinishReason, LlmClient, Message, TokenUsage};
pub use error::{LlmError, Result};
pub use openai::OpenAiClient;
pub use tools::{ToolCall, ToolDef};
```

To:
```rust
pub mod client;
pub mod error;
pub mod openai;
pub mod tools;

pub use client::{ChatRequest, ChatResponse, Chunk, FinishReason, LlmClient, Message, TokenUsage};
pub use error::{LlmError, Result};
pub use openai::OpenAiClient;
pub use tools::{ToolCall, ToolDef};
```

- [ ] **Step 4: Remove unused dependencies from `crates/llm/Cargo.toml`**

Remove lines 12 (`anyhow = "1"`) and 14 (`tokio-util = { version = "0.7", features = ["io"] }`).

Final `[dependencies]`:
```toml
[dependencies]
async-trait = "0.1"
reqwest = { version = "0.12", features = ["json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"
tracing = "0.1"
```

- [ ] **Step 5: Verify compilation**

Run: `cargo check -p llm`
Expected: PASS (compiles without streaming module or removed deps)

- [ ] **Step 6: Run existing tests**

Run: `cargo test -p llm`
Expected: All tests PASS

- [ ] **Step 7: Commit**

```bash
git add crates/llm/
git commit -m "chore(llm): remove dead code and unused dependencies"
```

---

### Task 2: Add `tool_calls` to `Message`

**Files:**
- Modify: `crates/llm/src/client.rs`
- Modify: `crates/llm/src/openai.rs`
- Modify: `crates/llm/tests/client_tests.rs`

- [ ] **Step 1: Write test for Message.tool_calls serialization**

Add to `crates/llm/tests/client_tests.rs`:

```rust
#[test]
fn test_message_with_tool_calls_serialization() {
    let tool_call = llm::ToolCall {
        id: "call_1".to_string(),
        name: "test_func".to_string(),
        arguments: serde_json::json!({"key": "value"}),
    };
    let msg = llm::Message {
        role: "assistant".to_string(),
        content: String::new(),
        tool_calls: Some(vec![tool_call]),
    };

    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains("\"tool_calls\""));
    assert!(json.contains("\"call_1\""));
    assert!(json.contains("\"test_func\""));
}

#[test]
fn test_message_without_tool_calls_omitted() {
    let msg = llm::Message {
        role: "user".to_string(),
        content: "Hello".to_string(),
        tool_calls: None,
    };

    let json = serde_json::to_string(&msg).unwrap();
    assert!(!json.contains("tool_calls"));
}

#[test]
fn test_message_tool_calls_deserialization() {
    let json = r#"{"role":"assistant","content":"","tool_calls":[{"id":"call_1","name":"test_func","arguments":{"key":"value"}}]}"#;
    let msg: llm::Message = serde_json::from_str(json).unwrap();
    assert_eq!(msg.role, "assistant");
    assert!(msg.tool_calls.is_some());
    let calls = msg.tool_calls.unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].id, "call_1");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p llm test_message_with_tool_calls_serialization`
Expected: compilation ERROR (Message has no `tool_calls` field)

- [ ] **Step 3: Add `tool_calls` field to `Message`**

Modify `crates/llm/src/client.rs`, change the `Message` struct (lines 4-8) to:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<super::tools::ToolCall>>,
}
```

Update constructor helpers to include `tool_calls: None`:

```rust
impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".into(),
            content: content.into(),
            tool_calls: None,
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
            tool_calls: None,
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".into(),
            content: content.into(),
            tool_calls: None,
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p llm -- test_message_with_tool_calls test_message_tool_calls`
Expected: 3 PASS

- [ ] **Step 5: Commit**

```bash
git add crates/llm/src/client.rs crates/llm/tests/client_tests.rs
git commit -m "feat(llm): add tool_calls field to Message"
```

---

### Task 3: Populate `tool_calls` from non-streaming `chat()` Response

**Files:**
- Modify: `crates/llm/src/openai.rs`
- Create: `crates/llm/tests/openai_integration_tests.rs`

- [ ] **Step 1: Write test for chat() returning tool calls**

Create `crates/llm/tests/openai_integration_tests.rs` with:

```rust
use llm::{ChatRequest, FinishReason, LlmClient, Message, OpenAiClient};

#[tokio::test]
async fn test_chat_returns_tool_calls() {
    let mut server = mockito::Server::new_async().await;

    let mock = server
        .mock("POST", "/chat/completions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_abc123",
                        "type": "function",
                        "function": {
                            "name": "get_weather",
                            "arguments": "{\"location\":\"NYC\"}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
        }"#)
        .create_async()
        .await;

    let client = OpenAiClient::new(
        server.url(),
        "test-key".to_string(),
        "gpt-4".to_string(),
    );

    let request = ChatRequest::new("gpt-4", vec![Message::user("weather in NYC?")]);
    let response = client.chat(request).await.unwrap();

    assert_eq!(response.finish_reason, FinishReason::ToolCalls);
    assert!(response.message.tool_calls.is_some());
    let tool_calls = response.message.tool_calls.unwrap();
    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls[0].id, "call_abc123");
    assert_eq!(tool_calls[0].name, "get_weather");
    assert_eq!(tool_calls[0].arguments, serde_json::json!({"location": "NYC"}));

    mock.assert_async().await;
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p llm -- test_chat_returns_tool_calls`
Expected: FAIL (tool_calls in response are parsed but discarded)

- [ ] **Step 3: Fix `chat()` to populate `tool_calls` on the returned `Message`**

In `crates/llm/src/openai.rs`, replace lines 165-171:

```rust
        let (message, finish_reason_str) = if let Some(choice) = body.choices.into_iter().next() {
            let content = choice.message.content.unwrap_or_default();
            let reason = choice.finish_reason;
            let msg = Message {
                role: choice.message.role,
                content,
            };
            (msg, reason)
```

With:

```rust
        let (message, finish_reason_str) = if let Some(choice) = body.choices.into_iter().next() {
            let content = choice.message.content.unwrap_or_default();
            let reason = choice.finish_reason;
            let tool_calls = choice.message.tool_calls.map(|calls| {
                calls
                    .into_iter()
                    .map(|tc| ToolCall {
                        id: tc.id,
                        name: tc.function.name,
                        arguments: serde_json::from_str(&tc.function.arguments)
                            .unwrap_or(serde_json::Value::Object(Default::default())),
                    })
                    .collect()
            });
            let msg = Message {
                role: choice.message.role,
                content,
                tool_calls,
            };
            (msg, reason)
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p llm -- test_chat_returns_tool_calls`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/llm/src/openai.rs crates/llm/tests/openai_integration_tests.rs
git commit -m "fix(llm): populate tool_calls from non-streaming chat response"
```

---

### Task 4: Fix Streaming 401/429 Error Handling

**Files:**
- Modify: `crates/llm/src/openai.rs`
- Modify: `crates/llm/tests/openai_integration_tests.rs`

- [ ] **Step 1: Write mockito test for 401 in streaming**

Add to `crates/llm/tests/openai_integration_tests.rs`:

```rust
#[tokio::test]
async fn test_chat_streaming_returns_authentication_failed_on_401() {
    let mut server = mockito::Server::new_async().await;

    let mock = server
        .mock("POST", "/chat/completions")
        .with_status(401)
        .with_body(r#"{"error": {"message": "Invalid API key"}}"#)
        .create_async()
        .await;

    let client = OpenAiClient::new(
        server.url(),
        "bad-key".to_string(),
        "gpt-4".to_string(),
    );

    let request = ChatRequest::new("gpt-4", vec![Message::user("Hello")]);
    let result = client.chat_streaming(request, Box::new(|_| {})).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        llm::LlmError::AuthenticationFailed => {}
        other => panic!("Expected AuthenticationFailed, got {:?}", other),
    }

    mock.assert_async().await;
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p llm -- test_chat_streaming_returns_authentication_failed_on_401`
Expected: FAIL (currently returns InvalidResponse, not AuthenticationFailed)

- [ ] **Step 3: Write mockito test for 429 in streaming**

Add to same file:

```rust
#[tokio::test]
async fn test_chat_streaming_returns_rate_limit_on_429() {
    let mut server = mockito::Server::new_async().await;

    let mock = server
        .mock("POST", "/chat/completions")
        .with_status(429)
        .with_body(r#"{"error": {"message": "Too many requests"}}"#)
        .create_async()
        .await;

    let client = OpenAiClient::new(
        server.url(),
        "test-key".to_string(),
        "gpt-4".to_string(),
    );

    let request = ChatRequest::new("gpt-4", vec![Message::user("Hello")]);
    let result = client.chat_streaming(request, Box::new(|_| {})).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        llm::LlmError::RateLimitExceeded => {}
        other => panic!("Expected RateLimitExceeded, got {:?}", other),
    }

    mock.assert_async().await;
}
```

- [ ] **Step 4: Add 401/429 checks to `chat_streaming()`**

In `crates/llm/src/openai.rs`, replace lines 212-220:

```rust
        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(LlmError::InvalidResponse(format!(
                "Status {}: {}",
                status.as_u16(),
                error_text
            )));
        }
```

With:

```rust
        let status = response.status();

        if status.as_u16() == 401 {
            return Err(LlmError::AuthenticationFailed);
        }

        if status.as_u16() == 429 {
            return Err(LlmError::RateLimitExceeded);
        }

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(LlmError::InvalidResponse(format!(
                "Status {}: {}",
                status.as_u16(),
                error_text
            )));
        }
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p llm -- test_chat_streaming_returns_authentication_failed test_chat_streaming_returns_rate_limit`
Expected: 2 PASS

- [ ] **Step 6: Commit**

```bash
git add crates/llm/src/openai.rs crates/llm/tests/openai_integration_tests.rs
git commit -m "fix(llm): add 401/429 error handling to chat_streaming"
```

---

### Task 5: Fix Streaming TokenUsage (Zero Tokens)

**Files:**
- Modify: `crates/llm/src/openai.rs`
- Modify: `crates/llm/tests/openai_integration_tests.rs`

- [ ] **Step 1: Write mockito test for streaming TokenUsage**

Add to `crates/llm/tests/openai_integration_tests.rs`:

```rust
#[tokio::test]
async fn test_chat_streaming_returns_token_usage() {
    let mut server = mockito::Server::new_async().await;

    let sse_body = concat!(
        "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello\"}}]}\n\n",
        "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":5,\"total_tokens\":15}}\n\n",
        "data: [DONE]\n\n"
    );

    let mock = server
        .mock("POST", "/chat/completions")
        .with_status(200)
        .with_header("content-type", "text/event-stream")
        .with_body(sse_body)
        .create_async()
        .await;

    let client = OpenAiClient::new(
        server.url(),
        "test-key".to_string(),
        "gpt-4".to_string(),
    );

    let request = ChatRequest::new("gpt-4", vec![Message::user("Hello")]);
    let response = client
        .chat_streaming(request, Box::new(|_| {}))
        .await
        .unwrap();

    assert_eq!(response.usage.prompt_tokens, 10);
    assert_eq!(response.usage.completion_tokens, 5);
    assert_eq!(response.usage.total_tokens, 15);

    mock.assert_async().await;
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p llm -- test_chat_streaming_returns_token_usage`
Expected: FAIL (usage returns all zeros 0, 0, 0 instead of 10, 5, 15)

- [ ] **Step 3: Fix `build_request()` to serialize the `stream` field**

The `ChatRequest` struct has a `stream: Option<bool>` field, but `build_request()` (lines 42-73) never includes it in the request JSON body. This means `chat_streaming()` sets `request.stream = Some(true)` but the server never receives it. Add `stream` serialization to `build_request()` after the `temperature` block (after line 70):

```rust
        if let Some(stream) = request.stream {
            body["stream"] = stream.into();
        }
```

The final `build_request()` method body:

```rust
        let mut body = serde_json::json!({
            "model": request.model,
            "messages": request.messages,
        });

        if let Some(tools) = request.tools {
            body["tools"] = serde_json::json!(tools
                .into_iter()
                .map(|t| {
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.parameters,
                        }
                    })
                })
                .collect::<Vec<_>>());
        }

        if let Some(max_tokens) = request.max_tokens {
            body["max_tokens"] = max_tokens.into();
        }

        if let Some(temperature) = request.temperature {
            body["temperature"] = temperature.into();
        }

        if let Some(stream) = request.stream {
            body["stream"] = stream.into();
        }

        body
```

- [ ] **Step 4: Add `stream_options` to streaming request and parse usage from SSE**

In `crates/llm/src/openai.rs`, after the `build_request` call (line 201), add `stream_options`:

Change:
```rust
        let url = format!("{}/chat/completions", self.base_url);
        let body = self.build_request(request);
```

To:
```rust
        let url = format!("{}/chat/completions", self.base_url);
        let mut body = self.build_request(request);
        body["stream_options"] = serde_json::json!({"include_usage": true});
```

Update the `SSEChunk` struct (around line 250) to include an optional `usage` field:

```rust
                #[derive(Deserialize)]
                struct SSEChunk {
                    choices: Vec<SSEChoice>,
                    #[serde(default)]
                    usage: Option<SSEUsage>,
                }

                #[derive(Deserialize, Default)]
                struct SSEUsage {
                    #[serde(rename = "prompt_tokens")]
                    prompt_tokens: i64,
                    #[serde(rename = "completion_tokens")]
                    completion_tokens: i64,
                    #[serde(rename = "total_tokens")]
                    total_tokens: i64,
                }
```

Add a variable to track the last seen usage, initialized alongside `full_content` and `finish_reason` (after line 239):

```rust
        let mut usage = TokenUsage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
        };
```

Inside the SSE chunk parsing loop, after `Ok(chunk) => {`, add usage accumulation before the choices processing:

```rust
                    Ok(chunk) => {
                        if let Some(ref sse_usage) = chunk.usage {
                            usage = TokenUsage {
                                prompt_tokens: sse_usage.prompt_tokens,
                                completion_tokens: sse_usage.completion_tokens,
                                total_tokens: sse_usage.total_tokens,
                            };
                        }
                        if let Some(choice) = chunk.choices.into_iter().next() {
                            // ... existing content/tool_calls/finish_reason handling
```

Replace the hardcoded zero usage at the end (around line 338):

```rust
            usage: TokenUsage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            },
```

With:

```rust
            usage,
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p llm -- test_chat_streaming_returns_token_usage`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/llm/src/openai.rs crates/llm/tests/openai_integration_tests.rs
git commit -m "fix(llm): return real TokenUsage from chat_streaming via stream_options"
```

---

### Task 6: Robustness Hardening

**Files:**
- Modify: `crates/llm/src/openai.rs`

- [ ] **Step 1: Add reqwest timeout**

In `crates/llm/src/openai.rs`, update the `OpenAiClient::new()` constructor:

```rust
    pub fn new(base_url: String, api_key: String, default_model: String) -> Self {
        let http_client = Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .expect("failed to build reqwest client");
        Self {
            http_client,
            base_url,
            api_key,
            default_model,
        }
    }
```

- [ ] **Step 2: Add tracing instrumentation**

Add `#[tracing::instrument(skip(self, callback))]` to `chat_streaming()` and `#[tracing::instrument(skip(self))]` to `chat()`. Also add `tracing::debug!` logs at key points. In `chat()`, after the response is parsed:

```rust
        tracing::debug!(
            finish_reason = ?finish_reason,
            tokens = response.usage.total_tokens,
            "chat completed"
        );
```

In `chat_streaming()`, at the end before the return:

```rust
        tracing::debug!(
            finish_reason = ?finish_reason,
            tool_calls = tool_calls_by_index.len(),
            tokens = usage.total_tokens,
            "chat streaming completed"
        );
```

- [ ] **Step 3: Fix silent SSE parse error swallowing**

Replace line 313 (`Err(_) => {}`):

```rust
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            data = %data.chars().take(200).collect::<String>(),
                            "Failed to parse SSE chunk, skipping"
                        );
                    }
```

- [ ] **Step 4: Run all tests to verify nothing broke**

Run: `cargo test -p llm`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/llm/src/openai.rs
git commit -m "fix(llm): add reqwest timeout, tracing spans, and SSE error logging"
```

---

### Task 7: Verify Downstream Compatibility

**Files:**
- Modify: `crates/torque-runtime/src/message.rs` (downstream fix)

- [ ] **Step 1: Run workspace check to find breakage**

Run: `cargo check --workspace`
Expected: ERROR in `torque-runtime/src/message.rs` — `Message` struct missing `tool_calls` field

- [ ] **Step 2: Fix the `From<RuntimeMessage> for LlmMessage` conversion**

In `crates/torque-runtime/src/message.rs`, lines 67-70, add `tool_calls: None`:

```rust
        LlmMessage {
            role: role.to_string(),
            content: value.content,
            tool_calls: None,
        }
```

- [ ] **Step 3: Run workspace check again**

Run: `cargo check --workspace`
Expected: PASS

- [ ] **Step 4: Run all llm tests + torque-runtime tests**

Run: `cargo test -p llm -p torque-runtime`
Expected: All tests PASS

- [ ] **Step 5: Run full test suite (optional but recommended)**

Run: `cargo test --workspace`
Expected: All tests PASS (or pre-existing failures unrelated to these changes)

- [ ] **Step 6: Commit**

```bash
git add crates/torque-runtime/src/message.rs
git commit -m "fix(torque-runtime): add tool_calls: None to LlmMessage construction"
```

---

## Verification Checklist (Before Merging)

- [ ] `cargo check -p llm` — no warnings
- [ ] `cargo test -p llm` — all tests pass (13 unit + 4 integration = 17 tests)
- [ ] `cargo check --workspace` — no downstream breakage
- [ ] `cargo test --workspace` — no regressions
- [ ] `git log --oneline` — 7 clean commits
