# Phase 4: Planner + Reliability

## Overview

**Goal**: Complete DAG execution flow with Planner generating DAGs and full failure handling.

**Prerequisites**: Phase 2 (Basic Execution)

---

## Success Criteria

- [ ] Planner can call LLM to generate valid DAG
- [ ] DAG passes structural and semantic validation
- [ ] All four failure policies work correctly (retry, skip, fallback, abort)
- [ ] Executor crash recovery properly handles stale locks
- [ ] Planner crash recovery handles PLANNING timeout

---

## Components

### 1. planner crate

**Purpose**: Planner Service HTTP entry, LLM DAG generation.

**API Endpoints**:
```
POST /runs
  Body: { instruction: string, tenant_id: string }
  Response: { run_id: string, status: string }

GET /runs/{run_id}
  Response: { run, nodes[], edges[] }

POST /runs/{run_id}/retry
  Re-trigger PLANNING_FAILED Run
```

**Flow**:
1. Receive task instruction
2. Fetch registered AgentTypes for prompt context
3. Call LLM with instruction + AgentTypes
4. Parse LLM JSON output as DAG
5. Validate DAG structurally (dag crate)
6. Validate DAG semantically (agent types exist, tools valid)
7. Persist to PostgreSQL (runs, nodes, edges)
8. Compute layers via topological sort
9. Enqueue root nodes
10. Return run_id

**LLM Prompt Template**:
```
You are a task planner. Given the following instruction and available agent types,
generate a DAG of tasks.

Available Agent Types:
{agent_types_json}

Instruction:
{user_instruction}

Output format:
{{
  "nodes": [...],
  "edges": [...]
}}
```

**DAG Validation** (semantic):
- All `agent_type` values reference registered AgentTypes
- All `tools` arrays are subsets of corresponding AgentType's tools
- All `failure_policy` values are valid (retry/skip/fallback/abort)
- `fallback_agent_type` set only when failure_policy = fallback
- Fallback agent's tools ⊆ original agent's tools

**Files**:
- `crates/planner/src/lib.rs`
- `crates/planner/src/handler.rs` - HTTP handlers
- `crates/planner/src/llm.rs` - LLM client
- `crates/planner/src/validation.rs` - Semantic validation
- `crates/planner/src/error.rs`
- `Cargo.toml`
- `src/main.rs`

**Tests**:
- `crates/planner/tests/validation.rs`
- `crates/planner/tests/llm_generation.rs` - Mock LLM

---

### 2. Failure Policies (in agent-runtime)

**Retry Policy**:
```rust
impl FailureHandler {
    async fn handle_retry(node: &Node, attempt: u32) -> RetryDecision {
        if attempt >= node.max_retries.unwrap_or(2) {
            RetryDecision::Escalate  // Trigger parent policy
        } else {
            let delay = base_delay * 2^attempt;
            RetryDecision::RetryAfter(delay)
        }
    }
}
```

**Skip Policy**:
- Write empty Artifact
- Mark node status SKIPPED
- Enqueue downstream nodes

**Fallback Policy**:
- Look up `fallback_agent_type` from node config
- Verify fallback tools ⊆ original tools
- Re-execute with fallback agent type

**Abort Policy**:
- Mark run status FAILED
- Cancel all PENDING nodes (status → CANCELLED)
- Let RUNNING nodes complete, then stop

**Files** (modify):
- `crates/agent-runtime/src/runtime.rs` - Add failure handling
- `crates/agent-runtime/src/failure.rs` - New: FailureHandler

---

### 3. Enhanced Crash Recovery

**Executor Crash Recovery** (enhance Phase 2):
```rust
async fn recover_crashed_nodes(&self) {
    // 1. Find stale LOCKED queue entries (> 10 min)
    let stale = self.queue.find_stale_locked().await?;
    
    for entry in stale {
        // 2. Check Redis execution lock
        if self.redis_lock_exists(&entry).await? {
            // Original executor may still be running, wait for TTL
            continue;
        }
        
        // 3. Check if node has Artifact (completed but status not updated)
        let has_artifact = self.db.node_has_artifact(entry.node_id).await?;
        
        if has_artifact {
            // 4a. Mark node DONE, queue DONE, enqueue downstream
            self.mark_completed_cleanly(entry).await?;
        } else {
            // 4b. No artifact, reset to PENDING
            self.reset_to_pending(entry).await?;
        }
    }
}
```

**Planner Crash Recovery** (new):
```rust
async fn recover_planning_runs(&self) {
    // On startup, scan for PLANNING runs > 5 minutes old
    let stale = self.db.find_stale_planning_runs(Duration::minutes(5)).await?;
    
    for run in stale {
        self.db.update_run_status(run.id, PLANNING_FAILED)?;
    }
}
```

**Files** (modify):
- `crates/executor/src/crash_recovery.rs` - Enhanced
- `crates/planner/src/lib.rs` - Add recovery on startup

---

### 4. node_logs Integration

**Purpose**: Record complete execution trace for debugging and audit.

**node_logs Table Usage**:
```rust
struct NodeLog {
    id: Uuid,
    node_id: Uuid,
    run_id: Uuid,
    executor_id: String,
    started_at: DateTime<Utc>,
    completed_at: DateTime<Utc>,
    prompt_tokens: i64,
    completion_tokens: i64,
    tool_calls: Vec<ToolCallRecord>,  // [{tool, args_summary, duration_ms, status, error}, ...]
    status: String,
    error: Option<String>,
}
```

**Write on Node Completion**:
```rust
async fn complete_node(&self, node_id: Uuid, output: ArtifactPointer) -> Result<NodeLog> {
    // Calculate token usage from LLM responses
    let log = NodeLog {
        // ... populate fields
    };
    self.db.node_logs_create(log).await
}
```

**Files** (modify):
- `crates/db/src/node_logs.rs` - New module
- `crates/db/migrations/008_create_node_logs.sql` - New migration
- `crates/agent-runtime/src/runtime.rs` - Write logs

---

## Architecture

```
planner (HTTP API)
  └→ dag (validation)
       └→ db (persistence)
            └→ queue (enqueue root nodes)

executor
  ├→ agent-runtime (execution + failure handling)
  │    └→ tool-executor, context-store
  ├→ queue (dequeue/enqueue)
  ├→ db (state + node_logs)
  └→ crash_recovery
```

---

## Dependencies

**No new external crates** - Using existing tokio, sqlx, redis-rs, etc.

---

## Implementation Order

1. **planner** - HTTP handlers, LLM integration, validation
2. **Failure policies** - Add to agent-runtime
3. **Enhanced crash recovery** - Executor + Planner
4. **node_logs** - Integration into execution flow

---

## Files to Create/Modify

```
crates/
├── planner/                 # NEW
│   ├── src/
│   │   ├── lib.rs
│   │   ├── handler.rs
│   │   ├── llm.rs
│   │   ├── validation.rs
│   │   └── error.rs
│   ├── src/main.rs        # Binary
│   ├── tests/
│   │   ├── validation.rs
│   │   └── llm_generation.rs
│   └── Cargo.toml
├── agent-runtime/          # MODIFY
│   ├── src/
│   │   ├── runtime.rs     # Add failure handling
│   │   └── failure.rs     # NEW
│   └── ...
├── executor/               # MODIFY
│   ├── src/
│   │   └── crash_recovery.rs  # Enhanced
│   └── ...
├── db/                    # MODIFY
│   ├── src/
│   │   └── node_logs.rs   # NEW
│   └── migrations/
│       └── 008_create_node_logs.sql  # NEW
```

---

## Next Phase

Phase 5: Multi-Tenancy + Admin - Tenant isolation, quotas, Admin API
