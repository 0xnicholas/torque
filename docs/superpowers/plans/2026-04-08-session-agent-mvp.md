# Session Agent MVP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a stable, product-facing single-agent session demo with persistent sessions, multi-turn chat, SSE streaming, bounded context handling, and a minimal safe tool surface.

**Architecture:** Evolve `crates/session-agent` into a clean MVP path rather than building a second demo stack. Keep the public surface small and product-facing while tightening internal seams around HTTP API, session persistence, agent runner, context trimming, streaming events, and one safe built-in tool.

**Tech Stack:** Rust, Axum, Tokio, SQLx/PostgreSQL, SSE, OpenAI-compatible LLM client, serde/serde_json

---

## File Map

### Existing files that should remain primary

- `crates/session-agent/src/main.rs`
  service bootstrap, environment loading, app construction
- `crates/session-agent/src/api/mod.rs`
  route registration and app-state wiring
- `crates/session-agent/src/api/sessions.rs`
  session create/get/list handlers
- `crates/session-agent/src/api/messages.rs`
  message list and streaming chat handlers
- `crates/session-agent/src/agent/runner.rs`
  per-turn execution loop and persistence orchestration
- `crates/session-agent/src/agent/context.rs`
  recent-window context trimming and summary seam
- `crates/session-agent/src/agent/stream.rs`
  SSE event model
- `crates/session-agent/src/db/sessions.rs`
  session queries and status updates
- `crates/session-agent/src/db/messages.rs`
  message persistence and recent-history reads
- `crates/session-agent/src/tools/builtin.rs`
  demo-safe built-in tools
- `crates/session-agent/src/tools/registry.rs`
  runtime tool lookup and execution
- `crates/session-agent/tests/common/mod.rs`
  integration test DB setup
- `crates/session-agent/tests/api_tests.rs`
  current API/data tests; keep or split as needed
- `crates/session-agent/README.md`
  local demo setup and API walkthrough

### New files recommended

- `crates/session-agent/src/app.rs`
  reusable app builder for production and tests
- `crates/session-agent/src/agent/runtime.rs`
  optional thin façade for “single turn input -> streaming output” if `runner.rs` starts growing too large
- `crates/session-agent/tests/session_http_api.rs`
  HTTP-level session lifecycle tests
- `crates/session-agent/tests/chat_streaming_api.rs`
  HTTP-level chat/SSE tests
- `crates/session-agent/tests/agent_runner_tests.rs`
  unit/integration tests around runner behavior with a fake LLM
- `crates/session-agent/tests/context_window_tests.rs`
  context trimming tests

### Optional test-only support

- `crates/session-agent/tests/common/fake_llm.rs`
  fake LLM implementation or adapter used by runner/SSE tests

If fake-LLM support fits better under `src/agent/` as a test helper module, keep it there instead of adding a new common file.

---

### Task 1: Create a Testable App Construction Seam

**Files:**
- Create: `crates/session-agent/src/app.rs`
- Modify: `crates/session-agent/src/lib.rs`
- Modify: `crates/session-agent/src/main.rs`
- Modify: `crates/session-agent/src/api/mod.rs`
- Test: `crates/session-agent/tests/session_http_api.rs`

- [ ] **Step 1: Write the failing HTTP app-construction test**

```rust
#[tokio::test]
async fn create_session_route_works_through_app_builder() {
    let app = test_app().await;
    let response = post_json(&app, "/sessions", serde_json::json!({})).await;
    assert_eq!(response.status(), StatusCode::OK);
}
```

- [ ] **Step 2: Run the new test to confirm the current app cannot be exercised cleanly**

Run: `cargo test -p session-agent create_session_route_works_through_app_builder -- --nocapture`

Expected: FAIL because there is no reusable app-construction seam for tests or because the current route wiring is too tightly bound to `main.rs` / concrete client setup.

- [ ] **Step 3: Add a reusable `build_app(...)` entry point**

```rust
pub fn build_app(
    db: Database,
    llm: Arc<OpenAiClient>,
) -> Router {
    api::router(db, llm)
}
```

If tests need a fake client, move one level further and introduce a wrapper state type that can be constructed in tests without invoking production env loading.

- [ ] **Step 4: Update `main.rs` to use the new app builder**

```rust
let app = session_agent::app::build_app(database, llm);
```

- [ ] **Step 5: Run the new test again**

Run: `cargo test -p session-agent create_session_route_works_through_app_builder -- --nocapture`

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/session-agent/src/app.rs crates/session-agent/src/lib.rs crates/session-agent/src/main.rs crates/session-agent/src/api/mod.rs crates/session-agent/tests/session_http_api.rs
git commit -m "refactor: add reusable session agent app builder"
```

---

### Task 2: Lock the Session Lifecycle API

**Files:**
- Modify: `crates/session-agent/src/api/mod.rs`
- Modify: `crates/session-agent/src/api/sessions.rs`
- Modify: `crates/session-agent/src/db/sessions.rs`
- Modify: `crates/session-agent/src/models/session.rs`
- Test: `crates/session-agent/tests/session_http_api.rs`

- [ ] **Step 1: Write failing HTTP tests for session lifecycle**

```rust
#[tokio::test]
async fn list_sessions_only_returns_sessions_for_current_api_key() {
    let app = test_app().await;
    let response = get(&app, "/sessions", "key-1").await;
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn get_session_rejects_other_api_keys() {
    let app = test_app().await;
    let response = get(&app, &format!("/sessions/{id}"), "wrong-key").await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
```

- [ ] **Step 2: Run the session API tests**

Run: `cargo test -p session-agent session_http_api -- --nocapture`

Expected: FAIL because `GET /sessions` is not implemented and session-list querying is incomplete.

- [ ] **Step 3: Implement `GET /sessions` and missing DB query helpers**

```rust
pub async fn list_by_api_key(
    pool: &PgPool,
    api_key: &str,
    limit: i64,
    offset: i64,
) -> anyhow::Result<Vec<Session>> { /* ... */ }
```

Also add the route:

```rust
.route("/sessions", post(sessions::create).get(sessions::list))
```

- [ ] **Step 4: Keep responses product-facing**

Ensure session responses expose only:

- `id`
- `status`
- `created_at`
- `updated_at`

and never echo the stored API key.

- [ ] **Step 5: Run the session API tests again**

Run: `cargo test -p session-agent session_http_api -- --nocapture`

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/session-agent/src/api/mod.rs crates/session-agent/src/api/sessions.rs crates/session-agent/src/db/sessions.rs crates/session-agent/src/models/session.rs crates/session-agent/tests/session_http_api.rs
git commit -m "feat: add session lifecycle api"
```

---

### Task 3: Lock the Streaming Chat Contract

**Files:**
- Modify: `crates/session-agent/src/api/messages.rs`
- Modify: `crates/session-agent/src/agent/stream.rs`
- Modify: `crates/session-agent/src/agent/runner.rs`
- Test: `crates/session-agent/tests/chat_streaming_api.rs`
- Test: `crates/session-agent/tests/agent_runner_tests.rs`

- [ ] **Step 1: Write a failing runner test with a fake LLM**

```rust
#[tokio::test]
async fn runner_persists_user_and_assistant_messages_for_one_turn() {
    let fake = FakeLlm::streaming_text("Hello from fake model");
    let saved = runner.run(&session, &user_message, tx).await.unwrap();
    assert_eq!(saved.content, "Hello from fake model");
}
```

- [ ] **Step 2: Write a failing streaming API test**

```rust
#[tokio::test]
async fn chat_endpoint_emits_start_chunk_and_done_events() {
    let app = test_app_with_fake_llm(FakeLlm::streaming_text("hello")).await;
    let events = post_chat_and_collect_events(&app, session_id, "Hi").await;
    assert_eq!(events[0]["event"], "start");
    assert_eq!(events.last().unwrap()["event"], "done");
}
```

- [ ] **Step 3: Run both chat-related test files**

Run: `cargo test -p session-agent chat_streaming_api agent_runner_tests -- --nocapture`

Expected: FAIL because the current code does not emit a `start` event and is not yet cleanly testable end-to-end.

- [ ] **Step 4: Add a `start` SSE event and make the streaming contract explicit**

```rust
pub enum StreamEvent {
    Start { session_id: Uuid },
    Chunk { content: String },
    ToolCall { name: String, arguments: Value },
    Done { message_id: Uuid, artifacts: Option<Value> },
    Error { code: String, message: String },
}
```

- [ ] **Step 5: Tighten `AgentRunner::run` around one-turn persistence**

Make sure the runner:

- saves the user message first
- loads recent history
- streams chunks
- saves the final assistant message once
- emits one terminal `done` event on success

- [ ] **Step 6: Run the chat tests again**

Run: `cargo test -p session-agent chat_streaming_api agent_runner_tests -- --nocapture`

Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/session-agent/src/api/messages.rs crates/session-agent/src/agent/stream.rs crates/session-agent/src/agent/runner.rs crates/session-agent/tests/chat_streaming_api.rs crates/session-agent/tests/agent_runner_tests.rs
git commit -m "feat: lock session chat streaming contract"
```

---

### Task 4: Harden Session Status and Error Handling

**Files:**
- Modify: `crates/session-agent/src/models/session.rs`
- Modify: `crates/session-agent/src/db/sessions.rs`
- Modify: `crates/session-agent/src/api/messages.rs`
- Modify: `crates/session-agent/src/agent/runner.rs`
- Test: `crates/session-agent/tests/agent_runner_tests.rs`

- [ ] **Step 1: Write failing tests for status transitions**

```rust
#[tokio::test]
async fn chat_marks_session_running_then_completed_on_success() {
    // send one successful turn
    // assert final session status == Completed
}

#[tokio::test]
async fn chat_marks_session_error_on_runner_failure() {
    // fake llm returns error
    // assert final session status == Error
}
```

- [ ] **Step 2: Run the status-transition tests**

Run: `cargo test -p session-agent status_transition -- --nocapture`

Expected: FAIL because successful turns currently do not clearly drive `Running -> Completed`.

- [ ] **Step 3: Implement explicit status transitions around each turn**

```rust
update_status(pool, session.id, SessionStatus::Running, None).await?;
// run turn
update_status(pool, session.id, SessionStatus::Completed, None).await?;
```

On failure:

```rust
update_status(pool, session.id, SessionStatus::Error, Some(&err.to_string())).await?;
```

- [ ] **Step 4: Keep repeat-chat behavior valid**

If `Completed` is treated as a ready state, keep `Session::can_receive_message()` aligned:

```rust
matches!(self.status, SessionStatus::Idle | SessionStatus::Completed)
```

- [ ] **Step 5: Run the runner/status tests again**

Run: `cargo test -p session-agent agent_runner_tests -- --nocapture`

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/session-agent/src/models/session.rs crates/session-agent/src/db/sessions.rs crates/session-agent/src/api/messages.rs crates/session-agent/src/agent/runner.rs crates/session-agent/tests/agent_runner_tests.rs
git commit -m "fix: enforce session turn status transitions"
```

---

### Task 5: Bound Context Growth for Demo Stability

**Files:**
- Modify: `crates/session-agent/src/agent/context.rs`
- Modify: `crates/session-agent/src/agent/runner.rs`
- Test: `crates/session-agent/tests/context_window_tests.rs`

- [ ] **Step 1: Write failing context-window tests**

```rust
#[test]
fn context_manager_only_keeps_recent_window() {
    let history = make_messages(25);
    let context = ContextManager::new().build_context(history);
    assert_eq!(context.messages.len(), DEFAULT_WINDOW_SIZE);
}

#[test]
fn context_manager_preserves_recent_order() {
    let history = make_messages(12);
    let context = ContextManager::new().build_context(history);
    assert_eq!(context.messages.first().unwrap().content, "message-2");
}
```

- [ ] **Step 2: Run the context tests**

Run: `cargo test -p session-agent context_window_tests -- --nocapture`

Expected: FAIL if the current file lacks clear behavior coverage or if the order/window behavior drifts while refactoring.

- [ ] **Step 3: Make the context policy explicit**

Keep MVP behavior simple and documented:

- recent turns only
- stable ordering
- one obvious `DEFAULT_WINDOW_SIZE`
- no hidden prompt stuffing

If older-turn summarization is deferred, leave a comment seam rather than speculative code.

- [ ] **Step 4: Keep runner usage aligned**

```rust
let history = get_recent_by_session(...).await?;
let context = self.context_manager.build_context(history);
```

Do not add a second hidden context path in `runner.rs`.

- [ ] **Step 5: Run the context tests again**

Run: `cargo test -p session-agent context_window_tests -- --nocapture`

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/session-agent/src/agent/context.rs crates/session-agent/src/agent/runner.rs crates/session-agent/tests/context_window_tests.rs
git commit -m "feat: bound session context window"
```

---

### Task 6: Reduce the Tool Surface to a Demo-Safe Whitelist

**Files:**
- Modify: `crates/session-agent/src/tools/builtin.rs`
- Modify: `crates/session-agent/src/tools/registry.rs`
- Modify: `crates/session-agent/src/agent/runner.rs`
- Test: `crates/session-agent/tests/agent_runner_tests.rs`
- Test: `crates/session-agent/tests/chat_streaming_api.rs`

- [ ] **Step 1: Write a failing test that asserts only demo-safe tools are exposed**

```rust
#[tokio::test]
async fn builtin_tool_registry_only_exposes_demo_safe_tools() {
    let registry = demo_tool_registry().await;
    let tools = registry.to_llm_tools().await;
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "web_search");
}
```

- [ ] **Step 2: Run the tool-surface tests**

Run: `cargo test -p session-agent demo_safe_tools -- --nocapture`

Expected: FAIL because the current built-ins include `file_read` and `code_execute`.

- [ ] **Step 3: Shrink `create_builtin_tools()` to one safe demo tool**

```rust
pub fn create_builtin_tools() -> Vec<Box<dyn Tool>> {
    vec![Box::new(WebSearchTool)]
}
```

Keep the registry generic; only the default built-in set should shrink.

- [ ] **Step 4: Confirm tool events still stream correctly**

Use an existing fake-LLM test to prove:

- tool call event appears
- tool result is folded back into the turn
- assistant still reaches `done`

- [ ] **Step 5: Run the tool-related tests again**

Run: `cargo test -p session-agent tool -- --nocapture`

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/session-agent/src/tools/builtin.rs crates/session-agent/src/tools/registry.rs crates/session-agent/src/agent/runner.rs crates/session-agent/tests/agent_runner_tests.rs crates/session-agent/tests/chat_streaming_api.rs
git commit -m "feat: restrict session agent to demo-safe tools"
```

---

### Task 7: Refresh Demo Documentation

**Files:**
- Modify: `crates/session-agent/README.md`
- Modify: `AGENTS.md`
- Test: none

- [ ] **Step 1: Write the documentation delta before editing**

Capture the exact MVP promises:

- session create/list/get
- message history
- SSE chat
- bounded context
- optional one-tool demo

- [ ] **Step 2: Update `crates/session-agent/README.md`**

Add:

- local setup
- migration command
- required env vars
- concrete curl examples for session create, message history, and SSE chat
- note that this is the current MVP slice, not the full future team runtime

- [ ] **Step 3: Update `AGENTS.md` only if the repo-level guide needs a one-line pointer**

Example:

```md
The current product-facing MVP path lives in `crates/session-agent`.
```

Skip this step if the current `AGENTS.md` already makes the prototype-vs-target split clear enough.

- [ ] **Step 4: Review the demo flow manually**

Check that the README examples line up with the actual endpoints:

- `POST /sessions`
- `GET /sessions`
- `GET /sessions/{id}`
- `GET /sessions/{id}/messages`
- `POST /sessions/{id}/chat`

- [ ] **Step 5: Commit**

```bash
git add crates/session-agent/README.md AGENTS.md
git commit -m "docs: refresh session agent mvp demo guide"
```

---

### Task 8: Full MVP Verification Pass

**Files:**
- Modify: none unless verification reveals issues
- Test: `crates/session-agent/tests/session_http_api.rs`
- Test: `crates/session-agent/tests/chat_streaming_api.rs`
- Test: `crates/session-agent/tests/agent_runner_tests.rs`
- Test: `crates/session-agent/tests/context_window_tests.rs`

- [ ] **Step 1: Run the focused crate test suite**

Run: `cargo test -p session-agent -- --nocapture`

Expected: PASS

- [ ] **Step 2: Run the service locally**

Run: `cargo run -p session-agent`

Expected: service boots, runs migrations, and listens on the configured bind address

- [ ] **Step 3: Exercise the README demo manually**

Check:

- session creation works
- listing works
- message history works
- SSE stream delivers chunked output
- at least one successful multi-turn session works

- [ ] **Step 4: Fix any verification-only drift**

Only make narrowly scoped fixes found during the verification pass.

- [ ] **Step 5: Commit final verification fixes if needed**

```bash
git add <exact files changed during verification>
git commit -m "fix: polish session agent mvp verification issues"
```

---

## Notes For The Implementer

- Keep the MVP product-facing. Do not leak future platform abstractions into public APIs unless required for testability.
- Reuse `crates/llm`; do not fork a second LLM client path.
- Prefer one clean fake-LLM seam over brittle tests that depend on live external calls.
- Do not add team, approval, capability, or recovery UI concepts into this MVP.
- If `runner.rs` becomes hard to reason about, split by responsibility, not by speculative architecture.

## Suggested Milestone Order

1. testable app seam
2. session lifecycle API
3. streaming chat contract
4. session status transitions
5. bounded context window
6. demo-safe tool whitelist
7. docs refresh
8. full verification
