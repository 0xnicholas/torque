# P2: Governance & Audit Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement governance and audit features for the memory pipeline - decision logging query API, enhanced review queue, manual compaction trigger, and decision analytics.

**Architecture:** Extend the existing memory pipeline with governance endpoints. Add new API routes for decision queries and analytics. Implement a DecisionLogService for querying decision history.

**Tech Stack:** Rust (tokio, sqlx, axum), PostgreSQL

---

## Task 1: Decision Log Query API

### Files
- Modify: `crates/torque-harness/src/repository/memory_v1.rs`
- Modify: `crates/torque-harness/src/service/memory.rs`
- Modify: `crates/torque-harness/src/api/v1/memory.rs`
- Test: `crates/torque-harness/tests/decision_log_tests.rs`

- [ ] **Step 1: Add list_decisions to MemoryRepositoryV1 trait**

Read `crates/torque-harness/src/repository/memory_v1.rs` around line 130.

Add to trait:
```rust
async fn list_decisions(
    &self,
    agent_instance_id: Option<Uuid>,
    decision_type: Option<&str>,
    start_date: Option<DateTime<Utc>>,
    end_date: Option<DateTime<Utc>>,
    limit: i64,
    offset: i64,
) -> anyhow::Result<Vec<MemoryDecisionLog>>;
```

- [ ] **Step 2: Implement list_decisions in PostgresMemoryRepositoryV1**

Read implementation around line 673.

Add implementation:
```rust
async fn list_decisions(
    &self,
    agent_instance_id: Option<Uuid>,
    decision_type: Option<&str>,
    start_date: Option<DateTime<Utc>>,
    end_date: Option<DateTime<Utc>>,
    limit: i64,
    offset: i64,
) -> anyhow::Result<Vec<MemoryDecisionLog>> {
    let mut query = "SELECT * FROM memory_decision_log WHERE 1=1".to_string();
    let mut params: Vec<Box<dyn sqlx::Encode<'_, sqlx::Postgres> + Send + Sync>> = Vec::new();
    let mut param_idx = 1;

    if let Some(agent_id) = agent_instance_id {
        query.push_str(&format!(" AND agent_instance_id = ${}", param_idx));
        params.push(Box::new(agent_id));
        param_idx += 1;
    }

    if let Some dtype) = decision_type {
        query.push_str(&format!(" AND decision_type = ${}", param_idx));
        params.push(Box::new(dtype.to_string()));
        param_idx += 1;
    }

    if let Some(start) = start_date {
        query.push_str(&format!(" AND created_at >= ${}", param_idx));
        params.push(Box::new(start));
        param_idx += 1;
    }

    if let Some(end) = end_date {
        query.push_str(&format!(" AND created_at <= ${}", param_idx));
        params.push(Box::new(end));
        param_idx += 1;
    }

    query.push_str(&format!(" ORDER BY created_at DESC LIMIT ${} OFFSET ${}", param_idx, param_idx + 1));
    params.push(Box::new(limit));
    params.push(Box::new(offset));

    // Use sqlx::query_as with manual mapping
    let rows = sqlx::query_as::<_, MemoryDecisionLog>(&query)
        .fetch_all(&*self.pool)
        .await?;

    Ok(rows)
}
```

- [ ] **Step 3: Add list_decisions to MemoryService**

Read `crates/torque-harness/src/service/memory.rs` around line 303.

Add method:
```rust
pub async fn list_decisions(
    &self,
    agent_instance_id: Option<Uuid>,
    decision_type: Option<&str>,
    start_date: Option<DateTime<Utc>>,
    end_date: Option<DateTime<Utc>>,
    limit: i64,
    offset: i64,
) -> anyhow::Result<Vec<MemoryDecisionLog>> {
    self.repo_v1.list_decisions(
        agent_instance_id,
        decision_type,
        start_date,
        end_date,
        limit,
        offset,
    ).await
}
```

- [ ] **Step 4: Add GET /v1/memory/decisions endpoint**

Read `crates/torque-harness/src/api/v1/memory.rs` around line 73.

Add after `list_candidates`:
```rust
#[derive(serde::Deserialize)]
pub struct DecisionListQuery {
    pub agent_instance_id: Option<Uuid>,
    pub decision_type: Option<String>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
    pub cursor: Option<String>,
}

pub async fn list_decisions(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Query(q): Query<DecisionListQuery>,
) -> Result<
    Json<ListResponse<MemoryDecisionLog>>,
    (StatusCode, Json<ErrorBody>),
> {
    let limit = q.limit.unwrap_or(50).clamp(1, 100);
    let offset = q.cursor.as_ref().and_then(|c| c.parse::<i64>().ok()).unwrap_or(0);

    let decisions = services
        .memory
        .list_decisions(
            q.agent_instance_id,
            q.decision_type.as_deref(),
            q.start_date,
            q.end_date,
            limit,
            offset,
        )
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    code: "DB_ERROR".into(),
                    message: e.to_string(),
                    details: None,
                    request_id: None,
                }),
            )
        })?;

    Ok(Json(ListResponse {
        data: decisions,
        pagination: Pagination {
            total: None,
            limit,
            offset,
            has_more: decisions.len() == limit as usize,
        },
    }))
}
```

- [ ] **Step 5: Register route in mod.rs**

Read `crates/torque-harness/src/api/v1/mod.rs` around line 172.

Add route:
```rust
.route("/v1/memory/decisions", get(memory::list_decisions))
```

- [ ] **Step 6: Create decision_log_tests.rs**

Create `crates/torque-harness/tests/decision_log_tests.rs`:
```rust
mod common;
use common::setup_test_db_or_skip;
use torque_harness::models::v1::memory::MemoryDecisionLog;
use torque_harness::repository::{MemoryRepositoryV1, PostgresMemoryRepositoryV1};
use uuid::Uuid;

#[tokio::test]
async fn test_list_decisions_empty() {
    let db = match setup_test_db_or_skip().await {
        Some(db) => db,
        None => return,
    };
    let repo = PostgresMemoryRepositoryV1::new(db);
    let decisions = repo.list_decisions(None, None, None, None, 10, 0).await.unwrap();
    assert!(decisions.is_empty());
}
```

- [ ] **Step 7: Run cargo check and tests**

Run: `cargo check -p torque-harness`
Run: `cargo test -p torque-harness decision_log`
Expected: PASS

- [ ] **Step 8: Commit**

```bash
git add crates/torque-harness/src/repository/memory_v1.rs crates/torque-harness/src/service/memory.rs crates/torque-harness/src/api/v1/memory.rs crates/torque-harness/src/api/v1/mod.rs crates/torque-harness/tests/decision_log_tests.rs
git commit -m "feat(governance): add decision log query API

- Add list_decisions() to repository and service
- Add GET /v1/memory/decisions endpoint
- Support filtering by agent, decision type, date range
- Add pagination"
```

---

## Task 2: Enhanced Review Queue

### Files
- Modify: `crates/torque-harness/src/api/v1/memory.rs`
- Modify: `crates/torque-harness/src/service/memory.rs`

- [ ] **Step 1: Add review queue stats to list_candidates response**

Read `crates/torque-harness/src/api/v1/memory.rs` around line 73.

Enhance `list_candidates` to return stats:
```rust
#[derive(serde::Serialize)]
pub struct CandidateListResponse {
    pub data: Vec<MemoryWriteCandidate>,
    pub pagination: Pagination,
    pub stats: Option<CandidateStats>,
}

#[derive(serde::Serialize)]
pub struct CandidateStats {
    pub total: i64,
    pub pending: i64,
    pub review_required: i64,
    pub approved: i64,
    pub rejected: i64,
}
```

- [ ] **Step 2: Add count_candidates_by_status to repository**

Read `crates/torque-harness/src/repository/memory_v1.rs`.

Add to trait:
```rust
async fn count_candidates_by_status(
    &self,
    agent_instance_id: Option<Uuid>,
) -> anyhow::Result<Vec<(String, i64)>>;
```

Add implementation:
```rust
async fn count_candidates_by_status(
    &self,
    agent_instance_id: Option<Uuid>,
) -> anyhow::Result<Vec<(String, i64)>> {
    let query = if agent_instance_id.is_some() {
        "SELECT status, COUNT(*) FROM memory_candidates WHERE agent_instance_id = $1 GROUP BY status"
    } else {
        "SELECT status, COUNT(*) FROM memory_candidates GROUP BY status"
    };

    let rows = if let Some(id) = agent_instance_id {
        sqlx::query_as::<_, (String, i64)>(query)
            .bind(id)
            .fetch_all(&*self.pool)
            .await?
    } else {
        sqlx::query_as::<_, (String, i64)>(query)
            .fetch_all(&*self.pool)
            .await?
    };

    Ok(rows)
}
```

- [ ] **Step 3: Add stats to list_candidates endpoint**

Update `list_candidates` to compute and return stats.

- [ ] **Step 4: Run cargo check**

Run: `cargo check -p torque-harness`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/torque-harness/src/repository/memory_v1.rs crates/torque-harness/src/service/memory.rs crates/torque-harness/src/api/v1/memory.rs
git commit -m "feat(governance): add stats to review queue endpoint

- Add count_candidates_by_status to repository
- Return candidate statistics in list response
- Include counts by status type"
```

---

## Task 3: Manual Compaction Trigger

### Files
- Modify: `crates/torque-harness/src/service/memory.rs`
- Modify: `crates/torque-harness/src/api/v1/memory.rs`
- Modify: `crates/torque-harness/src/jobs/memory_compaction.rs`
- Test: `crates/torque-harness/tests/compaction_tests.rs`

- [ ] **Step 1: Create compaction job tracking**

Add to `crates/torque-harness/src/models/v1/memory.rs`:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionJob {
    pub id: Uuid,
    pub agent_instance_id: Option<Uuid>,
    pub team_instance_id: Option<Uuid>,
    pub status: CompactionJobStatus,
    pub categories_processed: Vec<MemoryCategory>,
    pub entries_compacted: i64,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompactionJobStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}
```

- [ ] **Step 2: Add trigger_compaction to MemoryService**

Read `crates/torque-harness/src/service/memory.rs` around line 330.

Add method:
```rust
pub async fn trigger_compaction(
    &self,
    agent_instance_id: Option<Uuid>,
    team_instance_id: Option<Uuid>,
    categories: Option<Vec<MemoryCategory>>,
) -> anyhow::Result<CompactionJob> {
    let job = CompactionJob {
        id: Uuid::new_v4(),
        agent_instance_id,
        team_instance_id,
        status: CompactionJobStatus::Pending,
        categories_processed: categories.unwrap_or_default(),
        entries_compacted: 0,
        created_at: chrono::Utc::now(),
        completed_at: None,
    };

    // Spawn background compaction task
    let repo = self.repo_v1.clone();
    let job_id = job.id;

    tokio::spawn(async move {
        // Run compaction logic
        let _ = Self::run_compaction(repo, job_id).await;
    });

    Ok(job)
}

async fn run_compaction(
    repo: Arc<dyn MemoryRepositoryV1>,
    job_id: Uuid,
) -> anyhow::Result<()> {
    // TODO: Implement compaction logic
    Ok(())
}
```

- [ ] **Step 3: Add POST /v1/memory/compact endpoint**

Add to `crates/torque-harness/src/api/v1/memory.rs`:
```rust
#[derive(serde::Deserialize)]
pub struct CompactionRequest {
    pub agent_instance_id: Option<Uuid>,
    pub team_instance_id: Option<Uuid>,
    pub categories: Option<Vec<MemoryCategory>>,
}

pub async fn trigger_compaction(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Json(req): Json<CompactionRequest>,
) -> Result<
    Json<CompactionJob>,
    (StatusCode, Json<ErrorBody>),
> {
    let job = services
        .memory
        .trigger_compaction(
            req.agent_instance_id,
            req.team_instance_id,
            req.categories,
        )
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    code: "COMPACTION_ERROR".into(),
                    message: e.to_string(),
                    details: None,
                    request_id: None,
                }),
            )
        })?;

    Ok(Json(job))
}
```

- [ ] **Step 4: Register route**

Add to `crates/torque-harness/src/api/v1/mod.rs`:
```rust
.route("/v1/memory/compact", post(memory::trigger_compaction))
```

- [ ] **Step 5: Run cargo check**

Run: `cargo check -p torque-harness`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/torque-harness/src/service/memory.rs crates/torque-harness/src/api/v1/memory.rs crates/torque-harness/src/api/v1/mod.rs crates/torque-harness/src/models/v1/memory.rs
git commit -m "feat(governance): add manual compaction trigger API

- Add CompactionJob model
- Add trigger_compaction() to MemoryService
- Add POST /v1/memory/compact endpoint"
```

---

## Task 4: Decision Analytics

### Files
- Modify: `crates/torque-harness/src/repository/memory_v1.rs`
- Modify: `crates/torque-harness/src/service/memory.rs`
- Modify: `crates/torque-harness/src/api/v1/memory.rs`

- [ ] **Step 1: Add decision statistics query to repository**

Add to trait:
```rust
async fn get_decision_stats(
    &self,
    agent_instance_id: Option<Uuid>,
    start_date: Option<DateTime<Utc>>,
    end_date: Option<DateTime<Utc>>,
) -> anyhow::Result<DecisionStats>;
```

Add implementation:
```rust
async fn get_decision_stats(
    &self,
    agent_instance_id: Option<Uuid>,
    start_date: Option<DateTime<Utc>>,
    end_date: Option<DateTime<Utc>>,
) -> anyhow::Result<DecisionStats> {
    // Query for counts by decision type
    let type_query = "SELECT decision_type, COUNT(*) FROM memory_decision_log WHERE 1=1";
    // Query for avg quality score
    // Query for rejection reasons
    // Return DecisionStats struct
}
```

- [ ] **Step 2: Add DecisionStats model**

Add to `crates/torque-harness/src/models/v1/memory.rs`:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionStats {
    pub total_decisions: i64,
    pub approved: i64,
    pub rejected: i64,
    pub merged: i64,
    pub review: i64,
    pub approval_rate: f64,
    pub rejection_rate: f64,
    pub avg_quality_score: Option<f64>,
    pub top_rejection_reasons: Vec<RejectionReasonCount>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RejectionReasonCount {
    pub reason: String,
    pub count: i64,
}
```

- [ ] **Step 3: Add get_decision_stats to MemoryService**

Add method to forward to repository.

- [ ] **Step 4: Add GET /v1/memory/decisions/stats endpoint**

```rust
#[derive(serde::Deserialize)]
pub struct DecisionStatsQuery {
    pub agent_instance_id: Option<Uuid>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
}

pub async fn get_decision_stats(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Query(q): Query<DecisionStatsQuery>,
) -> Result<
    Json<DecisionStats>,
    (StatusCode, Json<ErrorBody>),
> {
    let stats = services
        .memory
        .get_decision_stats(q.agent_instance_id, q.start_date, q.end_date)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    code: "DB_ERROR".into(),
                    message: e.to_string(),
                    details: None,
                    request_id: None,
                }),
            )
        })?;

    Ok(Json(stats))
}
```

- [ ] **Step 5: Register route**

Add to `crates/torque-harness/src/api/v1/mod.rs`:
```rust
.route("/v1/memory/decisions/stats", get(memory::get_decision_stats))
```

- [ ] **Step 6: Run cargo check**

Run: `cargo check -p torque-harness`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/torque-harness/src/repository/memory_v1.rs crates/torque-harness/src/service/memory.rs crates/torque-harness/src/api/v1/memory.rs crates/torque-harness/src/api/v1/mod.rs crates/torque-harness/src/models/v1/memory.rs
git commit -m "feat(governance): add decision analytics endpoint

- Add DecisionStats model with approval/rejection rates
- Add get_decision_stats to repository
- Add GET /v1/memory/decisions/stats endpoint"
```

---

## Task 5: Final Verification

- [ ] **Step 1: Run full test suite**

Run: `cargo test -p torque-harness 2>&1 | tail -50`
Expected: All tests pass

- [ ] **Step 2: Run cargo check for warnings**

Run: `cargo check -p torque-harness 2>&1 | grep -E "warning|error"`
Expected: Only existing warnings

- [ ] **Step 3: Update STATUS.md**

Add P2: Governance & Audit section

- [ ] **Step 4: Final commit**

```bash
git add STATUS.md
git commit -m "docs: mark P2 Governance & Audit complete

- Decision log query API with filtering
- Enhanced review queue with stats
- Manual compaction trigger
- Decision analytics endpoint"
```

---

## Summary of Changes

### New API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/v1/memory/decisions` | GET | List decision history with filtering |
| `/v1/memory/decisions/stats` | GET | Get decision statistics |
| `/v1/memory/compact` | POST | Trigger manual compaction |

### New Models

| Model | Purpose |
|-------|---------|
| `DecisionStats` | Analytics aggregation |
| `CompactionJob` | Compaction job tracking |
| `CandidateStats` | Review queue statistics |
| `CandidateListResponse` | Enhanced list response |

### Modified Files
- `src/repository/memory_v1.rs` - list_decisions, count_candidates_by_status, get_decision_stats
- `src/service/memory.rs` - New query and trigger methods
- `src/api/v1/memory.rs` - New endpoints
- `src/api/v1/mod.rs` - Route registration
- `src/models/v1/memory.rs` - New model types
