# Phase 5: Multi-Tenancy + Admin

## Overview

**Goal**: Support multiple tenants with proper isolation and resource quotas.

**Prerequisites**: Phase 4 (Planner + Reliability)

---

## Success Criteria

- [ ] All queries enforce tenant_id filtering
- [ ] Scheduler implements weighted round-robin
- [ ] Concurrency quota enforced per tenant
- [ ] Token usage tracked and quota enforced
- [ ] Admin API for agent/tenant management works

---

## Components

### 1. Tenant Isolation

**Requirement**: All data access must be tenant-scoped.

**Implementation Pattern**:
```rust
// Every query must include tenant_id
async fn get_node(pool: &PgPool, tenant_id: Uuid, node_id: Uuid) -> Result<Option<Node>> {
    sqlx::query_as!(
        Node,
        "SELECT * FROM nodes WHERE id = $1 AND tenant_id = $2",
        node_id,
        tenant_id
    )
    .fetch_optional(pool)
    .await
}
```

**Files** (modify):
- `crates/db/src/runs.rs` - Add tenant_id to all queries
- `crates/db/src/nodes.rs`
- `crates/db/src/edges.rs`
- `crates/db/src/artifacts.rs`
- `crates/db/src/queue.rs`
- `crates/db/src/node_logs.rs`

**Redis Key Isolation**:
```
{tenant_id}:node:{node_id}:status
{tenant_id}:run:{run_id}:concurrency
{tenant_id}:token_usage:monthly
{tenant_id}:vfs:lock:{path}
```

---

### 2. Scheduler Weighted Round-Robin

**Current**: Simple tenant iteration
**Enhanced**: Weighted based on `tenants.weight`

```rust
struct Scheduler {
    tenants: Vec<Tenant>,
    current_index: usize,
}

impl Scheduler {
    fn next(&mut self) -> &Tenant {
        // Skip tenants at quota limit
        loop {
            let tenant = &self.tenants[self.current_index];
            self.current_index = (self.current_index + 1) % self.tenants.len();
            
            if !self.is_at_quota(tenant) {
                return tenant;
            }
        }
    }
    
    fn is_at_quota(&self, tenant: &Tenant) -> bool {
        let current = self.redis.get_concurrency(tenant.id).await.unwrap_or(0);
        current >= tenant.max_concurrency
    }
}
```

**Files** (modify):
- `crates/executor/src/scheduler.rs` - Add weighted round-robin

---

### 3. Concurrency Quota

**Enforcement Point**: Before dequeuing a node

```rust
async fn dequeue(&self, tenant_id: Uuid) -> Result<Option<QueueEntry>> {
    // Check concurrency quota
    let current = self.redis.get_concurrency(tenant_id).await?;
    let max = self.db.get_tenant_max_concurrency(tenant_id).await?;
    
    if current >= max {
        return Ok(None);  // Skip this tenant
    }
    
    // Proceed with dequeue
    let entry = self.queue.dequeue(tenant_id, self.executor_id).await?;
    
    if let Some(ref e) = entry {
        // Increment concurrency counter
        self.redis.inc_concurrency(tenant_id).await?;
    }
    
    Ok(entry)
}
```

**Redis Counter Keys**:
```
{tenant_id}:run:{run_id}:concurrency  → INCR/DECR
```

---

### 4. Token Usage Tracking

**Monthly Quota Check**:
```rust
async fn check_token_quota(&self, tenant_id: Uuid) -> Result<bool> {
    let used = self.redis.get_monthly_usage(tenant_id).await?;
    let quota = self.db.get_monthly_quota(tenant_id).await?;
    
    Ok(used < quota)
}
```

**Usage Sync** (background job):
```rust
// Every minute, sync from node_logs to Redis
async fn sync_token_usage(&self) {
    let tenants = self.db.get_all_tenants().await?;
    
    for tenant in tenants {
        let monthly_usage = self.db.aggregate_monthly_usage(tenant.id).await?;
        self.redis.set_monthly_usage(tenant.id, monthly_usage).await?;
    }
}
```

**Files**:
- `crates/executor/src/quota.rs` - NEW: Quota checking
- `crates/executor/src/usage_sync.rs` - NEW: Background sync job

---

### 5. Admin API

**Agent Management**:
```rust
// GET /agents
async fn list_agents() -> Vec<AgentType>

// POST /agents
async fn create_agent(agent: CreateAgentRequest) -> AgentType

// PUT /agents/{name}
async fn update_agent(name: String, update: UpdateAgentRequest) -> AgentType
```

**Tenant Management**:
```rust
// GET /tenants/{id}/usage
async fn get_usage(tenant_id: Uuid) -> TenantUsage {
    monthly_token_usage: i64,
    current_concurrency: i64,
    active_runs: i64,
}

// PUT /tenants/{id}/quota
async fn update_quota(tenant_id: Uuid, quota: QuotaUpdate) -> Tenant
```

**Files**:
- `crates/planner/src/admin_handler.rs` - NEW: Admin HTTP handlers
- `crates/planner/src/admin/error.rs` - NEW

---

## Architecture

```
Admin API (planner crate)
  └→ db (agent_types, tenants)

Executor Scheduler
  ├→ Weighted round-robin
  ├→ Concurrency quota (Redis counter)
  └→ Token quota (Redis cached from db)
```

---

## Database Schema Additions

```sql
-- Tenant quotas already exist, need to ensure proper indexing
CREATE INDEX idx_runs_tenant_id ON runs(tenant_id);
CREATE INDEX idx_nodes_tenant_id ON nodes(tenant_id);
CREATE INDEX idx_queue_tenant_id ON queue(tenant_id);

-- For usage aggregation query performance
CREATE INDEX idx_node_logs_tenant_month ON node_logs(tenant_id, recorded_at);
```

---

## Implementation Order

1. **Tenant isolation audit** - Ensure all queries have tenant_id
2. **Weighted scheduler** - Enhance scheduler with weights
3. **Concurrency quota** - Redis counters + enforcement
4. **Token usage tracking** - node_logs aggregation + Redis cache
5. **Admin API** - Agent and tenant management

---

## Files to Create/Modify

```
crates/
├── db/                        # MODIFY - Add tenant_id to all queries
│   ├── src/runs.rs
│   ├── src/nodes.rs
│   ├── src/edges.rs
│   ├── src/artifacts.rs
│   ├── src/queue.rs
│   └── src/node_logs.rs
│
├── executor/                  # MODIFY
│   ├── src/
│   │   ├── scheduler.rs     # Weighted round-robin
│   │   ├── quota.rs        # NEW: Quota enforcement
│   │   └── usage_sync.rs   # NEW: Background sync
│   └── ...
│
└── planner/                  # MODIFY
    ├── src/
    │   └── admin_handler.rs # NEW: Admin API handlers
    └── ...
```

---

## Next Phase

Phase 6: Observability - Structured logging, tracing, usage stats API
