# P7: Tool Governance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement policy-based tool governance with risk categorization, side-effect tracking, privileged action gates, and enforcement during tool execution.

**Architecture:** Tool governance uses two components:

1. **`ToolGovernanceService`** - Provides tool risk levels from configuration (blocked_tools, privileged_tools, default_risk_level). Feeds risk context into policy evaluation.

2. **`GovernedToolRegistry`** - Wraps `ToolRegistry` and enforces governance during execution. Uses BOTH `ToolGovernanceService` (for risk-based gating) AND `PolicyEvaluator` (for multi-source dimensional policy per the policy spec).

**Policy evaluation flow (per policy spec Section 11):**
1. Identify subject (tool_call) and dimensions
2. Collect applicable policy sources (system, capability, agent, team, selector, runtime)
3. Evaluate each dimension via `PolicyEvaluator`
4. Conservative merge within dimension
5. Return `PolicyDecision` with `allowed`, `requires_approval`, and `reasons`
6. Runtime handles result: allow execution, gate for approval, or block

**Approval vs Blocking:**
- `decision.allowed == false` → block with reasons.first()
- `decision.requires_approval == true` → return `ToolResult` indicating approval needed (not block)

**Tech Stack:** Rust, sqlx, reqwest (for webhook notifications), existing PolicyEvaluator

---

## File Structure

```
crates/torque-harness/src/
├── policy/
│   ├── mod.rs              # Add ToolRiskLevel, ToolGovernancePolicy exports
│   └── tool_governance.rs  # NEW: ToolGovernanceService, risk categorization
├── service/
│   ├── mod.rs              # Add ToolGovernanceService to ServiceContainer
│   └── tool.rs             # Add governance-aware tool execution
├── infra/
│   └── tool_registry.rs     # Add governance wrapper capability
└── api/v1/
    ├── mod.rs               # Add tool governance routes
    └── tool_policy.rs       # NEW: API endpoints for tool policy management

crates/torque-harness/src/models/v1/
└── tool_policy.rs          # NEW: ToolPolicy, ToolRiskLevel, ToolGovernanceConfig models
```

---

## Task 1: Tool Risk Model and Governance Policy

**Files:**
- Create: `crates/torque-harness/src/models/v1/tool_policy.rs`
- Modify: `crates/torque-harness/src/models/v1/mod.rs`
- Create: `crates/db/migrations/008_add_tool_governance.up.sql`

- [ ] **Step 1: Create tool_policy.rs with risk models**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolRiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl ToolRiskLevel {
    pub fn requires_approval(&self) -> bool {
        matches!(self, ToolRiskLevel::High | ToolRiskLevel::Critical)
    }
    
    pub fn is_privileged(&self) -> bool {
        matches!(self, ToolRiskLevel::Critical)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolPolicy {
    pub tool_name: String,
    pub risk_level: ToolRiskLevel,
    pub side_effects: Vec<ToolSideEffect>,
    pub requires_approval: bool,
    pub blocked: bool,
    pub blocked_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolSideEffect {
    FileSystem,
    Network,
    ExternalProcess,
    StateMutation,
    DataExfiltration,
    SystemLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolGovernanceConfig {
    pub default_risk_level: ToolRiskLevel,
    pub approval_required_above: ToolRiskLevel,
    pub blocked_tools: Vec<String>,
    pub privileged_tools: Vec<String>,
    pub side_effect_tracking: bool,
}
```

- [ ] **Step 2: Export from mod.rs**

Add to `crates/torque-harness/src/models/v1/mod.rs`:
```rust
pub mod tool_policy;
pub use tool_policy::{ToolPolicy, ToolRiskLevel, ToolSideEffect, ToolGovernanceConfig};
```

- [ ] **Step 3: Create migration**

```sql
-- 008_add_tool_governance.up.sql
CREATE TABLE IF NOT EXISTS v1_tool_policies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tool_name VARCHAR(255) NOT NULL UNIQUE,
    risk_level VARCHAR(50) NOT NULL DEFAULT 'medium',
    side_effects TEXT[] NOT NULL DEFAULT '{}',
    requires_approval BOOLEAN NOT NULL DEFAULT false,
    blocked BOOLEAN NOT NULL DEFAULT false,
    blocked_reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_tool_policies_risk_level ON v1_tool_policies(risk_level);
CREATE INDEX idx_tool_policies_blocked ON v1_tool_policies(blocked) WHERE blocked = true;
```

- [ ] **Step 4: Run cargo check**

```bash
cargo check -p torque-harness
```

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(P7): add tool risk model and governance policy"
```

---

## Task 2: ToolGovernanceService

**Files:**
- Create: `crates/torque-harness/src/policy/tool_governance.rs`
- Modify: `crates/torque-harness/src/policy/mod.rs`
- Modify: `crates/torque-harness/src/service/mod.rs`

- [ ] **Step 1: Create ToolGovernanceService**

```rust
use crate::models::v1::tool_policy::{ToolGovernanceConfig, ToolRiskLevel, ToolSideEffect};
use crate::policy::PolicyDecision;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct ToolGovernanceService {
    config: RwLock<ToolGovernanceConfig>,
    risk_cache: RwLock<HashMap<String, ToolRiskLevel>>,
}

impl ToolGovernanceService {
    pub fn new(config: ToolGovernanceConfig) -> Self {
        Self {
            config: RwLock::new(config),
            risk_cache: RwLock::new(HashMap::new()),
        }
    }

    pub async fn get_risk_level(&self, tool_name: &str) -> ToolRiskLevel {
        if let Some(cached) = self.risk_cache.read().await.get(tool_name) {
            return *cached;
        }
        
        // Check if tool is in blocked or privileged list
        let config = self.config.read().await;
        if config.blocked_tools.contains(&tool_name.to_string()) {
            return ToolRiskLevel::Critical;
        }
        if config.privileged_tools.contains(&tool_name.to_string()) {
            return ToolRiskLevel::High;
        }
        
        config.default_risk_level
    }

    pub async fn should_block(&self, tool_name: &str) -> Option<String> {
        let config = self.config.read().await;
        if config.blocked_tools.contains(&tool_name.to_string()) {
            Some(format!("Tool '{}' is blocked by governance policy", tool_name))
        } else {
            None
        }
    }

    pub async fn requires_approval(&self, tool_name: &str) -> bool {
        let risk_level = self.get_risk_level(tool_name).await;
        let config = self.config.read().await;
        risk_level.requires_approval() || config.approval_required_above == risk_level
    }

    pub fn risk_level_to_u8(level: &ToolRiskLevel) -> u8 {
        match level {
            ToolRiskLevel::Low => 1,
            ToolRiskLevel::Medium => 2,
            ToolRiskLevel::High => 3,
            ToolRiskLevel::Critical => 4,
        }
    }

    pub async fn update_config(&self, config: ToolGovernanceConfig) {
        *self.config.write().await = config;
        self.risk_cache.write().await.clear();
    }
}
```

- [ ] **Step 2: Update policy/mod.rs**

```rust
pub mod evaluator;
pub mod tool_governance;

pub use evaluator::PolicyEvaluator;
pub use tool_governance::ToolGovernanceService;
```

- [ ] **Step 3: Add to ServiceContainer**

Modify `crates/torque-harness/src/service/mod.rs` to add `tool_governance: Arc<ToolGovernanceService>` to ServiceContainer.

- [ ] **Step 4: Run cargo check**

```bash
cargo check -p torque-harness
```

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(P7): add ToolGovernanceService"
```

---

## Task 3: Governance-Aware Tool Execution

**Files:**
- Modify: `crates/torque-harness/src/harness/react.rs`
- Modify: `crates/torque-harness/src/infra/tool_registry.rs`
- Create: `crates/torque-harness/src/service/governed_tool.rs`

- [ ] **Step 1: Create GovernedToolRegistry wrapper**

```rust
use crate::infra::tool_registry::ToolRegistry;
use crate::policy::PolicyDecision;
use crate::policy::PolicyEvaluator;
use crate::policy::PolicyInput;
use crate::policy::PolicySources;
use crate::service::tool_governance::ToolGovernanceService;
use crate::tools::{ToolArc, ToolResult};
use serde_json::Value;
use std::sync::Arc;

pub struct GovernedToolRegistry {
    inner: Arc<ToolRegistry>,
    governance: Arc<ToolGovernanceService>,
    policy_evaluator: PolicyEvaluator,
}

impl GovernedToolRegistry {
    pub fn new(
        inner: Arc<ToolRegistry>,
        governance: Arc<ToolGovernanceService>,
    ) -> Self {
        Self {
            inner,
            governance,
            policy_evaluator: PolicyEvaluator::new(),
        }
    }

    pub async fn execute(
        &self,
        name: &str,
        args: Value,
        policy_sources: Option<&PolicySources>,
    ) -> anyhow::Result<ToolResult> {
        // 1. Check if blocked by governance config
        if let Some(reason) = self.governance.should_block(name).await {
            return Ok(ToolResult {
                success: false,
                content: String::new(),
                error: Some(reason),
            });
        }

        // 2. Check policy evaluator if sources provided
        if let Some(sources) = policy_sources {
            let input = PolicyInput {
                action_type: "tool_call".to_string(),
                tool_name: Some(name.to_string()),
                ..Default::default()
            };
            let decision = self.policy_evaluator.evaluate(&input, sources);
            
            // Block if not allowed
            if !decision.allowed {
                return Ok(ToolResult {
                    success: false,
                    content: String::new(),
                    error: Some(decision.reasons.first().cloned().unwrap_or_else(|| "Tool blocked by policy".to_string())),
                });
            }
            
            // Return approval-required indicator if requires_approval
            if decision.requires_approval {
                return Ok(ToolResult {
                    success: false,
                    content: "TOOL_REQUIRES_APPROVAL".to_string(),
                    error: Some("Tool requires approval before execution".to_string()),
                });
            }
        }

        // 3. Execute tool
        self.inner.execute(name, args).await
    }

    pub async fn get(&self, name: &str) -> Option<ToolArc> {
        self.inner.get(name).await
    }

    pub async fn list(&self) -> Vec<ToolArc> {
        self.inner.list().await
    }

    pub async fn to_llm_tools(&self) -> Vec<llm::ToolDef> {
        self.inner.to_llm_tools().await
    }
}
```

- [ ] **Step 2: Update ReActHarness to use governed execution**

Modify `crates/torque-harness/src/harness/react.rs`:
- Change `tools: Arc<ToolRegistry>` to `tools: Arc<dyn ToolExecution>`
- Add `execute_tool_with_governance()` method

```rust
pub trait ToolExecution: Send + Sync {
    async fn execute(&self, name: &str, args: Value) -> anyhow::Result<ToolResult>;
}

impl ToolExecution for GovernedToolRegistry {
    async fn execute(&self, name: &str, args: Value) -> anyhow::Result<ToolResult> {
        self.execute(name, args, None).await
    }
}
```

- [ ] **Step 3: Update execute_tool to use governance**

```rust
async fn execute_tool(
    &self,
    tool_call: &ToolCall,
    event_sink: mpsc::Sender<StreamEvent>,
) -> Result<ToolResult, ReActHarnessError> {
    // ... existing code ...

    let result = self
        .tools
        .execute(&tool_call.name, tool_call.arguments.clone())
        .await;

    // ... rest unchanged ...
}
```

- [ ] **Step 4: Run cargo check**

```bash
cargo check -p torque-harness
```

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(P7): add governance-aware tool execution"
```

---

## Task 4: Tool Governance API Endpoints

**Files:**
- Create: `crates/torque-harness/src/api/v1/tool_policy.rs`
- Modify: `crates/torque-harness/src/api/v1/mod.rs`
- Create: `crates/torque-harness/src/repository/tool_policy.rs`
- Modify: `crates/torque-harness/src/repository/mod.rs`

- [ ] **Step 1: Create ToolPolicyRepository**

```rust
use crate::models::v1::tool_policy::{ToolPolicy, ToolRiskLevel};
use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait ToolPolicyRepository: Send + Sync {
    async fn upsert(&self, policy: &ToolPolicy) -> anyhow::Result<()>;
    async fn get(&self, tool_name: &str) -> anyhow::Result<Option<ToolPolicy>>;
    async fn list(&self) -> anyhow::Result<Vec<ToolPolicy>>;
    async fn delete(&self, tool_name: &str) -> anyhow::Result<()>;
}
```

- [ ] **Step 2: Create API endpoints**

```rust
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json, Router,
};
use std::sync::Arc;

pub async fn list_tool_policies(
    State(repo): State<Arc<dyn ToolPolicyRepository>>,
) -> Result<Json<Vec<ToolPolicy>>, StatusCode> {
    repo.list()
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn get_tool_policy(
    State(repo): State<Arc<dyn ToolPolicyRepository>>,
    Path(tool_name): Path<String>,
) -> Result<Json<ToolPolicy>, StatusCode> {
    repo.get(&tool_name)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

pub async fn upsert_tool_policy(
    State(repo): State<Arc<dyn ToolPolicyRepository>>,
    Path(tool_name): Path<String>,
    Json(policy): Json<ToolPolicy>,
) -> Result<impl IntoResponse, StatusCode> {
    repo.upsert(&policy)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::CREATED)
}

pub async fn delete_tool_policy(
    State(repo): State<Arc<dyn ToolPolicyRepository>>,
    Path(tool_name): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    repo.delete(&tool_name)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}
```

- [ ] **Step 3: Add routes to mod.rs**

```rust
pub mod tool_policy;

pub fn tool_policy_routes() -> Router {
    Router::new()
        .route("/tool-policies", get(list_tool_policies))
        .route("/tool-policies/{tool_name}", get(get_tool_policy))
        .route("/tool-policies/{tool_name}", post(upsert_tool_policy))
        .route("/tool-policies/{tool_name}", delete(delete_tool_policy))
}
```

- [ ] **Step 4: Run cargo check**

```bash
cargo check -p torque-harness
```

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(P7): add tool governance API endpoints"
```

---

## Task 5: Integration and Testing

**Files:**
- Modify: `crates/torque-harness/src/service/run.rs`
- Create: `crates/torque-harness/tests/tool_governance_tests.rs`

- [ ] **Step 1: Wire ToolGovernanceService into RunService**

Modify `RunService` to use `GovernedToolRegistry` instead of raw `ToolRegistry`.

- [ ] **Step 2: Add ToolGovernanceService to ServiceContainer**

Ensure `ToolGovernanceService` is properly initialized with default config.

- [ ] **Step 3: Create tool_governance_tests.rs**

```rust
#[cfg(test)]
mod tests {
    use crate::models::v1::tool_policy::{ToolGovernanceConfig, ToolRiskLevel};
    use crate::policy::tool_governance::ToolGovernanceService;

    #[tokio::test]
    async fn test_blocked_tool_returns_critical_risk() {
        let config = ToolGovernanceConfig {
            default_risk_level: ToolRiskLevel::Medium,
            approval_required_above: ToolRiskLevel::High,
            blocked_tools: vec!["dangerous_tool".to_string()],
            privileged_tools: vec![],
            side_effect_tracking: true,
        };
        
        let service = ToolGovernanceService::new(config);
        let risk = service.get_risk_level("dangerous_tool").await;
        
        assert_eq!(risk, ToolRiskLevel::Critical);
    }

    #[tokio::test]
    async fn test_privileged_tool_returns_high_risk() {
        let config = ToolGovernanceConfig {
            default_risk_level: ToolRiskLevel::Low,
            approval_required_above: ToolRiskLevel::High,
            blocked_tools: vec![],
            privileged_tools: vec!["file_write".to_string()],
            side_effect_tracking: true,
        };
        
        let service = ToolGovernanceService::new(config);
        let risk = service.get_risk_level("file_write").await;
        
        assert_eq!(risk, ToolRiskLevel::High);
    }

    #[tokio::test]
    async fn test_unknown_tool_uses_default_risk() {
        let config = ToolGovernanceConfig {
            default_risk_level: ToolRiskLevel::Low,
            approval_required_above: ToolRiskLevel::High,
            blocked_tools: vec![],
            privileged_tools: vec![],
            side_effect_tracking: false,
        };
        
        let service = ToolGovernanceService::new(config);
        let risk = service.get_risk_level("unknown_tool").await;
        
        assert_eq!(risk, ToolRiskLevel::Low);
    }
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p torque-harness -- tool_governance
```

- [ ] **Step 5: Run all tests**

```bash
cargo test -p torque-harness
```

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat(P7): integrate tool governance and add tests"
```

---

## Task 6: Documentation Update

**Files:**
- Modify: `STATUS.md`

- [ ] **Step 1: Update STATUS.md**

Add P7 section with tool governance details:
- Tool risk categorization (Low, Medium, High, Critical)
- ToolGovernanceService with policy evaluation
- Governance-aware tool execution
- API endpoints for tool policy management

- [ ] **Step 2: Commit**

```bash
git add STATUS.md && git commit -m "docs: update STATUS.md for P7 Tool Governance"
```

---

## Summary

| Task | Description |
|------|-------------|
| 1 | Tool Risk Model and Governance Policy |
| 2 | ToolGovernanceService |
| 3 | Governance-Aware Tool Execution |
| 4 | Tool Governance API Endpoints |
| 5 | Integration and Testing |
| 6 | Documentation Update |

**Total: 6 Tasks, ~25 Steps**

---

## Dependencies

- Policy spec: `docs/superpowers/specs/2026-04-08-torque-policy-model-design.md` (tool dimension)
- Existing PolicyEvaluator: `crates/torque-harness/src/policy/evaluator.rs`
- Existing ToolRegistry: `crates/torque-harness/src/infra/tool_registry.rs`
