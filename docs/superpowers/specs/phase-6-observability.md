# Phase 6: Observability

## Overview

**Goal**: Production-ready observability with structured logging, tracing, and metrics.

**Prerequisites**: Phase 5 (Multi-Tenancy)

---

## Success Criteria

- [ ] All operations logged with run_id/node_id correlation
- [ ] Execution traces viewable per run/node
- [ ] Token usage API returns accurate aggregated data
- [ ] ContextStore thresholds configurable
- [ ] DAG dry run mode estimates token consumption

---

## Components

### 1. Structured Logging

**Correlation IDs**:
```
run_id, node_id, tenant_id
```

**Log Format**:
```json
{
  "timestamp": "2025-04-05T12:00:00Z",
  "level": "INFO",
  "message": "Node execution completed",
  "run_id": "uuid",
  "node_id": "uuid", 
  "tenant_id": "uuid",
  "executor_id": "executor-1",
  "duration_ms": 1234,
  "tool_calls": 5,
  "token_usage": { "prompt": 1000, "completion": 500 }
}
```

**Implementation**:
```rust
use tracing::{info, error, span, Level};
use tracing_subscriber::fmt::format::json;

fn node_execution_span(run_id: Uuid, node_id: Uuid, tenant_id: Uuid) -> Span {
    span!(
        Level: INFO,
        "node_execution",
        run_id = %run_id,
        node_id = %node_id,
        tenant_id = %tenant_id
    )
}
```

**Files** (new):
- `crates/executor/src/logging.rs` - Structured logging setup
- `crates/agent-runtime/src/logging.rs`

---

### 2. Execution Traces

**Trace Structure**:
```rust
struct ExecutionTrace {
    run_id: Uuid,
    node_id: Uuid,
    events: Vec<TraceEvent>,
}

enum TraceEvent {
    Started { timestamp: DateTime<Utc> },
    ToolCall { tool: String, args_summary: String, duration_ms: u64 },
    ToolResult { success: bool, error: Option<String> },
    Checkpoint { checkpoint_id: Uuid },
    Message { role: String, tokens: i64 },
    Completed { duration_ms: u64, output_size: i64 },
    Failed { error: String },
}
```

**Storage**: In `node_logs.tool_calls` as JSON array

**Files** (modify):
- `crates/agent-runtime/src/runtime.rs` - Emit trace events

---

### 3. Usage Statistics API

**Endpoint**:
```
GET /tenants/{id}/usage?period=monthly
Response: {
  "tenant_id": "uuid",
  "period": "2025-04",
  "total_runs": 100,
  "total_nodes": 500,
  "total_prompt_tokens": 1000000,
  "total_completion_tokens": 500000,
  "runs_by_status": { "done": 90, "failed": 5, "pending": 5 }
}
```

**Implementation**:
```rust
async fn get_usage_stats(
    pool: &PgPool,
    tenant_id: Uuid,
    period: YearMonth,
) -> Result<UsageStats> {
    let stats = sqlx::query_as!(
        UsageStats,
        r#"
        SELECT 
            COUNT(DISTINCT run_id) as "total_runs",
            COUNT(*) as "total_nodes",
            SUM(prompt_tokens) as "total_prompt_tokens",
            SUM(completion_tokens) as "total_completion_tokens"
        FROM node_logs
        WHERE tenant_id = $1 
          AND recorded_at >= $2
          AND recorded_at < $3
        "#,
        tenant_id,
        period.start(),
        period.end()
    )
    .fetch_one(pool)
    .await?;
    
    Ok(stats)
}
```

**Files**:
- `crates/planner/src/usage_handler.rs` - NEW: Usage API handler

---

### 4. Configurable ContextStore Thresholds

**Current**: Hardcoded at 256KB
**Enhanced**: Environment variables or config file

```rust
#[derive(Clone)]
pub struct ContextStoreConfig {
    pub small_threshold_bytes: usize,      // Default: 256 * 1024
    pub large_threshold_bytes: usize,       // Default: 10 * 1024 * 1024
    pub redis_ttl_hours: u64,              // Default: 24
    pub s3_endpoint: Option<String>,        // For MinIO compatibility
}

impl Default for ContextStoreConfig {
    fn default() -> Self {
        Self {
            small_threshold_bytes: 256 * 1024,
            large_threshold_bytes: 10 * 1024 * 1024,
            redis_ttl_hours: 24,
            s3_endpoint: None,
        }
    }
}
```

**Environment Variables**:
```
CONTEXT_STORE_SMALL_THRESHOLD=262144
CONTEXT_STORE_LARGE_THRESHOLD=10485760
CONTEXT_STORE_REDIS_TTL_HOURS=24
```

**Files** (modify):
- `crates/context-store/src/store.rs` - Use configurable thresholds

---

### 5. DAG Dry Run Mode

**Purpose**: Estimate token consumption before execution.

**Endpoint**:
```
POST /runs/{run_id}/dry-run
Response: {
  "estimated_prompt_tokens": 5000,
  "estimated_completion_tokens": 2000,
  "estimated_total_tokens": 7000,
  "estimated_cost_usd": 0.14,
  "warnings": ["Node 'researcher' may exceed context window"]
}
```

**Estimation Logic**:
```rust
fn estimate_tokens(node: &Node, context: &EstimationContext) -> TokenEstimate {
    let system_prompt_tokens = estimate_tokens_from_text(&node.agent_type.system_prompt);
    let instruction_tokens = estimate_tokens_from_text(&node.instruction);
    
    // Sum of upstream artifact sizes (rough estimate)
    let upstream_tokens = node.depends_on
        .iter()
        .map(|id| context.get_artifact_size(id))
        .sum();
    
    TokenEstimate {
        prompt: system_prompt_tokens + instruction_tokens + upstream_tokens,
        completion: node.agent_type.max_tokens,
    }
}
```

**Files**:
- `crates/planner/src/dry_run.rs` - NEW: Dry run estimation

---

## Implementation Order

1. **Structured logging** - Add tracing to all components
2. **Execution traces** - Integrate into agent-runtime
3. **Usage stats API** - Aggregation queries + endpoint
4. **Configurable thresholds** - Environment variable config
5. **DAG dry run** - Token estimation logic

---

## Files to Create/Modify

```
crates/
├── executor/                  # MODIFY
│   └── src/
│       └── logging.rs       # NEW: Structured logging
│
├── agent-runtime/            # MODIFY
│   └── src/
│       └── logging.rs       # NEW: Structured logging
│
├── context-store/           # MODIFY
│   └── src/
│       └── store.rs         # Configurable thresholds
│
├── planner/                 # MODIFY
│   ├── src/
│   │   ├── usage_handler.rs # NEW: Usage stats API
│   │   └── dry_run.rs       # NEW: Token estimation
│   └── ...
│
└── db/                      # MODIFY (if needed for stats queries)
```

---

## Success Criteria Summary

| Feature | Metric |
|---------|--------|
| Structured Logging | All log entries include run_id, node_id, tenant_id |
| Execution Traces | node_logs contains complete event history |
| Usage Stats API | Accurate aggregation within 1 minute of query |
| Configurable Thresholds | All threshold values from env vars |
| DAG Dry Run | Token estimates within ±20% of actual |

---

## Phase 1-6 Summary

| Phase | Focus | Key Deliverables |
|-------|-------|------------------|
| 1 | Core Skeleton | types, dag, db, queue |
| 2 | Basic Execution | context-store, tool-executor, agent-runtime, executor |
| 3 | Core Enhancements | Checkpointer, VFS, ContextManager |
| 4 | Planner + Reliability | LLM DAG generation, failure policies, crash recovery |
| 5 | Multi-Tenancy | Tenant isolation, quotas, Admin API |
| 6 | Observability | Structured logging, tracing, metrics |

**Total Estimated Time**: ~16 weeks (4 weeks per phase, parallelized where possible)
