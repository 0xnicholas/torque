use std::sync::Arc;
use torque_harness::infra::tool_registry::ToolRegistry;
use torque_harness::models::v1::tool_policy::ToolGovernanceConfig;
use torque_harness::models::v1::tool_policy::ToolRiskLevel;
use torque_harness::policy::evaluator::PolicyEvaluator;
use torque_harness::policy::tool_governance::ToolGovernanceService;
use torque_harness::policy::{PolicyDecision, PolicyInput, PolicySources};
use torque_harness::service::governed_tool::GovernedToolRegistry;

// ── Helpers ──────────────────────────────────────────────────────────

fn system_policy(json: serde_json::Value) -> PolicySources {
    PolicySources::new().with_system(json)
}

fn agent_policy(json: serde_json::Value) -> PolicySources {
    PolicySources::new().with_agent(json)
}

fn team_policy(json: serde_json::Value) -> PolicySources {
    PolicySources::new().with_team(json)
}

fn governance_service() -> Arc<ToolGovernanceService> {
    Arc::new(ToolGovernanceService::new(ToolGovernanceConfig {
        default_risk_level: ToolRiskLevel::Low,
        approval_required_above: ToolRiskLevel::High,
        blocked_tools: vec![],
        privileged_tools: vec![],
        side_effect_tracking: false,
    }))
}

/// A simple no-op tool for testing governance wrapping.
struct NoopTool;
#[async_trait::async_trait]
impl torque_harness::tools::Tool for NoopTool {
    fn name(&self) -> &str {
        "noop"
    }
    fn description(&self) -> &str {
        "Noop tool for testing"
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({})
    }
    async fn execute(
        &self,
        _args: serde_json::Value,
    ) -> anyhow::Result<torque_harness::tools::ToolResult> {
        Ok(torque_harness::tools::ToolResult {
            success: true,
            content: "done".to_string(),
            error: None,
        })
    }
}

// ── GovernedToolRegistry integration ─────────────────────────────────

#[tokio::test]
async fn governed_registry_blocks_forbidden_tool_via_policy_sources() {
    let registry = Arc::new(ToolRegistry::new());
    registry.register(Arc::new(NoopTool)).await;
    let governed =
        GovernedToolRegistry::new(registry.clone(), governance_service());

    let sources = agent_policy(serde_json::json!({"forbidden_tools": ["noop"]}));
    let result = governed
        .execute("noop", serde_json::json!({}), Some(&sources))
        .await
        .unwrap();

    assert!(!result.success);
    assert!(result
        .error
        .unwrap()
        .contains("forbidden by agent policy"));
}

#[tokio::test]
async fn governed_registry_blocks_require_approval_via_policy_sources() {
    let registry = Arc::new(ToolRegistry::new());
    registry.register(Arc::new(NoopTool)).await;
    let governed =
        GovernedToolRegistry::new(registry.clone(), governance_service());

    let sources =
        team_policy(serde_json::json!({"require_approval_tools": ["noop"]}));
    let result = governed
        .execute("noop", serde_json::json!({}), Some(&sources))
        .await
        .unwrap();

    assert!(!result.success);
    assert_eq!(result.content, "TOOL_REQUIRES_APPROVAL");
}

#[tokio::test]
async fn governed_registry_allows_unrestricted_tool() {
    let registry = Arc::new(ToolRegistry::new());
    registry.register(Arc::new(NoopTool)).await;
    let governed =
        GovernedToolRegistry::new(registry.clone(), governance_service());

    let result = governed
        .execute("noop", serde_json::json!({}), None)
        .await
        .unwrap();

    assert!(result.success);
    assert_eq!(result.content, "done");
}

#[tokio::test]
async fn governed_registry_allowed_tool_with_empty_policy() {
    let registry = Arc::new(ToolRegistry::new());
    registry.register(Arc::new(NoopTool)).await;
    let governed =
        GovernedToolRegistry::new(registry.clone(), governance_service());

    let result = governed
        .execute("noop", serde_json::json!({}), Some(&PolicySources::new()))
        .await
        .unwrap();

    assert!(result.success);
    assert_eq!(result.content, "done");
}

// ── Multi-dimension end-to-end ───────────────────────────────────────

#[test]
fn multi_dimension_single_evaluate_call() {
    let evaluator = PolicyEvaluator::new();

    let sources = PolicySources::new()
        .with_system(serde_json::json!({
            "forbidden_tools": ["danger"],
            "delegation_allowed": false,
            "memory_write_allowed": false
        }))
        .with_agent(serde_json::json!({
            "approval_required": true,
            "visibility_scope": "narrow"
        }));

    let input = PolicyInput {
        action_type: "tool_call".to_string(),
        tool_name: Some("danger".to_string()),
        ..Default::default()
    };

    let decision = evaluator.evaluate(&input, &sources);

    // Tool dimension: danger is forbidden
    assert!(!decision.allowed);
    // Approval dimension: approval required
    assert!(decision.requires_approval);
    // Visibility dimension: narrowed
    assert_eq!(decision.visibility_restriction.as_deref(), Some("narrow"));
    // Delegation dimension: not allowed
    assert!(!decision.delegation_restrictions.is_empty());
    // Memory dimension: write denied
    assert!(!decision.memory_restrictions.is_empty());
}

#[test]
fn dimension_isolation_memory_does_not_block_tool_execution() {
    let evaluator = PolicyEvaluator::new();

    let sources = agent_policy(serde_json::json!({
        "memory_write_allowed": false,
        "allowed_tools": ["*"]
    }));

    let input = PolicyInput {
        action_type: "tool_call".to_string(),
        tool_name: Some("safe_tool".to_string()),
        ..Default::default()
    };

    let decision = evaluator.evaluate(&input, &sources);

    // Tool execution allowed
    assert!(decision.allowed);
    // But memory is restricted
    assert!(!decision.memory_restrictions.is_empty());
    assert!(decision
        .memory_restrictions
        .iter()
        .any(|r| r.contains("memory_write_denied")));
}

#[test]
fn dimension_isolation_delegation_does_not_block_approval() {
    let evaluator = PolicyEvaluator::new();

    let sources = team_policy(serde_json::json!({
        "delegation_allowed": false,
        "approval_required": true,
        "approval_requirements": ["external_review"]
    }));

    let input = PolicyInput {
        action_type: "delegation".to_string(),
        ..Default::default()
    };

    let decision = evaluator.evaluate(&input, &sources);

    // Delegation should be restricted
    assert!(!decision.delegation_restrictions.is_empty());
    // Approval should ALSO be required (independent dimension)
    assert!(decision.requires_approval);
    assert!(decision
        .approval_dimensions
        .iter()
        .any(|d| d == "external_review"));
}

// ── Full PolicySources coverage ──────────────────────────────────────

#[test]
fn full_six_source_merge_all_dimensions() {
    let mut sources = PolicySources::new()
        .with_system(serde_json::json!({"max_delegation_depth": 2, "timeout_seconds": 300}))
        .with_capability(serde_json::json!({"approval_required": true}))
        .with_agent(serde_json::json!({"forbidden_tools": ["rm"], "max_memory_entries": 100, "require_review_before_write": true}))
        .with_team(serde_json::json!({"visibility_scope": "narrow"}));
    sources.runtime = Some(serde_json::json!({"max_concurrency": 5}));

    let input = PolicyInput {
        action_type: "tool_call".to_string(),
        tool_name: Some("rm".to_string()),
        ..Default::default()
    };

    let evaluator = PolicyEvaluator::new();
    let decision = evaluator.evaluate(&input, &sources);

    assert!(!decision.allowed); // rm forbidden
    assert!(decision.requires_approval); // capability
    assert_eq!(decision.visibility_restriction.as_deref(), Some("narrow")); // team
    assert!(decision.delegation_restrictions.iter().any(|r| r.contains("max_delegation_depth=2"))); // system
    assert!(decision.resource_limits.iter().any(|r| r.contains("timeout=300"))); // system
    assert!(decision.resource_limits.iter().any(|r| r.contains("max_concurrency=5"))); // selector
    assert!(!decision.memory_restrictions.is_empty()); // agent + team
}

// ── PolicyDecision merge behavior ────────────────────────────────────

#[test]
fn merge_conservative_allowed_false_beats_true() {
    let allow = PolicyDecision::default();
    let deny = PolicyDecision::deny("reason");
    let merged = allow.merge(deny);
    assert!(!merged.allowed);
}

#[test]
fn merge_accumulates_dimension_fields() {
    let a = PolicyDecision {
        tool_restrictions: vec!["a".into()],
        delegation_restrictions: vec!["d1".into()],
        ..PolicyDecision::default()
    };
    let b = PolicyDecision {
        tool_restrictions: vec!["b".into()],
        delegation_restrictions: vec!["d2".into()],
        memory_restrictions: vec!["m1".into()],
        ..PolicyDecision::default()
    };
    let merged = a.merge(b);
    assert_eq!(merged.tool_restrictions.len(), 2);
    assert_eq!(merged.delegation_restrictions.len(), 2);
    assert_eq!(merged.memory_restrictions.len(), 1);
}
