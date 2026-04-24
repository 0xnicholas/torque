# Deepagents-Inspired Harness Gap Closure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the smallest useful set of harness capabilities inspired by Deep Agents so Torque can plan work, offload context to files/artifacts, narrow delegation context, and gate risky file writes.

**Architecture:** Keep Torque's `torque-kernel` as the contract layer and implement the first usable version of these capabilities in `torque-harness`. Reuse the existing artifact, team shared state, policy, approval, and tool registration surfaces instead of introducing a second orchestration model or a new storage plane.

**Tech Stack:** Rust, Axum, SQLx/Postgres repositories, existing Torque artifact/policy/approval services, existing tool registry, existing tests under `crates/torque-harness/tests`

---

## File Map

### Existing files to modify

- `crates/torque-harness/src/tools/builtin.rs`
  Register and implement new built-in tools (`write_todos`, `read_todos`, file tools).
- `crates/torque-harness/src/tools/registry.rs`
  Expose the new tools through the tool registry used by the harness.
- `crates/torque-harness/src/tools/mod.rs`
  Shared tool types and exports.
- `crates/torque-harness/src/service/artifact.rs`
  Small helper paths for todo and offloaded tool-output artifacts.
- `crates/torque-harness/src/service/run.rs`
  Route large tool outputs through the offload flow and feed summaries into downstream execution.
- `crates/torque-harness/src/kernel_bridge/runtime.rs`
  Apply offload decisions during the LLM/tool loop and keep tool-result messages compact.
- `crates/torque-harness/src/service/delegation.rs`
  Build narrowed child execution packets instead of inheriting broad parent context.
- `crates/torque-harness/src/service/team/supervisor.rs`
  Sync todo summaries into shared team state and use narrowed delegation defaults.
- `crates/torque-harness/src/policy/evaluator.rs`
  Add file permission evaluation support.
- `crates/torque-harness/src/policy/decision.rs`
  Extend decisions with file-operation outcomes or reusable helper constructors.
- `crates/torque-harness/src/service/approval.rs`
  Add tool-operation approval creation/resume helpers for file writes.
- `crates/torque-kernel/src/task_packet.rs`
  Add optional compact summary / key facts fields while keeping the packet derived.

### New files to create

- `crates/torque-harness/src/tools/todos.rs`
  `TodoItem`, `TodoDocument`, and todo tool handlers.
- `crates/torque-harness/src/tools/vfs.rs`
  File-tool handlers and routing into the VFS backend layer.
- `crates/torque-harness/src/service/vfs.rs`
  `VfsBackend` trait, `RoutedVfs`, `ScratchBackend`, `WorkspaceBackend`.
- `crates/torque-harness/src/service/tool_offload.rs`
  Offload thresholds, summary generation, and artifact/scratch write helpers.
- `crates/torque-harness/src/service/context_compaction.rs`
  Compact-summary policy and helper interface for long-running sessions.
- `crates/torque-harness/src/service/delegation_packet.rs`
  Builder for minimal child task packets.
- `crates/torque-harness/src/policy/filesystem.rs`
  `FilesystemPermissionRule`, `FsAction`, matcher, and evaluation helpers.
- `crates/torque-harness/tests/todo_tools_tests.rs`
- `crates/torque-harness/tests/vfs_tools_tests.rs`
- `crates/torque-harness/tests/tool_offload_tests.rs`
- `crates/torque-harness/tests/context_compaction_tests.rs`
- `crates/torque-harness/tests/delegation_packet_tests.rs`
- `crates/torque-harness/tests/filesystem_permissions_tests.rs`
- `crates/torque-harness/tests/file_approval_flow_tests.rs`

### Files to consult while implementing

- `docs/learn.md`
- `AGENTS.md`
- `crates/torque-kernel/src/execution.rs`
- `crates/torque-kernel/src/delegation.rs`
- `crates/torque-harness/src/service/mod.rs`
- `crates/torque-harness/src/repository/artifact.rs`
- `crates/torque-harness/src/repository/approval.rs`

---

### Task 1: Add Todo Scratchpad Tools

**Files:**
- Create: `crates/torque-harness/src/tools/todos.rs`
- Modify: `crates/torque-harness/src/tools/builtin.rs`
- Modify: `crates/torque-harness/src/tools/registry.rs`
- Modify: `crates/torque-harness/src/service/artifact.rs`
- Test: `crates/torque-harness/tests/todo_tools_tests.rs`

- [ ] **Step 1: Write the failing tests**

Create tests for:
- `write_todos` creating a `todo_document` artifact for an instance scope
- `read_todos` returning the stored document
- updating one todo status without replacing the entire document

- [ ] **Step 2: Run only the todo tests and verify they fail**

Run: `cargo test -p torque-harness todo_tools_tests -- --nocapture`

Expected: compile or runtime failures because the todo tool module and handlers do not exist yet.

- [ ] **Step 3: Add todo data structures and handlers**

Implement:
- `TodoStatus`
- `TodoItem`
- `TodoDocument`
- `write_todos(scope, items, replace)`
- `read_todos(scope)`
- `update_todo(scope, id, status, notes)`

Store the serialized todo document in an artifact with a dedicated kind such as `todo_document`.

- [ ] **Step 4: Register the tools**

Wire the todo handlers into the built-in tool table and registry so the existing tool loop can invoke them by name.

- [ ] **Step 5: Re-run the todo tests and make them pass**

Run: `cargo test -p torque-harness todo_tools_tests -- --nocapture`

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/torque-harness/src/tools/todos.rs \
  crates/torque-harness/src/tools/builtin.rs \
  crates/torque-harness/src/tools/registry.rs \
  crates/torque-harness/src/service/artifact.rs \
  crates/torque-harness/tests/todo_tools_tests.rs
git commit -m "Add structured todo scratchpad tools"
```

---

### Task 2: Add Minimal VFS Backends and File Tools

**Files:**
- Create: `crates/torque-harness/src/service/vfs.rs`
- Create: `crates/torque-harness/src/tools/vfs.rs`
- Modify: `crates/torque-harness/src/tools/builtin.rs`
- Modify: `crates/torque-harness/src/tools/mod.rs`
- Modify: `crates/torque-harness/src/service/mod.rs`
- Test: `crates/torque-harness/tests/vfs_tools_tests.rs`

- [ ] **Step 1: Write the failing VFS tests**

Cover:
- listing `/scratch`
- writing and reading `/scratch/foo.txt`
- reading `/workspace/<known-file>`
- edit failure when `old_string` is not unique

- [ ] **Step 2: Run the VFS tests and verify they fail**

Run: `cargo test -p torque-harness vfs_tools_tests -- --nocapture`

Expected: FAIL because the VFS service and tool handlers do not exist.

- [ ] **Step 3: Implement the VFS service**

Add:
- `VfsBackend` trait
- `ScratchBackend`
- `WorkspaceBackend`
- `RoutedVfs`
- shared structs `FileInfo`, `EditResult`, `GrepMatch`

Keep the first version small:
- `/scratch/**` uses in-memory or existing context-store-backed scratch data
- `/workspace/**` is read/write under the repo root only

- [ ] **Step 4: Implement file tools**

Expose:
- `ls`
- `read_file`
- `write_file`
- `edit_file`
- `glob`
- `grep`

Reject unknown path prefixes early.

- [ ] **Step 5: Register the tools and service plumbing**

Make the VFS service reachable through the existing built-in tool registry.

- [ ] **Step 6: Re-run the VFS tests and make them pass**

Run: `cargo test -p torque-harness vfs_tools_tests -- --nocapture`

Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/torque-harness/src/service/vfs.rs \
  crates/torque-harness/src/tools/vfs.rs \
  crates/torque-harness/src/tools/builtin.rs \
  crates/torque-harness/src/tools/mod.rs \
  crates/torque-harness/src/service/mod.rs \
  crates/torque-harness/tests/vfs_tools_tests.rs
git commit -m "Add minimal routed VFS and file tools"
```

---

### Task 3: Offload Large Tool Results to Scratch or Artifacts

**Files:**
- Create: `crates/torque-harness/src/service/tool_offload.rs`
- Modify: `crates/torque-harness/src/kernel_bridge/runtime.rs`
- Modify: `crates/torque-harness/src/service/run.rs`
- Modify: `crates/torque-harness/src/service/artifact.rs`
- Modify: `crates/torque-harness/src/service/mod.rs`
- Test: `crates/torque-harness/tests/tool_offload_tests.rs`

- [ ] **Step 1: Write the failing offload tests**

Cover:
- small tool output remains inline
- medium output writes to `/scratch/tool-results/...`
- very large output creates an artifact and returns a compact summary with a ref

- [ ] **Step 2: Run the offload tests and verify they fail**

Run: `cargo test -p torque-harness tool_offload_tests -- --nocapture`

Expected: FAIL because offload policy does not exist.

- [ ] **Step 3: Implement offload policy**

Add a small service with:
- byte thresholds
- summary generation helper
- scratch/artifact destination selection
- a return shape that distinguishes inline vs offloaded payloads

- [ ] **Step 4: Integrate the policy into the runtime tool loop**

In the harness/kernel bridge:
- pass raw tool results through the offload service
- replace raw message injection with a compact message containing summary + reference

- [ ] **Step 5: Re-run the offload tests and make them pass**

Run: `cargo test -p torque-harness tool_offload_tests -- --nocapture`

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/torque-harness/src/service/tool_offload.rs \
  crates/torque-harness/src/kernel_bridge/runtime.rs \
  crates/torque-harness/src/service/run.rs \
  crates/torque-harness/src/service/artifact.rs \
  crates/torque-harness/src/service/mod.rs \
  crates/torque-harness/tests/tool_offload_tests.rs
git commit -m "Offload large tool outputs from model context"
```

---

### Task 4: Add Context Compaction and Derived Packet Summaries

**Files:**
- Create: `crates/torque-harness/src/service/context_compaction.rs`
- Modify: `crates/torque-kernel/src/task_packet.rs`
- Modify: `crates/torque-harness/src/kernel_bridge/runtime.rs`
- Modify: `crates/torque-harness/src/service/mod.rs`
- Test: `crates/torque-harness/tests/context_compaction_tests.rs`

- [ ] **Step 1: Write the failing compaction tests**

Cover:
- compaction triggering after a message-count or estimated-token threshold
- compact summaries entering the derived execution packet
- artifacts/refs remaining explicit after compaction

- [ ] **Step 2: Run the compaction tests and verify they fail**

Run: `cargo test -p torque-harness context_compaction_tests -- --nocapture`

Expected: FAIL because no compaction service or packet fields exist.

- [ ] **Step 3: Add compact-summary service**

Implement:
- compaction threshold policy
- compact summary object
- helper to extract key facts and preserved refs

- [ ] **Step 4: Extend `TaskPacket` carefully**

Add optional fields such as:
- `compact_summary`
- `key_facts`

Do not turn the packet into an authoritative state object.

- [ ] **Step 5: Integrate compaction into the runtime path**

Before building the execution-time message set:
- compact older messages when thresholds are met
- keep current task state, refs, and artifacts explicit

- [ ] **Step 6: Re-run the compaction tests and make them pass**

Run: `cargo test -p torque-harness context_compaction_tests -- --nocapture`

Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/torque-harness/src/service/context_compaction.rs \
  crates/torque-kernel/src/task_packet.rs \
  crates/torque-harness/src/kernel_bridge/runtime.rs \
  crates/torque-harness/src/service/mod.rs \
  crates/torque-harness/tests/context_compaction_tests.rs
git commit -m "Add context compaction and derived packet summaries"
```

---

### Task 5: Narrow Delegation Context by Default

**Files:**
- Create: `crates/torque-harness/src/service/delegation_packet.rs`
- Modify: `crates/torque-harness/src/service/delegation.rs`
- Modify: `crates/torque-harness/src/service/team/supervisor.rs`
- Modify: `crates/torque-harness/src/service/mod.rs`
- Test: `crates/torque-harness/tests/delegation_packet_tests.rs`

- [ ] **Step 1: Write the failing delegation tests**

Cover:
- child packets omit broad parent message history
- only selected artifacts and refs are forwarded
- child responses return concise summaries instead of transcripts

- [ ] **Step 2: Run the delegation tests and verify they fail**

Run: `cargo test -p torque-harness delegation_packet_tests -- --nocapture`

Expected: FAIL because delegation still relies on broader inherited context.

- [ ] **Step 3: Implement a delegation packet builder**

Build a small helper that accepts:
- delegated goal
- delegated instructions
- selected artifacts
- selected external refs
- selected constraints
- optional compact summary

and returns a narrow `TaskPacket`.

- [ ] **Step 4: Integrate the builder**

Use it in:
- regular delegation service
- team supervisor delegation path

Keep child return values compact and structured.

- [ ] **Step 5: Re-run the delegation tests and make them pass**

Run: `cargo test -p torque-harness delegation_packet_tests -- --nocapture`

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/torque-harness/src/service/delegation_packet.rs \
  crates/torque-harness/src/service/delegation.rs \
  crates/torque-harness/src/service/team/supervisor.rs \
  crates/torque-harness/src/service/mod.rs \
  crates/torque-harness/tests/delegation_packet_tests.rs
git commit -m "Narrow delegation context by default"
```

---

### Task 6: Add File Permissions and Approval Gates

**Files:**
- Create: `crates/torque-harness/src/policy/filesystem.rs`
- Modify: `crates/torque-harness/src/policy/evaluator.rs`
- Modify: `crates/torque-harness/src/policy/decision.rs`
- Modify: `crates/torque-harness/src/tools/vfs.rs`
- Modify: `crates/torque-harness/src/service/approval.rs`
- Test: `crates/torque-harness/tests/filesystem_permissions_tests.rs`
- Test: `crates/torque-harness/tests/file_approval_flow_tests.rs`

- [ ] **Step 1: Write the failing file-permission and approval tests**

Cover:
- read allowed on `/workspace/**`
- write denied on `/workspace/.git/**`
- write allowed on `/scratch/**`
- write on `/workspace/**` creating an approval request when policy requires it
- approved write resuming successfully

- [ ] **Step 2: Run the permission and approval tests and verify they fail**

Run: `cargo test -p torque-harness filesystem_permissions_tests file_approval_flow_tests -- --nocapture`

Expected: FAIL because filesystem policy and approval gating are not implemented.

- [ ] **Step 3: Add filesystem permission types and matcher**

Implement:
- `FsAction`
- `RuleEffect`
- `FilesystemPermissionRule`
- ordered matcher with default deny

- [ ] **Step 4: Enforce permissions in file tools**

Before any VFS operation:
- evaluate the path rule set
- fail fast on deny
- continue on allow

- [ ] **Step 5: Add approval gates for sensitive writes**

For write/edit on `/workspace/**`:
- create an approval request when policy says approval is required
- return an awaiting-approval result instead of performing the write immediately
- add resume helper(s) so the operation can continue after approval

- [ ] **Step 6: Re-run the permission and approval tests and make them pass**

Run: `cargo test -p torque-harness filesystem_permissions_tests file_approval_flow_tests -- --nocapture`

Expected: PASS

- [ ] **Step 7: Run the relevant package tests**

Run:
- `cargo test -p torque-harness -- --nocapture`
- `cargo test -p torque-kernel -- --nocapture`

Expected: PASS, or explicitly documented environment-related skips only.

- [ ] **Step 8: Commit**

```bash
git add crates/torque-harness/src/policy/filesystem.rs \
  crates/torque-harness/src/policy/evaluator.rs \
  crates/torque-harness/src/policy/decision.rs \
  crates/torque-harness/src/tools/vfs.rs \
  crates/torque-harness/src/service/approval.rs \
  crates/torque-harness/tests/filesystem_permissions_tests.rs \
  crates/torque-harness/tests/file_approval_flow_tests.rs
git commit -m "Add filesystem policy and approval gates"
```

---

## Integration Verification

- [ ] Run: `cargo test -p torque-kernel -- --nocapture`
- [ ] Run: `cargo test -p torque-harness -- --nocapture`
- [ ] Run one end-to-end manual flow against `torque-harness` covering:
  - todo creation
  - scratch write/read
  - workspace read
  - large tool output offload
  - narrowed delegation
  - workspace write requiring approval

Expected outcome:
- compact prompts
- artifacts or scratch refs for large results
- child work isolated from parent context
- dangerous writes blocked until approval

---

## Risks and Guardrails

- Do not collapse `Artifact`, `Memory`, and `ExternalContextRef` into one file-backed abstraction.
- Do not make `TaskPacket` authoritative.
- Keep the first VFS version constrained to `/scratch/**` and `/workspace/**`.
- Avoid adding new dependencies unless a missing primitive is impossible to reuse from the current codebase.
- Keep test scopes narrow and deterministic; avoid broad end-to-end tests until the helper layers are stable.

---

## Suggested Execution Order

1. Task 1: Todo Scratchpad
2. Task 2: Minimal VFS
3. Task 3: Tool Output Offload
4. Task 4: Context Compaction
5. Task 5: Narrow Delegation
6. Task 6: File Permissions + Approval
