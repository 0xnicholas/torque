# Torque Architecture

## Project Overview

Torque is a lightweight Agent Team System implemented in Rust. Core design philosophy: **LLM handles flow decisions, framework only provides execution guarantees**. No dependency on LangGraph, LangChain, or any third-party Agent frameworks.

---

## Architecture Overview

```
Planner Service
├── Receive user task
├── Call LLM to generate DAG (structured JSON)
├── DAG validation (structural + semantic)
├── Persist to PostgreSQL
└── Push root nodes to task queue

PostgreSQL Queue (SKIP LOCKED)

Executor Service (multi-instance, horizontal scaling)
├── Scheduler (weighted round-robin, quota check)
└── Worker Pool
    └── Agent Runtime
        ├── Checkpointer (NEW: state snapshots + crash recovery)
        ├── ContextManager (NEW: intelligent context optimization)
        ├── VirtualFileSystem (NEW: path-based file abstraction)
        ├── ContextStore (transparent routing Redis / S3)
        ├── LLM calls + tool call loop
        └── Tool Executor (permission check + execution log)

Storage Layer
├── Redis     Hot state, small results (< 256KB), concurrency counters, checkpoint snapshots
├── S3        Large results (>= 256KB), VFS large files
└── PostgreSQL DAG definitions, task states, artifact references, execution logs, usage records, checkpoint metadata, VFS metadata
```

---

## Core Design Decisions

### Execution Model

- **DAG**: Task graph is fully determined by Planner before execution, structure does not change during execution
- **Layer-based Concurrency**: All nodes in the same layer execute concurrently, wait between layers
- **Acyclic**: No cycles in graph. Retry handled by node failure policy, not in graph structure
- **Dependency Check**: Node checks all upstream nodes are DONE before execution; if not satisfied, skip and wait for next scheduling

### Message Protocol

- **Reference Passing**: Nodes don't pass data directly, only pass `task_id` references
- **Artifact Pointer**: Redis stores Artifact metadata (storage location, size, content type). Agent uses Pointer to decide whether to fetch from Redis or S3
- **Agent Unaware of Storage Layer**: All reads/writes go through `ContextStore` unified interface; storage routing is transparent to Agent

### Flow Decisions

- **LLM**: Responsible for task decomposition and DAG generation; framework does not hardcode any business logic
- **Framework**: Only handles concurrent scheduling, failure handling, state persistence, permission verification

---

## Three Core Modules (Enhancements)

### 1. Checkpointer

**Purpose**: State snapshots and crash recovery, supporting "time travel".

**Architecture**:
```
crates/checkpointer/
├── trait.rs           # Checkpointer trait + types
├── hybrid.rs         # HybridCheckpointer (PostgreSQL + Redis)
├── postgres.rs       # PostgreSQLCheckpointer
├── redis.rs          # RedisCheckpointer
└── error.rs         # Error types
```

**Storage Strategy**:
| Layer | Storage | Content |
|-------|---------|---------|
| Metadata | PostgreSQL | id, run_id, node_id, created_at, state_hash |
| State Snapshot | Redis | Full checkpoint state (TTL 24h) |

**Checkpoint Triggers**:
- After tool calls (every N calls)
- Explicit `create_checkpoint` tool call
- Configured interval (default 30s) - for liveness detection only

**Time Travel**: Resume execution from any historical checkpoint by selecting specific `checkpoint_id`.

### 2. VirtualFileSystem (VFS)

**Purpose**: Unified file abstraction - makes Agent code more natural with file paths instead of Artifact Pointers.

**Path Schema** (Hybrid Mode):
| Path Type | Schema | Access |
|-----------|--------|--------|
| Shared Workspace | `/{tid}/{rid}/workspace/*` | All nodes can read/write |
| Node Private | `/{tid}/{rid}/{nid}/*` | Only this node can write |
| Temp | `/{tid}/{rid}/temp/*` | Auto-cleanup after Run |

**VFS Shortcuts** (agents use these):
```
/workspace/file.json     → /{tenant_id}/{run_id}/workspace/file.json
/output/result.json     → /{tenant_id}/{run_id}/{node_id}/output/result.json
/temp/cache.json        → /{tenant_id}/{run_id}/temp/cache.json
```

**Concurrent Write Protection**: Advisory locking with Redis (`SET NX EX` with 30s TTL).

### 3. ContextManager

**Purpose**: Intelligent context optimization to prevent token overflow.

**Architecture**:
```
crates/agent-runtime/
└── context_mgr.rs       # ContextManager implementation
```

**Trigger Strategy** (Hybrid Mode):
- **Automatic**: Triggers when token usage exceeds 80% of max_tokens
- **Explicit**: Agent calls `context_compress` tool

**Compression Strategies**:
| Strategy | Behavior |
|----------|----------|
| `KeepLastN` | Keep last N messages, discard older |
| `SummarizeOlder` | Summarize older messages via LLM |
| `ExtractiveCompression` | Keep only tool calls and long outputs |

**Dual-Track Storage**:
| Layer | Storage | Purpose |
|-------|---------|---------|
| `full_history` | `node_logs` (permanent) | Audit, complete record |
| `compressed_context` | Memory only | Used for LLM prompts |
| `summary_chain` | `node_logs.tool_calls` | Audit trail of compressions |

---

## Service Details

### Planner Service

**Responsibilities**: Receive user tasks, generate and persist DAG.

**Flow**:
1. Create Run record, status `PLANNING`
2. Call LLM, request structured DAG JSON output
3. Structural validation (cycle detection, orphan nodes, edge reference validity)
4. Semantic validation (Agent type registered, tool set valid, instruction non-empty)
5. Validation passed → Write to PostgreSQL, Run status → `PENDING`
6. Push root nodes (no upstream dependencies) to task queue
7. Validation failed → Run status → `PLANNING_FAILED`, return detailed error

**Layer Computation**:
After validation passes, Planner performs topological sort and computes `layer` for each node:
- Root nodes (no upstream): `layer = 0`
- Other nodes: `layer = max(upstream nodes' layer) + 1`

`layer` is uniquely written by Planner, Executor only reads, does not recompute. Nodes with the same `layer` execute concurrently in Executor.

### Executor Service

**Responsibilities**: Consume tasks from queue, drive DAG execution.

**Scheduler Behavior**:
1. Traverse tenant list by weighted round-robin
2. Check if tenant's current concurrency exceeds quota (Redis counter)
3. Check if tenant's monthly token usage exceeds quota (Redis cache, sync from PostgreSQL every minute)
4. Fetch node from queue (PostgreSQL SKIP LOCKED)
5. Verify all upstream dependencies of node are DONE status
6. Distribute to Worker Pool

**Worker Pool**:
- Fixed-size async task pool, size based on machine resources
- Each Worker runs one Agent Runtime instance
- Worker returns to pool after completion, Scheduler continues distributing

**Horizontal Scaling**:
- Multiple Executor instances compete for queue consumption via SKIP LOCKED
- Each instance has unique `executor_id`, written to queue's `locked_by` field

**Crash Recovery Flow**:
1. Each Executor instance, on startup, periodically (every 30s) scans queue entries where `locked_at` > 10 minutes and status still `LOCKED`
2. For timed-out entries, check if corresponding node's Artifact has been written (query `artifacts` table)
3. **Has Artifact**: Node actually completed but status not updated; mark node status as `DONE`, queue entry as `DONE`, push downstream nodes
4. **No Artifact**: Node not completed; reset queue entry status to `PENDING`, clear `locked_at` and `locked_by`, set `available_at` to current time, reset node status to `PENDING`, wait for re-consumption
5. Re-queued nodes follow original failure policy (retry count not reset, counted as one failure)

### Agent Runtime

**Responsibilities**: Complete execution lifecycle for a single node.

**Agent Output Format Contract**:
```json
{
  "output": "Main result content, can be string, object, or array",
  "__requires_approval": false,
  "__metadata": {
    "summary": "Optional, within 50 chars, for downstream Agent quick judgment",
    "content_type": "text/plain | application/json | text/markdown"
  }
}
```

**Execution Flow**:
1. Read node definition from PostgreSQL (instruction, tools, constraints)
2. Batch pull upstream dependency results via ContextStore
3. Build LLM prompt (system prompt template + instruction + context)
4. Enter tool call loop:
   - LLM returns tool call → Tool Executor executes → result appended to context → continue
   - LLM returns final output → exit loop
5. Validate output format (must conform to Agent Output Format contract)
6. Write output to ContextStore
7. In single PostgreSQL transaction: update node status to DONE, query all downstream nodes where this node is upstream, for downstream nodes with all dependencies satisfied, execute `INSERT INTO queue ... ON CONFLICT (node_id) DO NOTHING`

### Tool Executor

**Responsibilities**: Actual tool call execution, built-in permission verification.

**Permission Verification Flow**:
1. Get `agent_type` and requested tool name from call context
2. Query Agent registry, check if this `agent_type`'s tool set includes the tool
3. Check if node definition's tool subset includes the tool (Orchestrator can narrow, cannot expand)
4. Verification failed returns error directly, does not execute
5. Each tool call records execution log (tool name, parameter summary, duration, status)

### ContextStore

**Responsibilities**: Unified result read/write interface, transparently routes to Redis or S3 for Agent.

**Write Routing Rules**:

| Condition | Storage | Redis Behavior |
|-----------|---------|----------------|
| Size < 256KB, JSON type | Redis direct | Store data body, TTL 24h |
| Size 256KB ~ 10MB | S3 stores data, Redis stores Pointer | TTL 24h |
| Size > 10MB | S3 stores data | Redis does not store |
| Binary files (images, PDFs, etc.) | S3 direct | Redis does not store |

**Artifact Pointer Structure**:
```json
{
  "task_id": "node-abc",
  "storage": "s3",
  "location": "s3://bucket/tenant/run/node",
  "size_bytes": 1048576,
  "content_type": "application/json",
  "created_at": "2026-01-01T00:00:00Z"
}
```

---

## Data Model

### PostgreSQL Tables

- **tenants**: id, name, weight, max_concurrency, monthly_token_quota, created_at
- **agent_types**: id, name, description, system_prompt, tools (JSONB), max_tokens, timeout_secs, created_at
- **runs**: id, tenant_id, status (PLANNING/PENDING/RUNNING/DONE/FAILED/PLANNING_FAILED), instruction, failure_policy, created_at, started_at, completed_at, error
- **nodes**: id, run_id, tenant_id, agent_type, fallback_agent_type, instruction, tools (JSONB), failure_policy, requires_approval, status (PENDING/RUNNING/DONE/FAILED/SKIPPED/PENDING_APPROVAL/CANCELLED), layer, created_at, started_at, completed_at, retry_count, error, executor_id
- **edges**: id, run_id, source_node, target_node
- **artifacts**: id, node_id, tenant_id, storage, location, size_bytes, content_type, created_at
- **node_logs**: id, node_id, run_id, tenant_id, executor_id, started_at, completed_at, prompt_tokens, completion_tokens, tool_calls (JSONB), status, error
- **queue**: id, tenant_id, run_id, node_id (UNIQUE), priority, status (PENDING/LOCKED/DONE), available_at, locked_at, locked_by, created_at
- **approval_requests**: id, node_id, run_id, tenant_id, reason, context (JSONB), status (PENDING/APPROVED/REJECTED), created_at, resolved_at, resolved_by
- **checkpoints**: id, run_id, node_id, tenant_id, state_hash, storage, location, created_at, expires_at
- **vfs_metadata**: id, run_id, node_id (NULL = shared), path, artifact_id, is_directory, created_at, modified_at

### Redis Key Schema

```
{tenant_id}:node:{node_id}:status        → PENDING / RUNNING / DONE / FAILED
{tenant_id}:node:{node_id}:artifact      → Artifact Pointer JSON (small result inline)
{tenant_id}:node:{node_id}:lock          → executor_id (execution lock)
{tenant_id}:run:{run_id}:concurrency     → Current concurrent node count (INCR/DECR)
{tenant_id}:token_usage:monthly          → Monthly cumulative token usage (cached, sync every minute)
{tenant_id}:checkpoint:{checkpoint_id}    → Checkpoint state snapshot (TTL 24h)
{tenant_id}:vfs:lock:{path}             → VFS advisory lock (TTL 30s)
```

### Two Locks

| Lock | Mechanism | Purpose | Scope |
|------|-----------|--------|-------|
| **Queue Lock** | PostgreSQL SKIP LOCKED | Prevent multiple Executors from fetching same queue entry | Queue consumption phase |
| **Execution Lock** | Redis `{tenant_id}:node:{node_id}:lock` | Prevent duplicate execution during crash recovery | Entire node execution |

---

## Failure Handling

### Four Failure Policies

| Policy | Behavior |
|--------|----------|
| **retry** | Exponential backoff retry, update queue's `available_at` for delayed re-enqueue. Default max 2 retries. Backoff formula: `base_delay * 2^retry_count` |
| **skip** | Write empty Artifact, mark node SKIPPED, downstream nodes continue. Agent system prompt should note this scenario. |
| **fallback** | Node pre-configured with fallback `agent_type`. On failure, re-execute same node with fallback type. Fallback's tool set must be subset of original's. |
| **abort** | Mark entire Run FAILED, cancel all PENDING nodes, RUNNING nodes wait for completion then stop |

### Priority

Node-level `failure_policy` > Run-level `failure_policy` > Global default (retry 2 times then abort)

---

## Human Intervention

### Trigger Methods

- **Rule-based**: Node definition has `requires_approval: true`, node automatically pauses before execution, status → `PENDING_APPROVAL`, create `approval_requests` record
- **Agent-initiated**: Agent outputs special signal (`"__requires_approval": true`), Executor recognizes, pauses node, creates approval request

### Approval Flow

1. Node status → `PENDING_APPROVAL`
2. Create `approval_requests` record in PostgreSQL (not Redis, avoid TTL loss)
3. Wait for external system (Webhook, admin console, etc.) to call approval API
4. Approval passes → node re-pushed to queue to continue
5. Approval rejected → trigger node's failure policy

---

## Multi-Tenancy

### Isolation Layers

- **Data Isolation**: All tables have `tenant_id`, all queries enforce tenant filtering, prohibit cross-tenant queries. Redis keys prefixed with `tenant_id`, S3 paths prefixed with `tenant_id`.
- **Execution Isolation**: Scheduler weighted round-robin, weight configured via `tenants.weight`. Before fetching tasks, check if `{tenant_id}:run:{run_id}:concurrency` exceeds `tenants.max_concurrency`.
- **Resource Quota**: Monthly token quota stored in `tenants.monthly_token_quota`. Real-time usage aggregated from `token_usage` table, cached in Redis `{tenant_id}:token_usage:monthly`, sync every minute. Tenants exceeding quota skip tasks, do not abort running nodes.

---

## Key APIs

### Planner API

```
POST /runs
  Body: { instruction: string, tenant_id: string }
  Response: { run_id: string, status: string }

GET /runs/{run_id}
  Response: { run, nodes[], edges[] }

POST /runs/{run_id}/retry
  Re-trigger PLANNING_FAILED Run
```

### Approval API

```
GET /approvals?tenant_id={id}&status=PENDING
  Response: [{ approval_id, node_id, run_id, reason, context }]

POST /approvals/{approval_id}/approve
POST /approvals/{approval_id}/reject
  Body: { resolved_by: string }
```

### Admin API

```
GET /agents                     List registered Agent types
POST /agents                    Register new Agent type
PUT /agents/{name}              Update Agent type config

GET /tenants/{id}/usage         Query tenant usage
PUT /tenants/{id}/quota         Update quota config
```

---

## Crate Structure

```
agent-team/
├── crates/
│   ├── types/              Shared type definitions (Run, Node, Artifact, etc.)
│   ├── dag/                DAG data structures, validation, topological sort (depends on types)
│   ├── db/                 PostgreSQL operations, sqlx (depends on types)
│   ├── checkpointer/       State snapshots and crash recovery (depends on types, db)
│   ├── queue/              Task queue SKIP LOCKED implementation (depends on types, db)
│   ├── context-store/      ContextStore trait and Redis/S3 implementation (depends on types)
│   ├── tool-executor/      Tool call execution and permission verification (depends on types, db)
│   ├── agent-runtime/      Agent execution lifecycle (depends on types, db, context-store, tool-executor)
│   ├── executor/           Executor Service entry, Scheduler + Worker Pool (depends on agent-runtime, queue, db, context-store)
│   └── planner/            Planner Service entry, HTTP service (depends on dag, db, queue, types)
├── Cargo.toml
└── AGENTS.md
```

**Dependency Rules**:
- `types` depends on no internal crates, base of all crates
- `dag` only depends on `types`, pure algorithm crate, no IO, easy for unit testing
- `db` only depends on `types`, encapsulates all PostgreSQL operations, other crates don't directly use sqlx
- `planner` and `executor` are two independent binary entry points, not dependent on each other
- **Prohibited**: Circular dependencies

---

## Environment Variables

```
# Database
DATABASE_URL              PostgreSQL connection string

# Redis
REDIS_URL                 Redis connection string

# S3
S3_BUCKET                 Artifact storage bucket name
S3_REGION                 AWS region
AWS_ACCESS_KEY_ID          S3 access key
AWS_SECRET_ACCESS_KEY      S3 secret key
S3_ENDPOINT_URL           Optional custom endpoint (for MinIO, etc.)

# LLM
LLM_BASE_URL              OpenAI compatible API base URL, default https://api.openai.com/v1
LLM_API_KEY               API key
LLM_PLANNER_MODEL         Model for Planner, default gpt-4o
LLM_AGENT_MODEL           Model for Agent Runtime, default gpt-4o-mini

# Executor
EXECUTOR_ID               Current Executor instance unique ID, recommend hostname + PID
EXECUTOR_WORKER_POOL_SIZE Worker Pool size, default 10
EXECUTOR_LOCK_TIMEOUT_SECS Execution lock timeout threshold, default 600 (10 minutes)

# Planner
PLANNER_PLANNING_TIMEOUT_SECS PLANNING status timeout threshold, default 300 (5 minutes)
```

---

## Tech Stack

| Purpose | Technology |
|---------|------------|
| Async Runtime | tokio |
| HTTP Service | axum |
| PostgreSQL | sqlx |
| Redis | redis-rs (async) |
| S3 | aws-sdk-s3 |
| LLM Calls | async-openai or direct reqwest |
| Serialization | serde + serde_json |
| Error Handling | thiserror + anyhow |
| Logging | tracing + tracing-subscriber |
| Configuration | config |

---

## Development Phases

### Phase 1: Core Execution (Goal: system can run)
- [ ] `types` crate: Run, Node, Edge, Artifact type definitions
- [ ] `dag` crate: DAG data structures, topological sort, validation logic
- [ ] `db` crate: Basic CRUD, runs / nodes / edges / artifacts tables
- [ ] `queue` crate: PostgreSQL SKIP LOCKED enqueue/dequeue/complete
- [ ] `context-store` crate: Redis + S3 routing, ContextStore trait
- [ ] `agent-runtime` crate: Single node execution, LLM calls, tool call loop
- [ ] `executor` crate: Scheduler + Worker Pool, basic layer-based concurrency
- [ ] `planner` crate: LLM calls, DAG validation, Run creation

### Phase 2: Reliability (Goal: production ready)
- [ ] Full failure policy implementation (retry backoff, skip, fallback, abort)
- [ ] Tool Executor permission verification
- [ ] Executor crash recovery (timeout lock takeover)
- [ ] Planner crash recovery (PLANNING timeout detection)
- [ ] Human intervention (PENDING_APPROVAL state machine)
- [ ] Approval API

### Phase 3: Multi-Tenancy (Goal: support multiple users)
- [ ] Tenant data isolation (all queries add tenant_id filter)
- [ ] Scheduler weighted round-robin
- [ ] Concurrency quota (Redis counter)
- [ ] Token usage recording and monthly quota checking
- [ ] Admin API

### Phase 4: Observability and Optimization
- [ ] Structured logging (tracing, correlated by run_id / node_id)
- [ ] Complete `node_logs` recording
- [ ] Usage statistics API
- [ ] Configurable ContextStore thresholds
- [ ] DAG dry run mode (estimate token consumption)
