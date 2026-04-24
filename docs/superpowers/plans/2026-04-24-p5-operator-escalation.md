# P5: Operator Escalation Endpoints Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement operator escalation endpoints for high-severity recovery issues that require human intervention.

**Architecture:**
- EscalationService to manage escalation lifecycle
- Escalation persistence in database
- API endpoints for operators to view and resolve escalations
- Integration with recovery system to create escalations when auto-resolution fails

**Tech Stack:** Rust (tokio, sqlx, axum), PostgreSQL

---

## Task 1: Escalation Data Model and Repository

### Files
- Create: `crates/torque-harness/src/models/v1/escalation.rs`
- Create: `crates/torque-harness/src/repository/escalation.rs`
- Modify: `crates/torque-harness/src/repository/mod.rs`

- [ ] **Step 1: Add Escalation model**

Create `crates/torque-harness/src/models/v1/escalation.rs`:
```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Escalation {
    pub id: Uuid,
    pub instance_id: Uuid,
    pub team_instance_id: Option<Uuid>,
    pub escalation_type: EscalationType,
    pub severity: EscalationSeverity,
    pub status: EscalationStatus,
    pub description: String,
    pub context: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolved_by: Option<Uuid>,
    pub resolution: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EscalationType {
    RecoveryFailed,
    TeamMemberFailed,
    ApprovalRequired,
    PolicyViolation,
    ResourceExceeded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EscalationSeverity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EscalationStatus {
    Pending,
    Acknowledged,
    InProgress,
    Resolved,
    Cancelled,
}
```

- [ ] **Step 2: Add EscalationRepository trait**

Create `crates/torque-harness/src/repository/escalation.rs`:
```rust
use crate::db::Database;
use crate::models::v1::escalation::{Escalation, EscalationStatus};
use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait EscalationRepository: Send + Sync {
    async fn create(&self, escalation: &Escalation) -> anyhow::Result<Escalation>;
    async fn get(&self, id: Uuid) -> anyhow::Result<Option<Escalation>>;
    async fn list_pending(&self, limit: i64) -> anyhow::Result<Vec<Escalation>>;
    async fn list_by_instance(&self, instance_id: Uuid) -> anyhow::Result<Vec<Escalation>>;
    async fn update_status(&self, id: Uuid, status: EscalationStatus) -> anyhow::Result<()>;
    async fn resolve(
        &self,
        id: Uuid,
        resolution: &str,
        resolved_by: Uuid,
    ) -> anyhow::Result<()>;
}

pub struct PostgresEscalationRepository {
    db: Database,
}
```

- [ ] **Step 3: Implement PostgresEscalationRepository**

Add SQL implementation using `v1_escalations` table.

- [ ] **Step 4: Add migration for escalations table**

Create migration to add `v1_escalations` table.

- [ ] **Step 5: Run cargo check**

Run: `cargo check -p torque-harness`

- [ ] **Step 6: Commit**

```bash
git add crates/torque-harness/src/models/v1/escalation.rs crates/torque-harness/src/repository/escalation.rs crates/torque-harness/src/repository/mod.rs
git commit -m "feat(escalation): add escalation data model and repository"
```

---

## Task 2: Escalation Service

### Files
- Create: `crates/torque-harness/src/service/escalation.rs`
- Modify: `crates/torque-harness/src/service/recovery.rs`

- [ ] **Step 1: Add EscalationService**

Create `crates/torque-harness/src/service/escalation.rs`:
```rust
use crate::models::v1::escalation::{Escalation, EscalationSeverity, EscalationStatus, EscalationType};
use crate::repository::escalation::EscalationRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct EscalationService {
    escalation_repo: Arc<dyn EscalationRepository>,
}

impl EscalationService {
    pub fn new(escalation_repo: Arc<dyn EscalationRepository>) -> Self {
        Self { escalation_repo }
    }

    pub async fn create_escalation(
        &self,
        instance_id: Uuid,
        escalation_type: EscalationType,
        severity: EscalationSeverity,
        description: String,
        context: serde_json::Value,
    ) -> anyhow::Result<Escalation> {
        let escalation = Escalation {
            id: Uuid::new_v4(),
            instance_id,
            team_instance_id: None,
            escalation_type,
            severity,
            status: EscalationStatus::Pending,
            description,
            context,
            created_at: chrono::Utc::now(),
            resolved_at: None,
            resolved_by: None,
            resolution: None,
        };

        self.escalation_repo.create(&escalation).await
    }

    pub async fn list_pending_escalations(
        &self,
        limit: i64,
    ) -> anyhow::Result<Vec<Escalation>> {
        self.escalation_repo.list_pending(limit).await
    }

    pub async fn resolve_escalation(
        &self,
        id: Uuid,
        resolution: &str,
        resolved_by: Uuid,
    ) -> anyhow::Result<Escalation> {
        self.escalation_repo.resolve(id, resolution, resolved_by).await?;
        self.escalation_repo.get(id).await?.ok_or_else(|| anyhow::anyhow!("Escalation not found"))
    }
}
```

- [ ] **Step 2: Integrate with RecoveryService**

Modify `RecoveryService` to create escalations when recovery fails:
```rust
pub async fn assess_and_escalate_if_needed(
    &self,
    instance_id: Uuid,
) -> anyhow::Result<Option<Escalation>> {
    let assessment = self.assess_recovery(/* ... */).await?;

    if matches!(assessment.disposition, RecoveryDisposition::Failed) {
        let escalation = self.escalation_service.create_escalation(
            instance_id,
            EscalationType::RecoveryFailed,
            EscalationSeverity::High,
            format!("Recovery failed: {:?}", assessment.recommended_action),
            serde_json::json!({ "assessment": assessment }),
        ).await?;
        return Ok(Some(escalation));
    }

    Ok(None)
}
```

- [ ] **Step 3: Run cargo check**

Run: `cargo check -p torque-harness`

- [ ] **Step 4: Commit**

```bash
git add crates/torque-harness/src/service/escalation.rs crates/torque-harness/src/service/recovery.rs
git commit -m "feat(escalation): add escalation service"
```

---

## Task 3: Operator Escalation API Endpoints

### Files
- Create: `crates/torque-harness/src/api/v1/escalations.rs`
- Modify: `crates/torque-harness/src/api/v1/mod.rs`

- [ ] **Step 1: Add escalation endpoints**

Create `crates/torque-harness/src/api/v1/escalations.rs`:
```rust
pub async fn list(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Query(q): Query<EscalationListQuery>,
) -> Result<Json<ListResponse<Escalation>>, (StatusCode, Json<ErrorBody>)> {
    let limit = q.limit.unwrap_or(50).clamp(1, 100);
    let escalations = services
        .escalation
        .list_pending_escalations(limit)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorBody { code: "DB_ERROR".into(), message: e.to_string(), details: None, request_id: None })))?;

    Ok(Json(ListResponse {
        data: escalations,
        pagination: Pagination { total: None, limit, offset: 0, has_more: false },
    }))
}

pub async fn get(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
) -> Result<Json<Escalation>, (StatusCode, Json<ErrorBody>)> {
    let escalation = services.escalation.get(id).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorBody { code: "DB_ERROR".into(), message: e.to_string(), details: None, request_id: None })))?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(escalation))
}

#[derive(serde::Deserialize)]
pub struct EscalationResolveRequest {
    pub resolution: String,
    pub resolved_by: Uuid,
}

pub async fn resolve(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
    Json(req): Json<EscalationResolveRequest>,
) -> Result<Json<Escalation>, (StatusCode, Json<ErrorBody>)> {
    let escalation = services.escalation.resolve_escalation(id, &req.resolution, req.resolved_by).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorBody { code: "DB_ERROR".into(), message: e.to_string(), details: None, request_id: None })))?;

    Ok(Json(escalation))
}
```

- [ ] **Step 2: Register routes**

Read `crates/torque-harness/src/api/v1/mod.rs`.

Add:
```rust
.route("/v1/escalations", get(escalations::list))
.route("/v1/escalations/:id", get(escalations::get))
.route("/v1/escalations/:id/resolve", post(escalations::resolve))
```

- [ ] **Step 3: Run cargo check**

Run: `cargo check -p torque-harness`

- [ ] **Step 4: Commit**

```bash
git add crates/torque-harness/src/api/v1/escalations.rs crates/torque-harness/src/api/v1/mod.rs
git commit -m "feat(escalation): add operator escalation API endpoints"
```

---

## Task 4: Final Verification

- [ ] **Step 1: Run full test suite**

Run: `cargo test -p torque-harness 2>&1 | tail -50`
Expected: All tests pass

- [ ] **Step 2: Run cargo check for warnings**

Run: `cargo check -p torque-harness 2>&1 | grep -E "warning|error"`
Expected: Only existing warnings

- [ ] **Step 3: Update STATUS.md**

Add P5 section documenting:
- Operator escalation endpoints
- GET /v1/escalations, GET /v1/escalations/{id}, POST /v1/escalations/{id}/resolve
- Integration with recovery system

- [ ] **Step 4: Final commit**

```bash
git add STATUS.md
git commit -m "docs: mark P5 Operator Escalation Endpoints complete"
```

---

## New Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/v1/escalations` | GET | List pending escalations |
| `/v1/escalations/{id}` | GET | Get escalation details |
| `/v1/escalations/{id}/resolve` | POST | Resolve an escalation |

## Test Count Impact

- New tests: escalation_tests (3-5)
- Expected total: ~146 tests