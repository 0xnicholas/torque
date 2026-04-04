# Torque Observability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Production-ready observability with structured logging, tracing, and metrics.

**Prerequisites:** Phase 5 (Multi-Tenancy)

**Tech Stack:** Rust, tracing, tracing-subscriber, axum

---

## File Structure Overview

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

## Phase 1: Structured Logging (Day 1)

### Task 1: Add structured logging to executor

**Files:**
- Create: `crates/executor/src/logging.rs`

- [ ] **Step 1: Create logging.rs**

```rust
use tracing::{Span, info, error, warn};
use uuid::Uuid;

pub fn node_execution_span(
    run_id: Uuid,
    node_id: Uuid,
    tenant_id: Uuid,
    executor_id: &str,
) -> Span {
    tracing::info_span!(
        "node_execution",
        run_id = %run_id,
        node_id = %node_id,
        tenant_id = %tenant_id,
        executor_id = %executor_id
    )
}

pub fn log_node_started(run_id: Uuid, node_id: Uuid, agent_type: &str) {
    info!(
        run_id = %run_id,
        node_id = %node_id,
        agent_type = %agent_type,
        "Node execution started"
    );
}

pub fn log_node_completed(
    run_id: Uuid,
    node_id: Uuid,
    duration_ms: u64,
    tool_calls: u32,
    prompt_tokens: i64,
    completion_tokens: i64,
) {
    info!(
        run_id = %run_id,
        node_id = %node_id,
        duration_ms = %duration_ms,
        tool_calls = %tool_calls,
        prompt_tokens = %prompt_tokens,
        completion_tokens = %completion_tokens,
        "Node execution completed"
    );
}

pub fn log_node_failed(run_id: Uuid, node_id: Uuid, error: &str) {
    error!(
        run_id = %run_id,
        node_id = %node_id,
        error = %error,
        "Node execution failed"
    );
}

pub fn log_tool_call(
    run_id: Uuid,
    node_id: Uuid,
    tool: &str,
    duration_ms: u64,
    status: &str,
) {
    info!(
        run_id = %run_id,
        node_id = %node_id,
        tool = %tool,
        duration_ms = %duration_ms,
        status = %status,
        "Tool call executed"
    );
}

pub fn setup_logging() {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};
    
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));
    
    tracing_subscriber::registry()
        .with(fmt::layer().json())
        .with(filter)
        .init();
}
```

- [ ] **Step 2: Update executor main.rs to use logging**

```rust
use executor::logging::setup_logging;

#[tokio::main]
async fn main() -> Result<(), ExecutorError> {
    setup_logging();
    // ... rest of main
}
```

- [ ] **Step 3: Commit**

```bash
git add crates/executor/src/logging.rs
git commit -m "feat(executor): add structured logging"
```

---

### Task 2: Add structured logging to agent-runtime

**Files:**
- Create: `crates/agent-runtime/src/logging.rs`

- [ ] **Step 1: Create logging.rs**

```rust
use tracing::Span;
use uuid::Uuid;

pub fn llm_call_span(run_id: Uuid, node_id: Uuid) -> Span {
    tracing::info_span!(
        "llm_call",
        run_id = %run_id,
        node_id = %node_id
    )
}

pub fn log_prompt_tokens(run_id: Uuid, node_id: Uuid, tokens: i64) {
    tracing::info!(
        run_id = %run_id,
        node_id = %node_id,
        prompt_tokens = %tokens,
        "LLM prompt sent"
    );
}

pub fn log_completion_tokens(run_id: Uuid, node_id: Uuid, tokens: i64) {
    tracing::info!(
        run_id = %run_id,
        node_id = %node_id,
        completion_tokens = %tokens,
        "LLM completion received"
    );
}

pub fn log_checkpoint_created(run_id: Uuid, node_id: Uuid, checkpoint_id: Uuid) {
    tracing::info!(
        run_id = %run_id,
        node_id = %node_id,
        checkpoint_id = %checkpoint_id,
        "Checkpoint created"
    );
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/agent-runtime/src/logging.rs
git commit -m "feat(agent-runtime): add structured logging"
```

---

## Phase 2: Execution Traces (Day 2)

### Task 3: Integrate trace events into runtime

**Files:**
- Modify: `crates/agent-runtime/src/runtime.rs`

- [ ] **Step 1: Update runtime.rs to emit trace events**

```rust
use crate::logging::{log_prompt_tokens, log_completion_tokens, log_checkpoint_created};

impl AgentRuntime {
    pub async fn execute(&self, node: &mut Node) -> Result<String, AgentError> {
        let span = llm_call_span(node.run_id, node.id);
        let _guard = span.enter();
        
        // ... existing execution logic
        
        let response = self.llm.chat(messages.clone())
            .await
            .map_err(|e| AgentError::Llm(e.to_string()))?;
        
        log_prompt_tokens(node.run_id, node.id, response.usage.prompt_tokens);
        log_completion_tokens(node.run_id, node.id, response.usage.completion_tokens);
        
        // ... rest of execution
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/agent-runtime/src/runtime.rs
git commit -m "feat(agent-runtime): emit execution trace events"
```

---

## Phase 3: Usage Stats API (Day 3)

### Task 4: Implement usage statistics endpoint

**Files:**
- Create: `crates/planner/src/usage_handler.rs`

- [ ] **Step 1: Create usage_handler.rs**

```rust
use axum::{Json, extract::Query};
use serde::{Deserialize, Serialize};
use db::PgPool;
use chrono::{Year, Month, Datelike};

#[derive(Debug, Serialize)]
pub struct UsageStats {
    pub tenant_id: uuid::Uuid,
    pub period: String,
    pub total_runs: i64,
    pub total_nodes: i64,
    pub total_prompt_tokens: i64,
    pub total_completion_tokens: i64,
    pub runs_by_status: RunsByStatus,
}

#[derive(Debug, Serialize)]
pub struct RunsByStatus {
    pub done: i64,
    pub failed: i64,
    pub pending: i64,
    pub running: i64,
}

#[derive(Debug, Deserialize)]
pub struct UsageQuery {
    pub period: Option<String>,
}

pub async fn get_usage(
    Path(tenant_id): Path<uuid::Uuid>,
    Query(query): Query<UsageQuery>,
) -> Result<Json<UsageStats>, String> {
    let pool = get_pool().await;
    
    let period = query.period.unwrap_or_else(|| {
        let now = chrono::Utc::now();
        format!("{}-{:02}", now.year(), now.month())
    });
    
    let (year, month) = parse_period(&period)?;
    let start = chrono::NaiveDate::from_ymd_opt(year, month, 1)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap();
    let end = if month == 12 {
        chrono::NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        chrono::NaiveDate::from_ymd_opt(year, month + 1, 1)
    }
    .unwrap()
    .and_hms_opt(0, 0, 0)
    .unwrap();
    
    let (total_runs, total_nodes, total_prompt, total_completion) = sqlx::query_as!(
        (i64, i64, i64, i64),
        r#"
        SELECT 
            COUNT(DISTINCT r.id) as "total_runs",
            COUNT(n.id) as "total_nodes",
            COALESCE(SUM(nl.prompt_tokens), 0) as "total_prompt_tokens",
            COALESCE(SUM(nl.completion_tokens), 0) as "total_completion_tokens"
        FROM runs r
        JOIN nodes n ON n.run_id = r.id
        LEFT JOIN node_logs nl ON nl.node_id = n.id
        WHERE r.tenant_id = $1
          AND r.created_at >= $2
          AND r.created_at < $3
        "#,
        tenant_id,
        start,
        end
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| e.to_string())?;
    
    let runs_by_status = sqlx::query_as!(
        RunsByStatus,
        r#"
        SELECT 
            COUNT(*) FILTER (WHERE r.status = 'done') as "done",
            COUNT(*) FILTER (WHERE r.status = 'failed') as "failed",
            COUNT(*) FILTER (WHERE r.status = 'pending') as "pending",
            COUNT(*) FILTER (WHERE r.status = 'running') as "running"
        FROM runs r
        WHERE r.tenant_id = $1
          AND r.created_at >= $2
          AND r.created_at < $3
        "#,
        tenant_id,
        start,
        end
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| e.to_string())?;
    
    Ok(Json(UsageStats {
        tenant_id,
        period,
        total_runs,
        total_nodes,
        total_prompt_tokens: total_prompt,
        total_completion_tokens: total_completion,
        runs_by_status,
    }))
}

fn parse_period(period: &str) -> Result<(i32, u32), String> {
    let parts: Vec<&str> = period.split('-').collect();
    if parts.len() != 2 {
        return Err("Invalid period format".to_string());
    }
    
    let year: i32 = parts[0].parse().map_err(|_| "Invalid year".to_string())?;
    let month: u32 = parts[1].parse().map_err(|_| "Invalid month".to_string())?;
    
    if month < 1 || month > 12 {
        return Err("Month must be 1-12".to_string());
    }
    
    Ok((year, month))
}

async fn get_pool() -> PgPool {
    let database_url = std::env::var("DATABASE_URL").unwrap();
    PgPool::connect(&database_url).await.unwrap()
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/planner/src/usage_handler.rs
git commit -m "feat(planner): implement usage statistics API"
```

---

## Phase 4: Configurable Thresholds (Day 4)

### Task 5: Make ContextStore thresholds configurable

**Files:**
- Modify: `crates/context-store/src/store.rs`

- [ ] **Step 1: Update store.rs with configurable thresholds**

```rust
use std::env;

#[derive(Clone)]
pub struct ContextStoreConfig {
    pub small_threshold_bytes: usize,
    pub large_threshold_bytes: usize,
    pub redis_ttl_hours: u64,
    pub s3_endpoint: Option<String>,
}

impl Default for ContextStoreConfig {
    fn default() -> Self {
        Self {
            small_threshold_bytes: env::var("CONTEXT_STORE_SMALL_THRESHOLD")
                .unwrap_or_else(|_| "262144".to_string())
                .parse()
                .unwrap_or(256 * 1024),
            large_threshold_bytes: env::var("CONTEXT_STORE_LARGE_THRESHOLD")
                .unwrap_or_else(|_| "10485760".to_string())
                .parse()
                .unwrap_or(10 * 1024 * 1024),
            redis_ttl_hours: env::var("CONTEXT_STORE_REDIS_TTL_HOURS")
                .unwrap_or_else(|_| "24".to_string())
                .parse()
                .unwrap_or(24),
            s3_endpoint: env::var("S3_ENDPOINT_URL").ok(),
        }
    }
}

pub fn route_storage(size_bytes: usize, content_type: &str, config: &ContextStoreConfig) -> StorageType {
    if size_bytes < config.small_threshold_bytes && content_type.contains("json") {
        StorageType::Redis
    } else if size_bytes < config.large_threshold_bytes {
        StorageType::S3
    } else {
        StorageType::S3
    }
}
```

- [ ] **Step 2: Update Redis and S3 stores to use config**

```rust
pub struct RedisContextStore {
    conn: ConnectionManager,
    tenant_id: uuid::Uuid,
    ttl_secs: u64,
}

impl RedisContextStore {
    pub fn new(conn: ConnectionManager, tenant_id: uuid::Uuid, config: &ContextStoreConfig) -> Self {
        Self {
            conn,
            tenant_id,
            ttl_secs: config.redis_ttl_hours * 3600,
        }
    }
}
```

- [ ] **Step 3: Commit**

```bash
git add crates/context-store/src/store.rs
git commit -m "feat(context-store): make thresholds configurable via env vars"
```

---

## Phase 5: DAG Dry Run (Day 5)

### Task 6: Implement token estimation

**Files:**
- Create: `crates/planner/src/dry_run.rs`

- [ ] **Step 1: Create dry_run.rs**

```rust
use serde::{Deserialize, Serialize};
use types::{Node, AgentType};
use db::PgPool;

#[derive(Debug, Serialize)]
pub struct DryRunResult {
    pub estimated_prompt_tokens: i64,
    pub estimated_completion_tokens: i64,
    pub estimated_total_tokens: i64,
    pub estimated_cost_usd: f64,
    pub warnings: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct DryRunQuery {
    pub run_id: uuid::Uuid,
}

pub async fn dry_run(
    pool: &PgPool,
    run_id: uuid::Uuid,
) -> Result<DryRunResult, String> {
    let nodes = db::nodes::get_by_run(pool, run_id)
        .await
        .map_err(|e| e.to_string())?;
    
    let agent_types = db::agent_types::get_by_names(
        pool,
        &nodes.iter().map(|n| n.agent_type.clone()).collect::<Vec<_>>()
    )
    .await
    .map_err(|e| e.to_string())?;
    
    let mut total_prompt = 0i64;
    let mut total_completion = 0i64;
    let mut warnings = Vec::new();
    
    for node in &nodes {
        let agent_type = agent_types.iter()
            .find(|a| a.name == node.agent_type)
            .ok_or_else(|| format!("Agent type not found: {}", node.agent_type))?;
        
        let system_tokens = estimate_tokens(&agent_type.system_prompt);
        let instruction_tokens = estimate_tokens(&node.instruction);
        let prompt_for_node = system_tokens + instruction_tokens;
        
        if prompt_for_node > (agent_type.max_tokens as i64 * 80 / 100) {
            warnings.push(format!(
                "Node '{}' prompt may exceed context window (estimated {} tokens)",
                node.id,
                prompt_for_node
            ));
        }
        
        total_prompt += prompt_for_node;
        total_completion += agent_type.max_tokens as i64;
    }
    
    let price_per_1k_tokens = 0.00003;
    let estimated_cost = (total_prompt + total_completion) as f64 * price_per_1k_tokens / 1000.0;
    
    Ok(DryRunResult {
        estimated_prompt_tokens: total_prompt,
        estimated_completion_tokens: total_completion,
        estimated_total_tokens: total_prompt + total_completion,
        estimated_cost_usd: estimated_cost,
        warnings,
    })
}

fn estimate_tokens(text: &str) -> i64 {
    (text.len() as i64 / 4) + 1
}
```

- [ ] **Step 2: Add dry_run endpoint to router**

```rust
// In planner/src/main.rs or admin_handler.rs
.route("/runs/:run_id/dry-run", axum::routing::post(dry_run_handler))

async fn dry_run_handler(Path(run_id): Path<uuid::Uuid>) -> Result<Json<DryRunResult>, String> {
    let pool = get_pool().await;
    let result = dry_run(&pool, run_id).await?;
    Ok(Json(result))
}
```

- [ ] **Step 3: Commit**

```bash
git add crates/planner/src/dry_run.rs
git commit -m "feat(planner): implement DAG dry run with token estimation"
```

---

## Phase 6: Integration (Day 6)

### Task 7: Workspace verification

- [ ] **Step 1: Run cargo check --workspace**

```bash
cargo check --workspace
```

- [ ] **Step 2: Run cargo test --workspace**

```bash
cargo test --workspace
```

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat: integrate Phase 6 observability components"
```

---

## Summary

| Phase | Tasks | Duration |
|-------|-------|----------|
| Phase 1: Structured Logging | Tasks 1-2 | Day 1 |
| Phase 2: Execution Traces | Task 3 | Day 2 |
| Phase 3: Usage Stats API | Task 4 | Day 3 |
| Phase 4: Configurable Thresholds | Task 5 | Day 4 |
| Phase 5: DAG Dry Run | Task 6 | Day 5 |
| Phase 6: Integration | Task 7 | Day 6 |

**Total Estimated Time:** 6 days

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

**Plan complete and saved to** `docs/superpowers/plans/2025-04-05-torque-observability-plan.md`
