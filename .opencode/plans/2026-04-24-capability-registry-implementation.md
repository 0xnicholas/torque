# Capability Registry Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement capability resolution per `2026-04-08-torque-capability-registry-model-design.md` - `CapabilityRef` (string) → resolves to `CapabilityProfile` → fetches `CapabilityRegistryBinding`s → returns `CapabilityResolution` with ordered candidates

**Architecture:** Capability resolution flow: Upper layer passes `CapabilityRef` string → Registry resolves to canonical profile → Fetches bindings → Returns resolution with candidates, rationale, risk level, quality tier

**V1 Deferrals (not in scope for this plan):**
- Constraint evaluation (policy, resource, approval context) - `_constraints` stored but not evaluated
- Alias resolution (alias → canonical, deprecated → replacement)
- Version-aware lookup

**Tech Stack:** Rust, sqlx, tokio, torque-harness

---

## File Structure

```
crates/torque-harness/src/
├── models/v1/capability.rs          # Add CapabilityRef, CapabilityResolution, ResolvedCandidate, CapabilityResolveByRefRequest
├── service/capability.rs            # Implement resolve_by_ref()
├── repository/capability.rs         # Add list_by_profile(), get_by_name()
└── api/v1/capabilities.rs          # Add resolve endpoint
```

---

## Phase 1: Define Types

### Task 1: Add CapabilityRef and Resolution Types

**Files:**
- Modify: `crates/torque-harness/src/models/v1/capability.rs`

- [ ] **Step 1: Add CapabilityRef newtype**

Add after line 77:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityRef(pub String);

impl CapabilityRef {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
```

- [ ] **Step 2: Add ResolvedCandidate struct**

Add after CapabilityRegistryBindingCreate (before line 71):
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedCandidate {
    pub capability_profile_id: Uuid,
    pub agent_definition_id: Uuid,
    pub match_rationale: String,
    pub policy_check_summary: Option<serde_json::Value>,
    pub risk_level: RiskLevel,
    pub quality_tier: QualityTier,
    pub compatibility_score: Option<f64>,
    pub cost_or_latency_estimate: Option<String>,  // v1: not evaluated
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityResolution {
    pub capability_ref: String,
    pub capability_profile_id: Uuid,
    pub candidates: Vec<ResolvedCandidate>,
    pub resolved_at: chrono::DateTime<chrono::Utc>,
}
```

- [ ] **Step 3: Add CapabilityResolveByRefRequest struct**

Add at end of file:
```rust
#[derive(Debug, Deserialize)]
pub struct CapabilityResolveByRefRequest {
    pub capability_ref: String,
    pub constraints: Option<serde_json::Value>,
}
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p torque-harness 2>&1 | tail -10`
Expected: Compiles successfully

- [ ] **Step 5: Commit**

```bash
git add crates/torque-harness/src/models/v1/capability.rs
git commit -m "feat(capability): add CapabilityRef and resolution result types"
```

---

## Phase 2: Repository Extensions

### Task 2: Add list_by_profile to BindingRepository

**Files:**
- Modify: `crates/torque-harness/src/repository/capability.rs:74-127`

- [ ] **Step 1: Add trait method**

Add to `CapabilityRegistryBindingRepository` trait (around line 74):
```rust
async fn list_by_profile(&self, profile_id: Uuid, limit: i64) -> anyhow::Result<Vec<CapabilityRegistryBinding>>;
```

- [ ] **Step 2: Implement in PostgresCapabilityRegistryBindingRepository**

Add method implementation around line 127:
```rust
async fn list_by_profile(&self, profile_id: Uuid, limit: i64) -> anyhow::Result<Vec<CapabilityRegistryBinding>> {
    let rows = sqlx::query_as::<_, CapabilityRegistryBinding>(
        "SELECT * FROM v1_capability_registry_bindings WHERE capability_profile_id = $1 ORDER BY compatibility_score DESC LIMIT $2"
    )
    .bind(profile_id)
    .bind(limit)
    .fetch_all(self.db.pool())
    .await?;
    Ok(rows)
}
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p torque-harness 2>&1 | tail -5`
Expected: Compiles successfully

- [ ] **Step 4: Commit**

```bash
git add crates/torque-harness/src/repository/capability.rs
git commit -m "feat(capability): add list_by_profile to binding repository"
```

---

### Task 3: Add get_by_name to ProfileRepository

**Files:**
- Modify: `crates/torque-harness/src/repository/capability.rs:10-73`

- [ ] **Step 1: Add trait method**

Add to `CapabilityProfileRepository` trait (around line 10):
```rust
async fn get_by_name(&self, name: &str) -> anyhow::Result<Option<CapabilityProfile>>;
```

- [ ] **Step 2: Implement in PostgresCapabilityProfileRepository**

Add method implementation around line 55:
```rust
async fn get_by_name(&self, name: &str) -> anyhow::Result<Option<CapabilityProfile>> {
    let row = sqlx::query_as::<_, CapabilityProfile>(
        "SELECT * FROM v1_capability_profiles WHERE name = $1"
    )
    .bind(name)
    .fetch_optional(self.db.pool())
    .await?;
    Ok(row)
}
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p torque-harness 2>&1 | tail -5`
Expected: Compiles successfully

- [ ] **Step 4: Commit**

```bash
git add crates/torque-harness/src/repository/capability.rs
git commit -m "feat(capability): add get_by_name to profile repository"
```

---

## Phase 3: Implement Resolution Logic

### Task 4: Implement resolve_by_ref

**Files:**
- Modify: `crates/torque-harness/src/service/capability.rs`

- [ ] **Step 1: Update imports**

Add to imports:
```rust
use crate::models::v1::capability::{
    CapabilityProfile, CapabilityProfileCreate, CapabilityRegistryBinding,
    CapabilityRegistryBindingCreate, CapabilityResolveRequest, CapabilityRef,
    CapabilityResolution, ResolvedCandidate, CapabilityResolveByRefRequest,
};
```

- [ ] **Step 2: Add resolve_by_ref method**

Add to `CapabilityService` impl (around line 23, after constructor):
```rust
pub async fn resolve_by_ref(
    &self,
    capability_ref: &str,
    _constraints: Option<serde_json::Value>,
) -> anyhow::Result<CapabilityResolution> {
    // 1. Resolve capability_ref string to canonical profile
    let profile = self.profile_repo.get_by_name(capability_ref).await?
        .ok_or_else(|| anyhow::anyhow!("Capability profile not found: {}", capability_ref))?;

    // 2. Get bindings for this profile
    let bindings = self.binding_repo.list_by_profile(profile.id, 10).await?;

    // 3. Build candidates from bindings
    let candidates: Vec<ResolvedCandidate> = bindings.into_iter().map(|b| {
        ResolvedCandidate {
            capability_profile_id: b.capability_profile_id,
            agent_definition_id: b.agent_definition_id,
            match_rationale: "Direct binding match".to_string(),
            policy_check_summary: None,
            risk_level: profile.risk_level.clone(),
            quality_tier: b.quality_tier,
            compatibility_score: b.compatibility_score,
        }
    }).collect();

    Ok(CapabilityResolution {
        capability_ref: capability_ref.to_string(),
        capability_profile_id: profile.id,
        candidates,
        resolved_at: chrono::Utc::now(),
    })
}
```

- [ ] **Step 3: Update existing resolve stub**

Replace the empty `resolve()` method (lines 66-71):
```rust
pub async fn resolve(
    &self,
    req: CapabilityResolveRequest,
) -> anyhow::Result<CapabilityResolution> {
    let capability_ref = req.constraints
        .as_ref()
        .and_then(|c| c.get("capability_ref"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("capability_ref required in constraints"))?;

    self.resolve_by_ref(capability_ref, req.constraints).await
}
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p torque-harness 2>&1 | tail -5`
Expected: Compiles successfully

- [ ] **Step 5: Commit**

```bash
git add crates/torque-harness/src/service/capability.rs
git commit -m "feat(capability): implement resolve_by_ref method"
```

---

## Phase 4: API Enhancement

### Task 5: Add Resolve Endpoint

**Files:**
- Modify: `crates/torque-harness/src/api/v1/capabilities.rs`

- [ ] **Step 1: Add resolve endpoint function**

Add at end of file (after line 167):
```rust
pub async fn resolve(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Json(req): Json<CapabilityResolveByRefRequest>,
) -> Result<Json<CapabilityResolution>, (StatusCode, Json<ErrorBody>)> {
    let resolution = services
        .capability
        .resolve_by_ref(&req.capability_ref, req.constraints)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    code: "RESOLVE_ERROR".into(),
                    message: e.to_string(),
                    details: None,
                    request_id: None,
                }),
            )
        })?;
    Ok(Json(resolution))
}
```

- [ ] **Step 2: Wire into router**

Modify `crates/torque-harness/src/api/v1/mod.rs` - add resolve route to capabilities router.

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p torque-harness 2>&1 | tail -5`
Expected: Compiles successfully

- [ ] **Step 4: Commit**

```bash
git add crates/torque-harness/src/api/v1/capabilities.rs
git commit -m "feat(capability): add resolve endpoint"
```

---

## Phase 5: Tests

### Task 6: Add Capability Resolution Test

**Files:**
- Create: `crates/torque-harness/tests/capability_resolution_tests.rs`

- [ ] **Step 1: Create test file**

```rust
use serial_test::serial;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use torque_harness::db::Database;
use torque_harness::models::v1::capability::{
    CapabilityProfileCreate, CapabilityRegistryBindingCreate, CapabilityResolveByRefRequest,
    RiskLevel,
};
use torque_harness::repository::{
    PostgresAgentDefinitionRepository, PostgresCapabilityProfileRepository,
    PostgresCapabilityRegistryBindingRepository,
};
use torque_harness::service::CapabilityService;

async fn setup_test_db() -> Option<Database> {
    let database_url = std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| {
        "postgres://postgres:postgres@localhost/torque_harness_test".to_string()
    });
    let pool = match PgPoolOptions::new().connect_lazy(&database_url) {
        Ok(pool) => pool,
        Err(_) => return None,
    };
    Some(Database::new(pool))
}

#[tokio::test]
#[serial]
async fn test_capability_resolve_by_ref() {
    let Some(db) = setup_test_db().await else { return; };

    let profile_repo = Arc::new(PostgresCapabilityProfileRepository::new(db.clone()));
    let binding_repo = Arc::new(PostgresCapabilityRegistryBindingRepository::new(db.clone()));
    let def_repo = Arc::new(PostgresAgentDefinitionRepository::new(db.clone()));
    let service = CapabilityService::new(profile_repo.clone(), binding_repo.clone());

    // Create agent definition first
    let def = def_repo.create(&torque_harness::models::v1::agent_definition::AgentDefinitionCreate {
        name: "test-agent".to_string(),
        description: None,
        system_prompt: None,
        tool_policy: serde_json::json!({}),
        memory_policy: serde_json::json!({}),
        delegation_policy: serde_json::json!({}),
        limits: serde_json::json!({}),
        default_model_policy: serde_json::json!({}),
    }).await.unwrap();

    // Create profile
    let profile = service.create_profile(CapabilityProfileCreate {
        name: "test.resolution".to_string(),
        description: Some("Test capability".to_string()),
        input_contract: None,
        output_contract: None,
        risk_level: RiskLevel::Low,
        default_agent_definition_id: None,
    }).await.unwrap();

    // Create binding
    service.create_binding(CapabilityRegistryBindingCreate {
        capability_profile_id: profile.id,
        agent_definition_id: def.id,
        compatibility_score: Some(0.9),
        quality_tier: torque_harness::models::v1::capability::QualityTier::Production,
        metadata: None,
    }).await.unwrap();

    // Resolve by name
    let resolution = service.resolve_by_ref("test.resolution", None).await.unwrap();

    assert_eq!(resolution.capability_ref, "test.resolution");
    assert_eq!(resolution.capability_profile_id, profile.id);
    assert_eq!(resolution.candidates.len(), 1);
    assert_eq!(resolution.candidates[0].agent_definition_id, def.id);
    assert_eq!(resolution.candidates[0].compatibility_score, Some(0.9));
}

#[tokio::test]
#[serial]
async fn test_capability_resolve_not_found() {
    let Some(db) = setup_test_db().await else { return; };

    let profile_repo = Arc::new(PostgresCapabilityProfileRepository::new(db.clone()));
    let binding_repo = Arc::new(PostgresCapabilityRegistryBindingRepository::new(db.clone()));
    let service = CapabilityService::new(profile_repo.clone(), binding_repo.clone());

    // Resolve non-existent capability
    let result = service.resolve_by_ref("nonexistent.capability", None).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}
```

- [ ] **Step 2: Run tests**

Run: `cd crates/torque-harness && cargo test capability_resolution -- --nocapture`
Expected: Both tests pass

- [ ] **Step 3: Run full test suite**

Run: `cargo test -p torque-harness 2>&1 | grep "test result"`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/torque-harness/tests/capability_resolution_tests.rs
git commit -m "test(capability): add capability resolution tests"
```

---

## Phase 6: Final Verification

### Task 7: Workspace Verification

- [ ] **Step 1: Run cargo check on workspace**

Run: `cargo check --workspace 2>&1 | tail -10`
Expected: Clean compilation

- [ ] **Step 2: Run all torque-harness tests**

Run: `cargo test -p torque-harness 2>&1 | grep -E "^(test result|running)"`
Expected: All tests pass

---

## Summary

| Phase | Task | Description |
|-------|------|-------------|
| 1 | Task 1 | Add CapabilityRef and resolution types |
| 2 | Task 2 | Add list_by_profile to binding repository |
| 2 | Task 3 | Add get_by_name to profile repository |
| 3 | Task 4 | Implement resolve_by_ref |
| 4 | Task 5 | Add resolve endpoint |
| 5 | Task 6 | Add resolution tests |
| 6 | Task 7 | Final verification |

**Key Deliverables:**
1. `CapabilityRef` - newtype wrapper around String
2. `CapabilityResolution` - resolution result with ordered candidates
3. `resolve_by_ref()` - actual implementation
4. `POST /v1/capabilities/resolve` - REST endpoint
5. Tests verifying resolution flow

**Spec Reference:** `docs/superpowers/specs/2026-04-08-torque-capability-registry-model-design.md`
