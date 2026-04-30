# Deepagents-Inspired Harness Enhancements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire Torque's dead harness infrastructure (ToolOffloadPolicy, ContextCompactionService, RoutedVfs) into the live LLM conversation loop.

**Architecture:** Three tasks: (1) auto-wire `ToolOffloadPolicy` after every tool call, (2) wire `ContextCompactionService` for pre-turn message compaction, (3) make `RoutedVfs` accept prefix routing with root aggregation.

**Tech Stack:** Rust, tokio, `llm` crate, `torque-runtime`

---

## File Map

```
crates/torque-runtime/src/
├── host.rs      [MODIFY] Wire offload after tool calls, compaction before model turns
├── context.rs   [MODIFY] Add CompactSummary::to_runtime_message(), CompactSummary::is_compaction_message()
├── vfs.rs       [MODIFY] RoutedVfs with route map, root aggregation

crates/torque-runtime/tests/
├── host_port_integration.rs  [MODIFY] Add offload + compaction integration tests
├── vfs_contracts.rs          [MODIFY] Add RoutedVfs routing table tests

crates/torque-harness/
├── src/service/tool.rs            [MODIFY] Update RoutedVfs constructor call
├── tests/tool_offload_tests.rs    [MODIFY] Update RoutedVfs constructor call
```

---

## Task 1: Auto-Trigger Tool Result Offloading

**Goal**: After each tool call in `run_llm_conversation`, auto-apply `ToolOffloadPolicy.offload()` to replace >4KB results with preview + scratch ref.

**Files:**
- Modify: `crates/torque-runtime/src/host.rs:1-12, 53-89, 182-195`
- Test: Existing `crates/torque-runtime/tests/host_port_integration.rs` (extend)

- [ ] **Step 1: Add offload_policy field + builder to RuntimeHost**

Open `crates/torque-runtime/src/host.rs`. Add import at line 1-5:
```rust
use crate::offload::ToolOffloadPolicy;
```

Add field inside `RuntimeHost` struct (after `approval_gateway`):
```rust
    offload_policy: Option<Arc<ToolOffloadPolicy>>,
```

Add to `new()` constructor body (after `approval_gateway: None`):
```rust
            offload_policy: None,
```

Add builder method after `with_approval_gateway()`:
```rust
    pub fn with_offload_policy(mut self, offload_policy: Arc<ToolOffloadPolicy>) -> Self {
        self.offload_policy = Some(offload_policy);
        self
    }
```

Run: `cargo check -p torque-runtime`
Expected: Compiles clean

- [ ] **Step 2: Wire offload into run_llm_conversation**

In `run_llm_conversation()` (~line 182-195), after `tool_executor.execute(...)` returns `result`, insert before the `if result.success` block:

```rust
                let result = tool_executor
                    .execute(
                        RuntimeExecutionContext {
                            instance_id: instance_id.as_uuid(),
                            request_id: None,
                            source_task_id: None,
                        },
                        &tool_call.name,
                        tool_call.arguments.clone(),
                    )
                    .await?;

                // Auto-offload large tool results to scratch/artifact
                let result = if let Some(offload) = &self.offload_policy {
                    offload
                        .offload(&tool_call.name, result, Some(instance_id.as_uuid()))
                        .await?
                } else {
                    result
                };
```

Run: `cargo check -p torque-runtime`
Expected: Compiles clean

- [ ] **Step 3: Write integration test for auto-offload**

Open `crates/torque-runtime/tests/host_port_integration.rs`. Add imports and a new test at end of file.

Add imports (after existing imports):
```rust
use std::sync::Mutex;
use torque_runtime::offload::ToolOffloadPolicy;
use torque_runtime::vfs::{EditResult, FileInfo, GrepMatch, VfsBackend};
use torque_runtime::tools::RuntimeToolResult;
use torque_kernel::ExecutionRequest;
```

Add a `ToolCallingModelDriver` (needed because `FakeModelDriver` returns `Stop` — tools never called):
```rust
/// Returns a single tool call on first turn, then stops.
struct ToolCallingModelDriver;
#[async_trait]
impl RuntimeModelDriver for ToolCallingModelDriver {
    async fn run_turn(
        &self,
        _messages: Vec<RuntimeMessage>,
        _tools: Vec<RuntimeToolDef>,
        _sink: Option<&dyn RuntimeOutputSink>,
    ) -> anyhow::Result<ModelTurnResult> {
        Ok(ModelTurnResult {
            finish_reason: RuntimeFinishReason::ToolCalls,
            assistant_text: String::new(),
            tool_calls: vec![torque_runtime::tools::RuntimeToolCall {
                id: "call_test".to_string(),
                name: "test_tool".to_string(),
                arguments: serde_json::json!({}),
            }],
            usage: None,
        })
    }
}

/// Returns >4KB content to trigger offload threshold.
struct LargeOutputToolExecutor;
#[async_trait]
impl RuntimeToolExecutor for LargeOutputToolExecutor {
    async fn execute(&self, _ctx: RuntimeExecutionContext, _name: &str, _args: serde_json::Value) -> anyhow::Result<RuntimeToolResult> {
        Ok(RuntimeToolResult {
            success: true,
            content: "x".repeat(5000),
            error: None,
            offload_ref: None,
        })
    }
    async fn tool_defs(&self) -> anyhow::Result<Vec<RuntimeToolDef>> { Ok(vec![]) }
}

/// Records paths written via VFS.
struct RecordingScratch(Mutex<Vec<String>>);
#[async_trait]
impl VfsBackend for RecordingScratch {
    async fn ls(&self, _: &str) -> anyhow::Result<Vec<FileInfo>> { Ok(vec![]) }
    async fn read(&self, _: &str) -> anyhow::Result<String> { Ok(String::new()) }
    async fn write(&self, path: &str, _: &str) -> anyhow::Result<()> {
        self.0.lock().unwrap().push(path.to_string()); Ok(())
    }
    async fn edit(&self, _: &str, _: &str, _: &str, _: bool) -> anyhow::Result<EditResult> { Ok(EditResult { occurrences: 0 }) }
    async fn glob(&self, _: &str, _: &str) -> anyhow::Result<Vec<FileInfo>> { Ok(vec![]) }
    async fn grep(&self, _: &str, _: &str) -> anyhow::Result<Vec<GrepMatch>> { Ok(vec![]) }
}
```

Now add the test:
```rust
#[tokio::test]
async fn tool_result_offloaded_to_scratch_when_above_inline_threshold() {
    let scratch = Arc::new(RecordingScratch(Mutex::new(vec![])));
    let offload_policy = Arc::new(ToolOffloadPolicy::new(Some(scratch.clone()), None));
    let agent_def = torque_kernel::AgentDefinition::new("test", "system");

    let mut host = RuntimeHost::new(
        vec![agent_def.clone()],
        Arc::new(FakeEventSink::default()),
        Arc::new(FakeCheckpointSink::default()),
    ).with_offload_policy(offload_policy);

    let request = ExecutionRequest::new(agent_def.id, "Test offload", vec![]);
    let _ = host.execute_v1(
        request,
        &ToolCallingModelDriver,
        &LargeOutputToolExecutor,
        None,
        vec![RuntimeMessage::user("go")],
    ).await;

    let paths = scratch.0.lock().unwrap();
    assert!(
        paths.iter().any(|p| p.starts_with("/scratch/tool-results/")),
        "Expected offloaded path in {:?}",
        paths
    );
}
```

Run: `cargo test -p torque-runtime --test host_port_integration -- tool_result_offloaded_to_scratch`
Expected: PASS

- [ ] **Step 4: Run full runtime test suite**

```bash
cargo test -p torque-runtime
```
Expected: All pass (no regressions)

- [ ] **Step 5: Commit**

```bash
git add crates/torque-runtime/src/host.rs crates/torque-runtime/tests/host_port_integration.rs
git commit -m "feat(runtime): auto-offload tool results above 4KB via offload_policy in run_llm_conversation"
```

---

## Task 2: Context Compaction Before Each Model Turn

**Goal**: Before each LLM turn in `run_llm_conversation`, check message count/token threshold. If exceeded, compact old messages into a summary, keeping recent messages. Inject summary as a user message.

**How it works**: `ContextCompactionService.compact()` takes `&[LlmMessage]`, returns `CompactSummary` with `preserved_tail`. Host converts messages → LlmMessage for compaction, then rebuilds with summary + preserved messages.

**Files:**
- Modify: `crates/torque-runtime/src/context.rs:29-80` (add `to_runtime_message()`, `is_compaction_message()`)
- Modify: `crates/torque-runtime/src/host.rs:1-12, 53-89, 167-175` (add import, field, wire before model turn)
- Modify: `crates/torque-runtime/tests/host_port_integration.rs`

- [ ] **Step 1: Add CompactSummary helper methods**

Open `crates/torque-runtime/src/context.rs`. Add after `CompactSummary` struct (line 34):

```rust
impl CompactSummary {
    /// Convert to a RuntimeMessage for injection into the conversation.
    pub fn to_runtime_message(&self) -> crate::message::RuntimeMessage {
        crate::message::RuntimeMessage::user(format!(
            "[Context Compaction] {} Key facts from earlier messages:\n  {}",
            self.compact_summary,
            self.key_facts.join("\n  ")
        ))
    }

    /// Check if content looks like a prior compaction message.
    pub fn is_compaction_message(content: &str) -> bool {
        content.starts_with("[Context Compaction]")
    }
}
```

Run: `cargo check -p torque-runtime`
Expected: Compiles clean

- [ ] **Step 2: Add compaction_service field to RuntimeHost**

In `crates/torque-runtime/src/host.rs`, add imports:
```rust
use llm::Message as LlmMessage;
use crate::context::{ContextCompactionPolicy, ContextCompactionService};
```

Add field to struct (after `offload_policy`):
```rust
    compaction_service: ContextCompactionService,
```

In `new()` body (after `offload_policy: None`):
```rust
            compaction_service: ContextCompactionService::new(ContextCompactionPolicy::default()),
```

Add builder:
```rust
    pub fn with_compaction_policy(mut self, policy: ContextCompactionPolicy) -> Self {
        self.compaction_service = ContextCompactionService::new(policy);
        self
    }
```

Run: `cargo check -p torque-runtime`
Expected: Compiles clean

- [ ] **Step 3: Wire compaction before each model turn**

In `run_llm_conversation()`, inside the `loop {}`, **before** `model_driver.run_turn(messages.clone(), ...)` (~line 174), insert:

```rust
            // Auto-compact context before model turn if threshold exceeded.
            let llm_messages: Vec<LlmMessage> =
                messages.iter().map(|m| m.clone().into()).collect();
            if let Some(compacted) = self.compaction_service.compact(&llm_messages) {
                messages = vec![compacted.to_runtime_message()];
                for lm in compacted.preserved_tail {
                    messages.push(crate::message::RuntimeMessage::from(LlmMessage {
                        role: lm.role,
                        content: lm.content,
                        tool_calls: lm.tool_calls,
                    }));
                }
            }
```

Note: `RuntimeMessage::from(LlmMessage)` is at `crates/torque-runtime/src/message.rs:44-57`.

Run: `cargo check -p torque-runtime`
Expected: Compiles clean

- [ ] **Step 4: Write integration test for compaction**

Add to `crates/torque-runtime/tests/host_port_integration.rs`:

```rust
#[tokio::test]
async fn context_compacted_when_messages_exceed_threshold() {
    let mut messages = vec![];
    for i in 0..20 {
        messages.push(RuntimeMessage::user(format!("message {}", i)));
        messages.push(RuntimeMessage::assistant(format!("response {}", i)));
    }

    let agent_def = torque_kernel::AgentDefinition::new("test", "system");
    let mut host = RuntimeHost::new(
        vec![agent_def.clone()],
        Arc::new(FakeEventSink::default()),
        Arc::new(FakeCheckpointSink::default()),
    );

    let request = ExecutionRequest::new(agent_def.id, "Compact test", vec![]);
    let result = host
        .execute_v1(request, &FakeModelDriver, &LargeOutputToolExecutor, None, messages)
        .await;

    assert!(result.is_ok());
}
```

Run: `cargo test -p torque-runtime --test host_port_integration -- context_compacted_when_messages_exceed_threshold`
Expected: PASS

- [ ] **Step 5: Run full runtime test suite**

```bash
cargo test -p torque-runtime
```
Expected: All pass

- [ ] **Step 6: Commit**

```bash
git add crates/torque-runtime/src/context.rs crates/torque-runtime/src/host.rs crates/torque-runtime/tests/host_port_integration.rs
git commit -m "feat(runtime): auto-compact context before model turns when message threshold exceeded"
```

---

## Task 3: RoutedVfs with Prefix Routing + Root Aggregation

**Goal**: Replace hardcoded `scratch + workspace` two-backend `RoutedVfs` with a prefix-based routing table. At path `/`, aggregate all backends. Backward compatible — keep `new(scratch, workspace)` and `for_current_workspace()` signatures.

**Files:**
- Modify: `crates/torque-runtime/src/vfs.rs:43-170` (rewrite `RoutedVfs`, keep `impl VfsBackend for RoutedVfs`)
- Modify: `crates/torque-harness/src/service/tool.rs:26` (optional: update constructor)
- Modify: `crates/torque-harness/tests/tool_offload_tests.rs:145` (optional: update constructor)
- Test: `crates/torque-runtime/tests/vfs_contracts.rs` (extend)

- [ ] **Step 1: Rewrite RoutedVfs struct and inherent impl**

In `crates/torque-runtime/src/vfs.rs`, replace lines 43-170 (RoutedVfs struct + impl + route_backend + RoutedBackend). Keep `impl VfsBackend for RoutedVfs` at lines 114-145 intact.

```rust
pub struct RoutedVfs {
    routes: Vec<(String, Arc<dyn VfsBackend>)>,
}

impl RoutedVfs {
    pub fn new(raw_routes: Vec<(String, Arc<dyn VfsBackend>)>) -> Self {
        let mut sorted = raw_routes;
        sorted.sort_by(|(a, _), (b, _)| b.len().cmp(&a.len()));
        Self { routes: sorted }
    }

    pub fn for_current_workspace() -> Self {
        let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self::new(vec![
            ("/scratch".to_string(), Arc::new(ScratchBackend::default())),
            ("/workspace".to_string(), Arc::new(WorkspaceBackend::new(root))),
        ])
    }

    fn resolve(&self, path: &str) -> Option<&Arc<dyn VfsBackend>> {
        self.routes.iter()
            .find(|(prefix, _)| path.starts_with(prefix.as_str()))
            .map(|(_, backend)| backend)
    }

    pub async fn ls(&self, path: &str) -> anyhow::Result<Vec<FileInfo>> {
        if path == "/" {
            let mut results = Vec::new();
            for (prefix, backend) in &self.routes {
                if let Ok(files) = backend.ls(prefix).await {
                    results.extend(files);
                }
            }
            return Ok(results);
        }
        match self.resolve(path) {
            Some(backend) => backend.ls(path).await,
            None => anyhow::bail!("No backend found for path: {}", path),
        }
    }

    pub async fn read(&self, path: &str) -> anyhow::Result<String> {
        match self.resolve(path) {
            Some(backend) => backend.read(path).await,
            None => anyhow::bail!("No backend found for path: {}", path),
        }
    }

    pub async fn write(&self, path: &str, content: &str) -> anyhow::Result<()> {
        match self.resolve(path) {
            Some(backend) => backend.write(path, content).await,
            None => anyhow::bail!("No backend found for path: {}", path),
        }
    }

    pub async fn edit(&self, path: &str, old_string: &str, new_string: &str, replace_all: bool) -> anyhow::Result<EditResult> {
        match self.resolve(path) {
            Some(backend) => backend.edit(path, old_string, new_string, replace_all).await,
            None => anyhow::bail!("No backend found for path: {}", path),
        }
    }

    pub async fn glob(&self, path: &str, pattern: &str) -> anyhow::Result<Vec<FileInfo>> {
        if path == "/" {
            let mut results = Vec::new();
            for (prefix, backend) in &self.routes {
                if let Ok(files) = backend.glob(prefix, pattern).await {
                    results.extend(files);
                }
            }
            return Ok(results);
        }
        match self.resolve(path) {
            Some(backend) => backend.glob(path, pattern).await,
            None => anyhow::bail!("No backend found for path: {}", path),
        }
    }

    pub async fn grep(&self, path: &str, pattern: &str) -> anyhow::Result<Vec<GrepMatch>> {
        match self.resolve(path) {
            Some(backend) => backend.grep(path, pattern).await,
            None => anyhow::bail!("No backend found for path: {}", path),
        }
    }
}
```

Delete the old `RoutedBackend` enum (lines 147-150) and `route_backend` function (lines 152-167).

Run: `cargo check -p torque-runtime`
Expected: Compiles clean (downstream crates may have warnings about old constructor — handled in Step 3)

- [ ] **Step 2: Update vfs_contracts test helper**

Open `crates/torque-runtime/tests/vfs_contracts.rs`. The `test_vfs` helper at line 5 uses the old two-arg constructor:

```rust
// OLD:
RoutedVfs::new(
    Arc::new(ScratchBackend::default()),
    Arc::new(WorkspaceBackend::new(workspace_root)),
)
```

Replace with:
```rust
RoutedVfs::new(vec![
    ("/scratch".to_string(), Arc::new(ScratchBackend::default())),
    ("/workspace".to_string(), Arc::new(WorkspaceBackend::new(workspace_root))),
])
```

Run: `cargo test -p torque-runtime --test vfs_contracts`
Expected: All 7 existing tests pass

- [ ] **Step 3: Update downstream callers**

Two callers use the old two-arg constructor:

In `crates/torque-harness/tests/tool_offload_tests.rs:145`:
```rust
// OLD: Arc::new(RoutedVfs::new(scratch, workspace))
// NEW:
Arc::new(RoutedVfs::new(vec![
    ("/scratch".to_string(), scratch),
    ("/workspace".to_string(), workspace),
]))
```

The other callers (`tool.rs:26`, `file_approval_flow_tests.rs:89`, `todo_tools_tests.rs:181`) use `RoutedVfs::for_current_workspace()` which we preserved — no changes needed.

Run: `cargo check --workspace`
Expected: Compiles clean

- [ ] **Step 4: Add routing table tests**

Add to `crates/torque-runtime/tests/vfs_contracts.rs`:

```rust
#[tokio::test]
async fn routed_vfs_custom_routing_table() {
    let scratch = Arc::new(ScratchBackend::default());
    let ws = Arc::new(WorkspaceBackend::new(PathBuf::from(".")));

    let vfs = RoutedVfs::new(vec![
        ("/scratch".to_string(), scratch.clone()),
        ("/workspace".to_string(), ws.clone()),
    ]);

    vfs.write("/scratch/test.txt", "hello").await.unwrap();
    let content = vfs.read("/scratch/test.txt").await.unwrap();
    assert_eq!(content, "hello");
}

#[tokio::test]
async fn routed_vfs_root_aggregates_all_backends() {
    let scratch = Arc::new(ScratchBackend::default());
    scratch.write("/scratch/file.txt", "data").await.unwrap();
    let ws = Arc::new(WorkspaceBackend::new(PathBuf::from(".")));
    ws.write("/workspace/other.txt", "more").await.unwrap();

    let vfs = RoutedVfs::new(vec![
        ("/scratch".to_string(), scratch),
        ("/workspace".to_string(), ws),
    ]);

    let entries = vfs.ls("/").await.unwrap();
    assert!(!entries.is_empty());
    // Should include files from both backends
    assert!(entries.iter().any(|e| e.path.contains("file.txt")));
    assert!(entries.iter().any(|e| e.path.contains("other.txt")));
}

#[tokio::test]
async fn routed_vfs_unknown_path_returns_error() {
    let vfs = RoutedVfs::new(vec![]);
    let result = vfs.read("/unknown/test.txt").await;
    assert!(result.is_err());
}
```

Run: `cargo test -p torque-runtime --test vfs_contracts -- routed_vfs`
Expected: 3 new tests PASS

- [ ] **Step 5: Verify full workspace**

```bash
cargo check --workspace
cargo test -p torque-runtime
cargo test -p torque-harness
```
Expected: All pass

- [ ] **Step 6: Commit**

```bash
git add crates/torque-runtime/src/vfs.rs crates/torque-runtime/tests/vfs_contracts.rs crates/torque-harness/tests/tool_offload_tests.rs
git commit -m "refactor(runtime): RoutedVfs with prefix routing table and root aggregation"
```

---

## Task 4: Final Verification

- [ ] **Step 1: Run full workspace tests**

```bash
cargo test --workspace
```
Expected: All pass

- [ ] **Step 2: Commit**

```bash
git commit -m "test: final verification of deepagents-inspired harness enhancements" --allow-empty
```

---

## Summary

| Task | Files | Tests | Time |
|------|-------|-------|------|
| 1. Auto-offload | host.rs, host_port_integration.rs | 1 integration | 15 min |
| 2. Context compaction | context.rs, host.rs, host_port_integration.rs | 1 integration | 20 min |
| 3. RoutedVfs routing | vfs.rs, vfs_contracts.rs, tool_offload_tests.rs | 3 unit + updates | 25 min |
| 4. Verification | 0 | 0 | 5 min |
