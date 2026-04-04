# Phase 1: Core Skeleton

## Overview

**Goal**: Build the foundational infrastructure so the system can boot and basic data structures work.

**Scope**: Core types, DAG structures, database schema, and queue mechanism.

---

## Success Criteria

- [ ] All types can be serialized/deserialized
- [ ] DAG can be validated (no cycles, all references valid)
- [ ] Database tables can be created and migrated
- [ ] Queue supports SKIP LOCKED for concurrent workers

---

## Components

### 1. types crate

**Purpose**: Shared type definitions for all other crates.

**Types to define**:
- `Run`: id, tenant_id, status, instruction, failure_policy, timestamps
- `Node`: id, run_id, agent_type, instruction, tools, failure_policy, status, layer, etc.
- `Edge`: id, run_id, source_node, target_node
- `Artifact`: id, node_id, storage, location, size_bytes, content_type
- `QueueEntry`: id, run_id, node_id, status, priority, available_at, locked_at, locked_by
- `Tenant`: id, name, weight, max_concurrency, monthly_token_quota
- `AgentType`: id, name, system_prompt, tools, max_tokens, timeout_secs

**Files**:
- `crates/types/src/lib.rs` - Public exports
- `crates/types/src/run.rs` - Run type
- `crates/types/src/node.rs` - Node type
- `crates/types/src/edge.rs` - Edge type
- `crates/types/src/artifact.rs` - Artifact type
- `crates/types/src/queue.rs` - QueueEntry type
- `crates/types/src/tenant.rs` - Tenant type
- `crates/types/src/agent_type.rs` - AgentType type
- `crates/types/src/error.rs` - Common error types

**Tests**:
- `crates/types/tests/serialization.rs` - Test JSON/DB serialization

---

### 2. dag crate

**Purpose**: DAG data structures, topological sort, validation.

**Functions**:
- `validate_dag(nodes, edges)` → Result<(), DagError>
  - Checks for cycles using Kahn's algorithm
  - Validates all node references in edges exist
  - Ensures no orphan nodes (every node reachable from at least one root)
- `compute_layers(nodes, edges)` → HashMap<NodeId, Layer>
  - Root nodes (no upstream) → layer 0
  - Other nodes → max(upstream layers) + 1
- `topological_sort(nodes, edges)` → Vec<NodeId>

**Files**:
- `crates/dag/src/lib.rs`
- `crates/dag/src/validate.rs`
- `crates/dag/src/topo_sort.rs`
- `crates/dag/src/layers.rs`
- `crates/dag/src/error.rs`

**Tests**:
- `crates/dag/tests/validation.rs`
- `crates/dag/tests/topo_sort.rs`
- `crates/dag/tests/layers.rs`

---

### 3. db crate

**Purpose**: PostgreSQL operations via sqlx. All database access goes through this crate.

**Tables** (sqlx migrations):
- `001_create_tenants.sql`
- `002_create_agent_types.sql`
- `003_create_runs.sql`
- `004_create_nodes.sql`
- `005_create_edges.sql`
- `006_create_artifacts.sql`
- `007_create_queue.sql`

**Functions**:
- `runs::create(run) → Run`
- `runs::get(id) → Run`
- `runs::update_status(id, status) → ()`
- `nodes::create(node) → Node`
- `nodes::get(id) → Node`
- `nodes::update_status(id, status) → ()`
- `edges::create(edge) → Edge`
- `edges::get_by_run(run_id) → Vec<Edge>`
- `artifacts::create(artifact) → Artifact`
- `artifacts::get_by_node(node_id) → Vec<Artifact>`

**Files**:
- `crates/db/src/lib.rs`
- `crates/db/src/runs.rs`
- `crates/db/src/nodes.rs`
- `crates/db/src/edges.rs`
- `crates/db/src/artifacts.rs`
- `crates/db/src/queue.rs`
- `crates/db/src/migrations.rs`

**Tests**:
- `crates/db/tests/crud.rs` - Basic CRUD tests (requires test database)

---

### 4. queue crate

**Purpose**: Task queue operations using PostgreSQL SKIP LOCKED.

**Functions**:
- `enqueue(entry) → Result<Uuid, QueueError>` - INSERT ... ON CONFLICT DO NOTHING
- `dequeue(tenant_id, executor_id) → Result<Option<QueueEntry>, QueueError>` - SKIP LOCKED
- `complete(queue_id) → Result<(), QueueError>` - Mark DONE
- `reset_to_pending(queue_id) → Result<(), QueueError>` - Reset LOCKED → PENDING
- `get_waiting_count(tenant_id) → Result<u64, QueueError>`

**Files**:
- `crates/queue/src/lib.rs`
- `crates/queue/src/enqueue.rs`
- `crates/queue/src/dequeue.rs`
- `crates/queue/src/complete.rs`
- `crates/queue/src/error.rs`

**Tests**:
- `crates/queue/tests/concurrent_dequeue.rs` - Test SKIP LOCKED behavior

---

## Architecture

```
types (no dependencies)
    ↓
dag (depends on types) - pure algorithm, no IO
    ↓
db (depends on types) - PostgreSQL operations
    ↓
queue (depends on types, db) - queue operations
```

---

## Dependencies

**External crates**:
- `serde` + `serde_json` - Serialization
- `sqlx` - PostgreSQL async driver
- `thiserror` - Error types
- `uuid` - UUID generation
- `chrono` - DateTime handling

---

## Implementation Order

1. **types** - Define all shared types first
2. **dag** - Pure algorithm, no external dependencies
3. **db** - PostgreSQL schema and basic CRUD
4. **queue** - SKIP LOCKED queue operations

---

## Files to Create

```
crates/
├── types/
│   ├── src/
│   │   ├── lib.rs
│   │   ├── run.rs
│   │   ├── node.rs
│   │   ├── edge.rs
│   │   ├── artifact.rs
│   │   ├── queue.rs
│   │   ├── tenant.rs
│   │   ├── agent_type.rs
│   │   └── error.rs
│   ├── tests/
│   │   └── serialization.rs
│   └── Cargo.toml
├── dag/
│   ├── src/
│   │   ├── lib.rs
│   │   ├── validate.rs
│   │   ├── topo_sort.rs
│   │   ├── layers.rs
│   │   └── error.rs
│   ├── tests/
│   │   ├── validation.rs
│   │   ├── topo_sort.rs
│   │   └── layers.rs
│   └── Cargo.toml
├── db/
│   ├── src/
│   │   ├── lib.rs
│   │   ├── runs.rs
│   │   ├── nodes.rs
│   │   ├── edges.rs
│   │   ├── artifacts.rs
│   │   ├── queue.rs
│   │   └── migrations.rs
│   ├── migrations/
│   │   ├── 001_create_tenants.sql
│   │   ├── 002_create_agent_types.sql
│   │   ├── 003_create_runs.sql
│   │   ├── 004_create_nodes.sql
│   │   ├── 005_create_edges.sql
│   │   ├── 006_create_artifacts.sql
│   │   └── 007_create_queue.sql
│   ├── tests/
│   │   └── crud.rs
│   └── Cargo.toml
└── queue/
    ├── src/
    │   ├── lib.rs
    │   ├── enqueue.rs
    │   ├── dequeue.rs
    │   ├── complete.rs
    │   └── error.rs
    ├── tests/
    │   └── concurrent_dequeue.rs
    └── Cargo.toml
```

---

## Next Phase

Phase 2: Basic Execution - context-store, tool-executor, agent-runtime, executor
