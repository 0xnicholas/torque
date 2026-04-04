# Torque Multi-Tenancy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Support multiple tenants with proper isolation and resource quotas.

**Prerequisites:** Phase 4 (Planner + Reliability)

**Tech Stack:** Rust, tokio, redis-rs, sqlx

---

## File Structure Overview

```
crates/
├── db/                        # MODIFY - Add tenant_id to all queries
│   ├── src/
│   │   ├── runs.rs           # Modify: add tenant_id filter
│   │   ├── nodes.rs          # Modify: add tenant_id filter
│   │   ├── edges.rs          # Modify: add tenant_id filter
│   │   ├── artifacts.rs      # Modify: add tenant_id filter
│   │   ├── queue.rs          # Modify: add tenant_id filter
│   │   └── node_logs.rs      # Modify: add tenant_id filter
│   └── ...
│
├── executor/                  # MODIFY
│   ├── src/
│   │   ├── scheduler.rs      # Weighted round-robin
│   │   ├── quota.rs         # NEW: Quota enforcement
│   │   └── usage_sync.rs    # NEW: Background sync
│   └── ...
│
└── planner/                  # MODIFY
    ├── src/
    │   └── admin_handler.rs  # NEW: Admin API
    └── ...
```

---

## Phase 1: Tenant Isolation Audit (Day 1)

### Task 1: Audit and update all db queries

**Files:**
- Modify: `crates/db/src/runs.rs`
- Modify: `crates/db/src/nodes.rs`
- Modify: `crates/db/src/edges.rs`
- Modify: `crates/db/src/artifacts.rs`
- Modify: `crates/db/src/queue.rs`
- Modify: `crates/db/src/node_logs.rs`

- [ ] **Step 1: Update runs.rs with tenant_id filtering**

```rust
// Before:
pub async fn get(pool: &PgPool, id: Uuid) -> Result<Option<Run>, sqlx::Error> {
    sqlx::query_as!(Run, "SELECT * FROM runs WHERE id = $1", id)
        .fetch_optional(pool)
        .await
}

// After:
pub async fn get(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<Option<Run>, sqlx::Error> {
    sqlx::query_as!(
        Run,
        "SELECT * FROM runs WHERE id = $1 AND tenant_id = $2",
        id,
        tenant_id
    )
    .fetch_optional(pool)
    .await
}
```

- [ ] **Step 2: Update all other modules similarly**

```rust
// nodes.rs
pub async fn get(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<Option<Node>, sqlx::Error> {
    sqlx::query_as!(
        Node,
        "SELECT * FROM nodes WHERE id = $1 AND tenant_id = $2",
        id,
        tenant_id
    )
    .fetch_optional(pool)
    .await
}

// edges.rs
pub async fn get_by_run(pool: &PgPool, tenant_id: Uuid, run_id: Uuid) -> Result<Vec<Edge>, sqlx::Error> {
    sqlx::query_as!(
        Edge,
        "SELECT e.* FROM edges e JOIN runs r ON e.run_id = r.id WHERE e.run_id = $1 AND r.tenant_id = $2",
        run_id,
        tenant_id
    )
    .fetch_all(pool)
    .await
}

// artifacts.rs
pub async fn get_by_node(pool: &PgPool, tenant_id: Uuid, node_id: Uuid) -> Result<Vec<Artifact>, sqlx::Error> {
    sqlx::query_as!(
        Artifact,
        "SELECT * FROM artifacts WHERE node_id = $1 AND tenant_id = $2",
        node_id,
        tenant_id
    )
    .fetch_all(pool)
    .await
}

// queue.rs
pub async fn dequeue(
    pool: &PgPool,
    tenant_id: Uuid,
    executor_id: &str
) -> Result<Option<QueueEntry>, sqlx::Error> {
    sqlx::query_as!(
        QueueEntry,
        r#"
        SELECT * FROM queue
        WHERE tenant_id = $1
          AND status = 'pending'
          AND available_at <= NOW()
        ORDER BY priority DESC, created_at ASC
        LIMIT 1
        FOR UPDATE SKIP LOCKED
        "#,
        tenant_id
    )
    .fetch_optional(pool)
    .await
}
```

- [ ] **Step 3: Commit**

```bash
git add crates/db/src/
git commit -m "feat(db): add tenant_id filtering to all queries"
```

---

## Phase 2: Weighted Scheduler (Day 2)

### Task 2: Implement weighted round-robin scheduler

**Files:**
- Modify: `crates/executor/src/scheduler.rs`

- [ ] **Step 1: Update scheduler.rs**

```rust
use std::collections::VecDeque;
use types::Tenant;
use db::PgPool;

pub struct Scheduler {
    tenants: VecDeque<TenantWithWeight>,
    current_index: usize,
}

#[derive(Clone)]
struct TenantWithWeight {
    id: uuid::Uuid,
    weight: i32,
    max_concurrency: i32,
}

impl Scheduler {
    pub async fn new(pool: &PgPool) -> Result<Self, sqlx::Error> {
        let tenants = sqlx::query_as!(Tenant, "SELECT * FROM tenants")
            .fetch_all(pool)
            .await?;
        
        let weighted: VecDeque<_> = tenants.into_iter()
            .flat_map(|t| {
                std::iter::repeat(TenantWithWeight {
                    id: t.id,
                    weight: t.weight,
                    max_concurrency: t.max_concurrency,
                }).take(t.weight as usize)
            })
            .collect();
        
        Ok(Self {
            tenants: weighted,
            current_index: 0,
        })
    }
    
    pub fn next(&mut self) -> Option<uuid::Uuid> {
        if self.tenants.is_empty() {
            return None;
        }
        
        let len = self.tenants.len();
        for _ in 0..len {
            let tenant = self.tenants[self.current_index].clone();
            self.current_index = (self.current_index + 1) % len;
            return Some(tenant.id);
        }
        None
    }
    
    pub async fn is_at_quota(&self, pool: &PgPool, tenant_id: uuid::Uuid) -> Result<bool, sqlx::Error> {
        let current: i64 = redis::cmd("GET")
            .arg(format!("{}:concurrency", tenant_id))
            .query_async(&mut get_redis_conn().await?)
            .await
            .unwrap_or(0);
        
        let tenant = self.tenants.iter()
            .find(|t| t.id == tenant_id)
            .ok_or(sqlx::Error::RowNotFound)?;
        
        Ok(current >= tenant.max_concurrency as i64)
    }
}

async fn get_redis_conn() -> Result<redis::aio::ConnectionManager, redis::RedisError> {
    let redis_url = std::env::var("REDIS_URL").unwrap();
    redis::Client::open(redis_url)
        .unwrap()
        .get_connection_manager()
        .await
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/executor/src/scheduler.rs
git commit -m "feat(executor): implement weighted round-robin scheduler"
```

---

## Phase 3: Concurrency Quota (Day 3)

### Task 3: Implement quota enforcement

**Files:**
- Create: `crates/executor/src/quota.rs`

- [ ] **Step 1: Create quota.rs**

```rust
use redis::aio::ConnectionManager;
use db::PgPool;

pub struct QuotaManager {
    redis: ConnectionManager,
}

impl QuotaManager {
    pub fn new(redis: ConnectionManager) -> Self {
        Self { redis }
    }
    
    pub async fn check_concurrency_quota(&self, pool: &PgPool, tenant_id: uuid::Uuid) -> Result<bool, String> {
        let current: i64 = redis::cmd("GET")
            .arg(format!("{}:run:{}:concurrency", tenant_id, "*"))
            .query_async(&mut self.redis.clone())
            .await
            .unwrap_or(0);
        
        let max = self.get_max_concurrency(pool, tenant_id).await?;
        
        Ok(current < max)
    }
    
    pub async fn increment_concurrency(&self, tenant_id: uuid::Uuid, run_id: uuid::Uuid) -> Result<(), String> {
        let key = format!("{}:run:{}:concurrency", tenant_id, run_id);
        redis::cmd("INCR")
            .arg(&key)
            .query_async(&mut self.redis.clone())
            .await
            .map_err(|e| e.to_string())?;
        
        Ok(())
    }
    
    pub async fn decrement_concurrency(&self, tenant_id: uuid::Uuid, run_id: uuid::Uuid) -> Result<(), String> {
        let key = format!("{}:run:{}:concurrency", tenant_id, run_id);
        redis::cmd("DECR")
            .arg(&key)
            .query_async(&mut self.redis.clone())
            .await
            .map_err(|e| e.to_string())?;
        
        Ok(())
    }
    
    async fn get_max_concurrency(&self, pool: &PgPool, tenant_id: uuid::Uuid) -> Result<i32, String> {
        let tenant = sqlx::query_as!(
            Tenant,
            "SELECT * FROM tenants WHERE id = $1",
            tenant_id
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;
        
        Ok(tenant.map(|t| t.max_concurrency).unwrap_or(10))
    }
}
```

- [ ] **Step 2: Create usage_sync.rs**

```rust
use std::time::Duration;
use db::PgPool;
use redis::aio::ConnectionManager;

pub struct UsageSyncer {
    pool: PgPool,
    redis: ConnectionManager,
}

impl UsageSyncer {
    pub fn new(pool: PgPool, redis: ConnectionManager) -> Self {
        Self { pool, redis }
    }
    
    pub async fn sync_all(&self) -> Result<(), String> {
        let tenants = sqlx::query_as!(Tenant, "SELECT * FROM tenants")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        
        for tenant in tenants {
            let usage = self.aggregate_monthly_usage(tenant.id).await?;
            self.set_monthly_usage(tenant.id, usage).await?;
        }
        
        Ok(())
    }
    
    async fn aggregate_monthly_usage(&self, tenant_id: uuid::Uuid) -> Result<i64, String> {
        let usage = sqlx::query_scalar!(
            r#"
            SELECT COALESCE(SUM(prompt_tokens + completion_tokens), 0)
            FROM node_logs
            WHERE tenant_id = $1
              AND recorded_at >= date_trunc('month', NOW())
            "#,
            tenant_id
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| e.to_string())?;
        
        Ok(usage)
    }
    
    async fn set_monthly_usage(&self, tenant_id: uuid::Uuid, usage: i64) -> Result<(), String> {
        let key = format!("{}:token_usage:monthly", tenant_id);
        redis::cmd("SET")
            .arg(&key)
            .arg(usage)
            .query_async(&mut self.redis.clone())
            .await
            .map_err(|e| e.to_string())?;
        
        redis::cmd("EXPIRE")
            .arg(&key)
            .arg(60)
            .query_async(&mut self.redis.clone())
            .await
            .map_err(|e| e.to_string())?;
        
        Ok(())
    }
    
    pub async fn start_background_sync(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                if let Err(e) = self.sync_all().await {
                    tracing::error!("Usage sync failed: {}", e);
                }
            }
        });
    }
}
```

- [ ] **Step 3: Commit**

```bash
git add crates/executor/src/quota.rs crates/executor/src/usage_sync.rs
git commit -m "feat(executor): implement quota enforcement and usage sync"
```

---

## Phase 4: Admin API (Day 4)

### Task 4: Implement Admin API

**Files:**
- Create: `crates/planner/src/admin_handler.rs`

- [ ] **Step 1: Create admin_handler.rs**

```rust
use axum::{Router, Json, extract::Path};
use types::{Tenant, AgentType};
use db::PgPool;

pub fn router() -> Router {
    Router::new()
        .route("/agents", axum::routing::get(list_agents))
        .route("/agents", axum::routing::post(create_agent))
        .route("/agents/:name", axum::routing::put(update_agent))
        .route("/tenants/:id/usage", axum::routing::get(get_usage))
        .route("/tenants/:id/quota", axum::routing::put(update_quota))
}

async fn list_agents() -> Result<Json<Vec<AgentType>>, String> {
    let pool = get_pool().await;
    let agents = db::agent_types::list_all(&pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(Json(agents))
}

async fn create_agent(Json(payload): Json<CreateAgentRequest>) -> Result<Json<AgentType>, String> {
    let pool = get_pool().await;
    let agent = AgentType::new(payload.name, payload.system_prompt, payload.tools);
    db::agent_types::create(&pool, &agent)
        .await
        .map_err(|e| e.to_string())?;
    Ok(Json(agent))
}

async fn update_agent(Path(name): Path<String>, Json(payload): Json<UpdateAgentRequest>) -> Result<Json<AgentType>, String> {
    let pool = get_pool().await;
    let agent = db::agent_types::update(&pool, &name, &payload)
        .await
        .map_err(|e| e.to_string())?;
    Ok(Json(agent))
}

async fn get_usage(Path(tenant_id): Path<uuid::Uuid>) -> Result<Json<TenantUsage>, String> {
    let pool = get_pool().await;
    
    let monthly_usage: i64 = redis::cmd("GET")
        .arg(format!("{}:token_usage:monthly", tenant_id))
        .query_async(&mut get_redis_conn().await?)
        .await
        .unwrap_or(0);
    
    let current_concurrency: i64 = redis::cmd("GET")
        .arg(format!("{}:concurrency", tenant_id))
        .query_async(&mut get_redis_conn().await?)
        .await
        .unwrap_or(0);
    
    let active_runs = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM runs WHERE tenant_id = $1 AND status = 'running'",
        tenant_id
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| e.to_string())?;
    
    Ok(Json(TenantUsage {
        tenant_id,
        monthly_token_usage: monthly_usage,
        current_concurrency,
        active_runs,
    }))
}

async fn update_quota(Path(tenant_id): Path<uuid::Uuid>, Json(payload): Json<QuotaUpdate>) -> Result<Json<Tenant>, String> {
    let pool = get_pool().await;
    let tenant = db::tenants::update_quota(&pool, tenant_id, payload)
        .await
        .map_err(|e| e.to_string())?;
    Ok(Json(tenant))
}

async fn get_pool() -> PgPool {
    let database_url = std::env::var("DATABASE_URL").unwrap();
    PgPool::connect(&database_url).await.unwrap()
}

async fn get_redis_conn() -> redis::aio::ConnectionManager {
    let redis_url = std::env::var("REDIS_URL").unwrap();
    redis::Client::open(redis_url).unwrap().get_connection_manager().await.unwrap()
}

#[derive(serde::Deserialize)]
pub struct CreateAgentRequest {
    pub name: String,
    pub system_prompt: String,
    pub tools: Vec<String>,
}

#[derive(serde::Deserialize)]
pub struct UpdateAgentRequest {
    pub description: Option<String>,
    pub system_prompt: Option<String>,
    pub tools: Option<Vec<String>>,
}

#[derive(serde::Serialize)]
pub struct TenantUsage {
    pub tenant_id: uuid::Uuid,
    pub monthly_token_usage: i64,
    pub current_concurrency: i64,
    pub active_runs: i64,
}

#[derive(serde::Deserialize)]
pub struct QuotaUpdate {
    pub max_concurrency: Option<i32>,
    pub monthly_token_quota: Option<i64>,
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/planner/src/admin_handler.rs
git commit -m "feat(planner): implement Admin API"
```

---

## Phase 5: Integration (Day 5)

### Task 5: Workspace verification

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
git commit -m "feat: integrate Phase 5 components"
```

---

## Summary

| Phase | Tasks | Duration |
|-------|-------|----------|
| Phase 1: Tenant Isolation | Task 1 | Day 1 |
| Phase 2: Weighted Scheduler | Task 2 | Day 2 |
| Phase 3: Concurrency Quota | Task 3 | Day 3 |
| Phase 4: Admin API | Task 4 | Day 4 |
| Phase 5: Integration | Task 5 | Day 5 |

**Total Estimated Time:** 5 days

---

**Plan complete and saved to** `docs/superpowers/plans/2025-04-05-torque-multi-tenancy-plan.md`
