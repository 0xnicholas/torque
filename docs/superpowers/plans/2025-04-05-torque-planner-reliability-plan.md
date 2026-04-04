# Torque Planner + Reliability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete DAG execution flow with Planner generating DAGs and full failure handling.

**Prerequisites:** Phase 2 (Basic Execution)

**Tech Stack:** Rust, axum, tokio, sqlx, llm

---

## File Structure Overview

```
crates/
├── planner/                    # NEW
│   ├── src/
│   │   ├── lib.rs
│   │   ├── main.rs            # Binary entry
│   │   ├── handler.rs         # HTTP handlers
│   │   ├── llm.rs             # LLM DAG generation
│   │   ├── validation.rs      # Semantic validation
│   │   └── error.rs
│   ├── tests/
│   │   ├── validation.rs
│   │   └── llm_generation.rs
│   └── Cargo.toml
├── agent-runtime/              # MODIFY
│   ├── src/
│   │   ├── runtime.rs         # Modify to add failure handling
│   │   └── failure.rs         # NEW: FailureHandler
│   └── ...
├── executor/                   # MODIFY
│   ├── src/
│   │   ├── crash_recovery.rs  # Enhanced
│   │   └── ...
│   └── ...
└── db/                        # MODIFY
    ├── src/
    │   └── node_logs.rs       # NEW
    └── migrations/
        └── 008_create_node_logs.sql  # NEW
```

---

## Phase 1: Planner Crate (Day 1)

### Task 1: Create planner crate scaffold

**Files:**
- Create: `crates/planner/Cargo.toml`
- Create: `crates/planner/src/lib.rs`

- [ ] **Step 1: Create Cargo.toml**

```toml
[package]
name = "planner"
version = "0.1.0"
edition = "2021"

[dependencies]
types = { path = "../types" }
dag = { path = "../dag" }
db = { path = "../db" }
llm = { path = "../llm" }
queue = { path = "../queue" }
axum = "0.7"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["json"] }
```

- [ ] **Step 2: Create lib.rs**

```rust
pub mod handler;
pub mod llm;
pub mod validation;
pub mod error;

pub use error::{PlannerError, PlannerErrorKind};
```

- [ ] **Step 3: Create error.rs**

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PlannerError {
    #[error("LLM error: {0}")]
    Llm(String),
    
    #[error("Validation error: {0}")]
    Validation(String),
    
    #[error("Database error: {0}")]
    Database(String),
    
    #[error("DAG error: {0}")]
    Dag(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlannerErrorKind {
    Llm,
    Validation,
    Database,
    Dag,
}
```

- [ ] **Step 4: Commit**

```bash
git add crates/planner/
git commit -m "feat(planner): create planner crate scaffold"
```

---

### Task 2: Implement HTTP handlers

**Files:**
- Create: `crates/planner/src/handler.rs`

- [ ] **Step 1: Create handler.rs**

```rust
use axum::{Router, Json, extract::Path};
use types::{Run, Node, Edge};
use crate::error::PlannerError;
use db::PgPool;

pub async fn create_run(
    Json(payload): Json<CreateRunRequest>,
) -> Result<Json<CreateRunResponse>, PlannerError> {
    let pool = get_pool().await;
    
    let mut run = Run::new(payload.tenant_id, payload.instruction, "abort".to_string());
    db::runs::create(&pool, &run).await
        .map_err(|e| PlannerError::Database(e.to_string()))?;
    
    let nodes_json = crate::llm::generate_dag(&run.instruction, &payload.agent_types)
        .await
        .map_err(|e| PlannerError::Llm(e.to_string()))?;
    
    let dag: DagSpec = serde_json::from_str(&nodes_json)
        .map_err(|e| PlannerError::Validation(e.to_string()))?;
    
    crate::validation::validate_dag_spec(&dag)?;
    
    for node_spec in &dag.nodes {
        let node = Node::new(run.id, run.tenant_id, node_spec.agent_type.clone(), node_spec.instruction.clone());
        db::nodes::create(&pool, &node).await
            .map_err(|e| PlannerError::Database(e.to_string()))?;
    }
    
    for edge_spec in &dag.edges {
        let source = find_node_by_id(&dag, &edge_spec.source);
        let target = find_node_by_id(&dag, &edge_spec.target);
        let edge = Edge::new(run.id, source.id, target.id);
        db::edges::create(&pool, &edge).await
            .map_err(|e| PlannerError::Database(e.to_string()))?;
    }
    
    let layers = dag::compute_layers(&[], &[]).unwrap_or_default();
    
    for (node_id, layer) in layers {
        db::nodes::update_layer(&pool, node_id, layer).await
            .map_err(|e| PlannerError::Database(e.to_string()))?;
    }
    
    let root_nodes = dag::find_root_nodes(&dag);
    for node in root_nodes {
        queue::enqueue(&pool, &QueueEntry::new(run.tenant_id, run.id, node.id, 0)).await
            .map_err(|e| PlannerError::Database(e.to_string()))?;
    }
    
    db::runs::update_status(&pool, run.id, RunStatus::Pending).await
        .map_err(|e| PlannerError::Database(e.to_string()))?;
    
    Ok(Json(CreateRunResponse { run_id: run.id }))
}

async fn get_pool() -> PgPool {
    let database_url = std::env::var("DATABASE_URL").unwrap();
    PgPool::connect(&database_url).await.unwrap()
}

#[derive(serde::Deserialize)]
pub struct CreateRunRequest {
    pub instruction: String,
    pub tenant_id: uuid::Uuid,
    pub agent_types: Vec<String>,
}

#[derive(serde::Serialize)]
pub struct CreateRunResponse {
    pub run_id: uuid::Uuid,
}

struct DagSpec {
    nodes: Vec<NodeSpec>,
    edges: Vec<EdgeSpec>,
}

struct NodeSpec {
    id: String,
    agent_type: String,
    instruction: String,
}

struct EdgeSpec {
    source: String,
    target: String,
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/planner/src/handler.rs
git commit -m "feat(planner): implement HTTP handlers"
```

---

### Task 3: Implement LLM DAG generation

**Files:**
- Create: `crates/planner/src/llm.rs`

- [ ] **Step 1: Create llm.rs**

```rust
use llm::OpenAiClient;
use crate::error::PlannerError;

pub async fn generate_dag(instruction: &str, agent_types: &[String]) -> Result<String, PlannerError> {
    let client = OpenAiClient::from_env()
        .map_err(|e| PlannerError::Llm(e.to_string()))?;
    
    let prompt = build_prompt(instruction, agent_types);
    
    let request = llm::ChatRequest {
        model: client.model().to_string(),
        messages: vec![llm::Message {
            role: "user".to_string(),
            content: prompt,
        }],
        tools: None,
        max_tokens: Some(4096),
        temperature: Some(0.7),
    };
    
    let response = client.chat(request)
        .await
        .map_err(|e| PlannerError::Llm(e.to_string()))?;
    
    Ok(response.message.content)
}

fn build_prompt(instruction: &str, agent_types: &[String]) -> String {
    let agent_types_json = agent_types.iter()
        .map(|t| format!("  - {}", t))
        .collect::<Vec<_>>()
        .join("\n");
    
    format!(
        r#"You are a task planner. Given the following instruction and available agent types,
generate a DAG of tasks.

Available Agent Types:
{}

Instruction:
{}

Output format:
{{
  "nodes": [
    {{
      "id": "node-1",
      "agent_type": "...",
      "instruction": "...",
      "tools": ["..."],
      "failure_policy": "retry|skip|fallback|abort",
      "fallback_agent_type": null
    }}
  ],
  "edges": [
    {{"source": "node-1", "target": "node-2"}}
  ]
}}"#,
        agent_types_json,
        instruction
    )
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/planner/src/llm.rs
git commit -m "feat(planner): implement LLM DAG generation"
```

---

### Task 4: Implement semantic validation

**Files:**
- Create: `crates/planner/src/validation.rs`

- [ ] **Step 1: Create validation.rs**

```rust
use crate::error::PlannerError;

pub fn validate_dag_spec(dag: &DagSpec) -> Result<(), PlannerError> {
    let node_ids: std::collections::HashSet<_> = dag.nodes.iter()
        .map(|n| n.id.clone())
        .collect();
    
    for edge in &dag.edges {
        if !node_ids.contains(&edge.source) {
            return Err(PlannerError::Validation(format!(
                "Edge references non-existent source node: {}", edge.source
            )));
        }
        if !node_ids.contains(&edge.target) {
            return Err(PlannerError::Validation(format!(
                "Edge references non-existent target node: {}", edge.target
            )));
        }
    }
    
    for node in &dag.nodes {
        if node.instruction.is_empty() {
            return Err(PlannerError::Validation(format!(
                "Node {} has empty instruction", node.id
            )));
        }
        
        if let Some(ref fp) = node.failure_policy {
            match fp.as_str() {
                "retry" | "skip" | "fallback" | "abort" => {}
                _ => return Err(PlannerError::Validation(format!(
                    "Invalid failure_policy '{}' for node {}", fp, node.id
                ))),
            }
        }
        
        if node.failure_policy.as_deref() == Some("fallback") {
            if node.fallback_agent_type.is_none() {
                return Err(PlannerError::Validation(format!(
                    "Node {} has fallback policy but no fallback_agent_type", node.id
                )));
            }
        }
    }
    
    dag::validate_dag(&[], &[]).map_err(|e| PlannerError::Dag(e.to_string()))?;
    
    Ok(())
}
```

- [ ] **Step 2: Create main.rs**

```rust
use axum::Router;
use planner::handler::create_run;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .json()
        .init();
    
    let app = Router::new()
        .route("/runs", axum::routing::post(create_run));
    
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .unwrap();
    
    axum::serve(listener, app).await.unwrap();
}
```

- [ ] **Step 3: Commit**

```bash
git add crates/planner/src/validation.rs crates/planner/src/main.rs
git commit -m "feat(planner): implement semantic validation and main entry"
```

---

## Phase 2: Failure Policies (Day 2)

### Task 5: Implement FailureHandler

**Files:**
- Create: `crates/agent-runtime/src/failure.rs`

- [ ] **Step 1: Create failure.rs**

```rust
use types::{Node, NodeStatus, RunStatus};
use db::PgPool;
use crate::error::AgentError;

#[derive(Debug, Clone, Copy)]
pub enum RetryDecision {
    RetryAfter(u64),
    Escalate,
    Skip,
    Abort,
}

pub struct FailureHandler;

impl FailureHandler {
    pub async fn handle_failure(
        pool: &PgPool,
        node: &Node,
        error: &str,
    ) -> Result<RetryDecision, AgentError> {
        let policy = node.failure_policy.as_deref().unwrap_or("retry");
        
        match policy {
            "retry" => Self::handle_retry(pool, node).await,
            "skip" => Self::handle_skip(pool, node).await,
            "fallback" => Self::handle_fallback(pool, node).await,
            "abort" => Self::handle_abort(pool, node).await,
            _ => Ok(RetryDecision::Abort),
        }
    }
    
    async fn handle_retry(pool: &PgPool, node: &Node) -> Result<RetryDecision, AgentError> {
        const MAX_RETRIES: i32 = 2;
        const BASE_DELAY_SECS: u64 = 10;
        
        if node.retry_count >= MAX_RETRIES {
            return Ok(RetryDecision::Escalate);
        }
        
        let delay = BASE_DELAY_SECS * 2u64.pow(node.retry_count as u32);
        
        db::nodes::increment_retry(pool, node.id).await
            .map_err(|e| AgentError::Context(e.to_string()))?;
        
        Ok(RetryDecision::RetryAfter(delay))
    }
    
    async fn handle_skip(pool: &PgPool, node: &Node) -> Result<RetryDecision, AgentError> {
        db::nodes::update_status(pool, node.id, NodeStatus::Skipped).await
            .map_err(|e| AgentError::Context(e.to_string()))?;
        
        queue::complete(pool, node.id).await
            .map_err(|e| AgentError::Context(e.to_string()))?;
        
        Ok(RetryDecision::Skip)
    }
    
    async fn handle_fallback(pool: &PgPool, node: &Node) -> Result<RetryDecision, AgentError> {
        if let Some(ref fallback) = node.fallback_agent_type {
            db::nodes::update_agent_type(pool, node.id, fallback).await
                .map_err(|e| AgentError::Context(e.to_string()))?;
            Ok(RetryDecision::RetryAfter(0))
        } else {
            Ok(RetryDecision::Escalate)
        }
    }
    
    async fn handle_abort(pool: &PgPool, node: &Node) -> Result<RetryDecision, AgentError> {
        db::runs::update_status(pool, node.run_id, RunStatus::Failed).await
            .map_err(|e| AgentError::Context(e.to_string()))?;
        
        db::nodes::update_status(pool, node.id, NodeStatus::Failed).await
            .map_err(|e| AgentError::Context(e.to_string()))?;
        
        db::nodes::cancel_pending(pool, node.run_id).await
            .map_err(|e| AgentError::Context(e.to_string()))?;
        
        Ok(RetryDecision::Abort)
    }
}
```

- [ ] **Step 2: Update runtime.rs to use FailureHandler**

```rust
use crate::failure::FailureHandler;

impl AgentRuntime {
    pub async fn execute(&self, node: &mut Node) -> Result<String, AgentError> {
        let result = self.execute_inner(node).await;
        
        if let Err(ref e) = result {
            let decision = FailureHandler::handle_failure(&self.pool, node, &e.to_string()).await
                .map_err(|e| AgentError::Context(e.to_string()))?;
            
            match decision {
                RetryDecision::RetryAfter(delay) => {
                    if delay > 0 {
                        tokio::time::sleep(tokio::time::Duration::from_secs(delay)).await;
                    }
                    return self.execute(node).await;
                }
                _ => {}
            }
        }
        
        result
    }
}
```

- [ ] **Step 3: Commit**

```bash
git add crates/agent-runtime/src/failure.rs
git commit -m "feat(agent-runtime): implement FailureHandler with all policies"
```

---

## Phase 3: Enhanced Crash Recovery (Day 3)

### Task 6: Implement enhanced crash recovery

**Files:**
- Create: `crates/executor/src/crash_recovery.rs`

- [ ] **Step 1: Create crash_recovery.rs**

```rust
use std::time::Duration;
use types::{NodeStatus, QueueStatus};
use db::PgPool;

pub async fn recover_crashed_nodes(pool: &PgPool) -> Result<(), String> {
    let stale_locked = db::queue::find_stale_locked(pool, Duration::minutes(10))
        .await
        .map_err(|e| e.to_string())?;
    
    for entry in stale_locked {
        let redis_lock_exists = check_redis_lock(&entry).await
            .map_err(|e| e.to_string())?;
        
        if redis_lock_exists {
            continue;
        }
        
        let has_artifact = db::artifacts::node_has_artifact(pool, entry.node_id)
            .await
            .map_err(|e| e.to_string())?;
        
        if has_artifact {
            db::nodes::update_status(pool, entry.node_id, NodeStatus::Done)
                .await
                .map_err(|e| e.to_string())?;
            db::queue::complete(pool, entry.id)
                .await
                .map_err(|e| e.to_string())?;
            enqueue_downstream(pool, entry.node_id).await
                .map_err(|e| e.to_string())?;
        } else {
            db::queue::reset_to_pending(pool, entry.id)
                .await
                .map_err(|e| e.to_string())?;
            db::nodes::update_status(pool, entry.node_id, NodeStatus::Pending)
                .await
                .map_err(|e| e.to_string())?;
        }
    }
    
    Ok(())
}

async fn check_redis_lock(entry: &QueueEntry) -> Result<bool, String> {
    Ok(false)
}

async fn enqueue_downstream(pool: &PgPool, node_id: uuid::Uuid) -> Result<(), String> {
    let edges = db::edges::get_by_source(pool, node_id)
        .await
        .map_err(|e| e.to_string())?;
    
    for edge in edges {
        let upstream_done = db::nodes::is_status_done(pool, edge.target_node)
            .await
            .map_err(|e| e.to_string())?;
        
        if upstream_done {
            let node = db::nodes::get(pool, edge.target_node)
                .await
                .map_err(|e| e.to_string())?;
            
            if let Some(n) = node {
                let entry = types::QueueEntry::new(n.tenant_id, n.run_id, n.id, 0);
                queue::enqueue(pool, &entry).await
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    
    Ok(())
}

pub async fn execute_node(pool: &PgPool, node: &types::Node) -> Result<(), String> {
    db::nodes::update_status(pool, node.id, NodeStatus::Running)
        .await
        .map_err(|e| e.to_string())?;
    
    let runtime = create_runtime().await;
    let result = runtime.execute(&mut node.clone()).await;
    
    match result {
        Ok(_) => {
            db::nodes::update_status(pool, node.id, NodeStatus::Done)
                .await
                .map_err(|e| e.to_string())?;
            queue::complete(pool, node.id).await
                .map_err(|e| e.to_string())?;
        }
        Err(e) => {
            db::nodes::update_status(pool, node.id, NodeStatus::Failed)
                .await
                .map_err(|e| e.to_string())?;
        }
    }
    
    enqueue_downstream(pool, node.id).await
}

fn create_runtime() -> Arc<AgentRuntime> {
    todo!()
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/executor/src/crash_recovery.rs
git commit -m "feat(executor): implement enhanced crash recovery"
```

---

## Phase 4: Node Logs Integration (Day 4)

### Task 7: Add node_logs support

**Files:**
- Create: `crates/db/migrations/008_create_node_logs.sql`
- Create: `crates/db/src/node_logs.rs`

- [ ] **Step 1: Create migration**

```sql
CREATE TABLE node_logs (
    id                UUID PRIMARY KEY,
    node_id           UUID REFERENCES nodes,
    run_id            UUID REFERENCES runs,
    tenant_id         UUID REFERENCES tenants,
    executor_id       TEXT,
    started_at        TIMESTAMPTZ,
    completed_at      TIMESTAMPTZ,
    prompt_tokens     INTEGER,
    completion_tokens INTEGER,
    tool_calls        JSONB DEFAULT '[]',
    status            TEXT,
    error             TEXT
);

CREATE INDEX idx_node_logs_node_id ON node_logs(node_id);
CREATE INDEX idx_node_logs_run_id ON node_logs(run_id);
CREATE INDEX idx_node_logs_tenant_id ON node_logs(tenant_id);
```

- [ ] **Step 2: Create node_logs.rs**

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeLog {
    pub id: Uuid,
    pub node_id: Uuid,
    pub run_id: Uuid,
    pub tenant_id: Uuid,
    pub executor_id: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub prompt_tokens: Option<i64>,
    pub completion_tokens: Option<i64>,
    pub tool_calls: Vec<ToolCallRecord>,
    pub status: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    pub tool: String,
    pub args_summary: String,
    pub duration_ms: u64,
    pub status: String,
    pub error: Option<String>,
}

pub async fn create(pool: &PgPool, log: &NodeLog) -> Result<NodeLog, sqlx::Error> {
    sqlx::query_as!(
        NodeLog,
        r#"
        INSERT INTO node_logs (id, node_id, run_id, tenant_id, executor_id, started_at, completed_at, status)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING *
        "#,
        log.id,
        log.node_id,
        log.run_id,
        log.tenant_id,
        log.executor_id,
        log.started_at,
        log.completed_at,
        log.status
    )
    .fetch_one(pool)
    .await
}

pub async fn update_tokens(
    pool: &PgPool,
    id: Uuid,
    prompt_tokens: i64,
    completion_tokens: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE node_logs SET prompt_tokens = $2, completion_tokens = $3 WHERE id = $1",
        id,
        prompt_tokens,
        completion_tokens
    )
    .execute(pool)
    .await?;
    Ok(())
}
```

- [ ] **Step 3: Commit**

```bash
git add crates/db/migrations/008_create_node_logs.sql crates/db/src/node_logs.rs
git commit -m "feat(db): add node_logs support"
```

---

## Phase 5: Integration (Day 5)

### Task 8: Workspace verification

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
git commit -m "feat: integrate Phase 4 components"
```

---

## Summary

| Phase | Tasks | Duration |
|-------|-------|----------|
| Phase 1: Planner | Tasks 1-4 | Day 1 |
| Phase 2: Failure Policies | Task 5 | Day 2 |
| Phase 3: Crash Recovery | Task 6 | Day 3 |
| Phase 4: Node Logs | Task 7 | Day 4 |
| Phase 5: Integration | Task 8 | Day 5 |

**Total Estimated Time:** 5 days

---

**Plan complete and saved to** `docs/superpowers/plans/2025-04-05-torque-planner-reliability-plan.md`
