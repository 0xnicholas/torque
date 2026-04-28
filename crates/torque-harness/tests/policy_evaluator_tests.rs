use torque_harness::policy::{PolicyDecision, PolicyEvaluator, PolicyInput, PolicySources};

// ── Helper ──────────────────────────────────────────────────────────

fn empty_sources() -> PolicySources {
    PolicySources::new()
}

fn system_policy(json: serde_json::Value) -> PolicySources {
    PolicySources::new().with_system(json)
}

fn agent_policy(json: serde_json::Value) -> PolicySources {
    PolicySources::new().with_agent(json)
}

fn team_policy(json: serde_json::Value) -> PolicySources {
    PolicySources::new().with_team(json)
}

fn eval(sources: &PolicySources) -> PolicyDecision {
    eval_with_tool("test_tool", sources)
}

fn eval_with_tool(tool_name: &str, sources: &PolicySources) -> PolicyDecision {
    let evaluator = PolicyEvaluator::new();
    let input = PolicyInput {
        action_type: "tool_call".to_string(),
        tool_name: Some(tool_name.to_string()),
        ..Default::default()
    };
    evaluator.evaluate(&input, sources)
}

// ── Conservative merge ──────────────────────────────────────────────

#[test]
fn empty_sources_return_default_allowed() {
    let decision = eval(&empty_sources());
    assert!(decision.allowed);
    assert!(!decision.requires_approval);
    assert!(decision.tool_restrictions.is_empty());
    assert!(decision.approval_dimensions.is_empty());
    assert!(decision.delegation_restrictions.is_empty());
    assert!(decision.resource_limits.is_empty());
    assert!(decision.memory_restrictions.is_empty());
}

#[test]
fn tool_forbidden_in_any_source_denies() {
    let sources = agent_policy(serde_json::json!({"forbidden_tools": ["danger"]}));
    let decision = eval_with_tool("danger", &sources);
    assert!(!decision.allowed);
    assert!(decision.reasons.iter().any(|r| r.contains("danger")));
}

#[test]
fn approval_required_in_any_source_sets_requires_approval() {
    let sources = team_policy(serde_json::json!({"approval_required": true}));
    let decision = eval(&sources);
    assert!(decision.requires_approval);
    assert!(decision.reasons.iter().any(|r| r.contains("Approval required")));
}

#[test]
fn visibility_scope_narrow_restricts() {
    let sources = system_policy(serde_json::json!({"visibility_scope": "narrow"}));
    let decision = eval(&sources);
    assert_eq!(decision.visibility_restriction.as_deref(), Some("narrow"));
}

#[test]
fn delegation_not_allowed_adds_restriction() {
    let sources = agent_policy(serde_json::json!({"delegation_allowed": false}));
    let decision = eval(&sources);
    assert!(!decision.delegation_restrictions.is_empty());
}

#[test]
fn delegation_max_depth_adds_restriction() {
    let sources = team_policy(serde_json::json!({"max_delegation_depth": 1}));
    let decision = eval(&sources);
    assert!(decision
        .delegation_restrictions
        .iter()
        .any(|r| r.contains("max_delegation_depth=1")));
}

#[test]
fn child_delegation_disallowed_adds_restriction() {
    let sources = agent_policy(serde_json::json!({"child_delegation_allowed": false}));
    let decision = eval(&sources);
    assert!(decision
        .delegation_restrictions
        .iter()
        .any(|r| r.contains("child_delegation_disallowed")));
}

#[test]
fn resource_budget_cap_adds_limit() {
    let sources = system_policy(serde_json::json!({"resource_budget_cap": 500}));
    let decision = eval(&sources);
    assert!(decision
        .resource_limits
        .iter()
        .any(|r| r.contains("budget_cap=500")));
}

#[test]
fn resource_max_concurrency_adds_limit() {
    let sources = agent_policy(serde_json::json!({"max_concurrency": 3}));
    let decision = eval(&sources);
    assert!(decision
        .resource_limits
        .iter()
        .any(|r| r.contains("max_concurrency=3")));
}

#[test]
fn resource_timeout_adds_limit() {
    let sources = team_policy(serde_json::json!({"timeout_seconds": 120}));
    let decision = eval(&sources);
    assert!(decision
        .resource_limits
        .iter()
        .any(|r| r.contains("timeout=120")));
}

#[test]
fn resource_defer_under_pressure_adds_limit() {
    let sources = system_policy(serde_json::json!({"defer_under_pressure": true}));
    let decision = eval(&sources);
    assert!(decision
        .resource_limits
        .iter()
        .any(|r| r.contains("defer_under_pressure")));
}

#[test]
fn memory_write_denied_adds_restriction() {
    let sources = agent_policy(serde_json::json!({"memory_write_allowed": false}));
    let decision = eval(&sources);
    assert!(decision
        .memory_restrictions
        .iter()
        .any(|r| r.contains("memory_write_denied")));
}

#[test]
fn memory_candidate_only_adds_restriction() {
    let sources = team_policy(serde_json::json!({"memory_candidate_only": true}));
    let decision = eval(&sources);
    assert!(decision
        .memory_restrictions
        .iter()
        .any(|r| r.contains("memory_candidate_only")));
}

#[test]
fn memory_max_entries_adds_restriction() {
    let sources = system_policy(serde_json::json!({"max_memory_entries": 100}));
    let decision = eval(&sources);
    assert!(decision
        .memory_restrictions
        .iter()
        .any(|r| r.contains("max_entries=100")));
}

#[test]
fn memory_require_review_adds_restriction() {
    let sources = agent_policy(serde_json::json!({"require_review_before_write": true}));
    let decision = eval(&sources);
    assert!(decision
        .memory_restrictions
        .iter()
        .any(|r| r.contains("require_review_before_write")));
}

// ── Cross-dimension isolation ───────────────────────────────────────

#[test]
fn tool_deny_does_not_block_memory() {
    let sources = agent_policy(serde_json::json!({
        "forbidden_tools": ["danger"],
        "memory_write_allowed": true
    }));
    let decision = eval_with_tool("danger", &sources);
    // tool is denied, but memory is unrestricted
    assert!(!decision.allowed);
    assert!(decision.memory_restrictions.is_empty());
}

#[test]
fn memory_deny_does_not_block_tool() {
    let sources = team_policy(serde_json::json!({
        "memory_write_allowed": false,
        "allowed_tools": ["*"]
    }));
    let decision = eval(&sources);
    // memory is restricted but tool is allowed
    assert!(!decision.memory_restrictions.is_empty());
    assert!(decision.tool_restrictions.is_empty());
}

#[test]
fn approval_require_does_not_block_resource() {
    let sources = agent_policy(serde_json::json!({
        "approval_required": true,
        "resource_budget_cap": 1000
    }));
    let decision = eval(&sources);
    assert!(decision.requires_approval);
    assert!(!decision.resource_limits.is_empty());
    // approval dimension does not prevent resource from being read
}

#[test]
fn visibility_narrow_does_not_affect_delegation() {
    let sources = team_policy(serde_json::json!({
        "visibility_scope": "narrow",
        "delegation_allowed": true
    }));
    let decision = eval(&sources);
    assert_eq!(decision.visibility_restriction.as_deref(), Some("narrow"));
    assert!(decision.delegation_restrictions.is_empty());
}

// ── Multi-source merge ──────────────────────────────────────────────

#[test]
fn multi_source_conservative_merge_on_same_dimension() {
    let system = serde_json::json!({"forbidden_tools": ["x"]});
    let agent = serde_json::json!({"allowed_tools": ["*"]});
    let sources = PolicySources::new()
        .with_system(system)
        .with_agent(agent);
    let decision = eval_with_tool("x", &sources);
    // system's forbidden_tools overrides agent's allow-all
    assert!(!decision.allowed);
}

#[test]
fn three_sources_all_contribute() {
    let sources = PolicySources::new()
        .with_system(serde_json::json!({"delegation_allowed": false}))
        .with_agent(serde_json::json!({"approval_required": true}))
        .with_team(serde_json::json!({"memory_write_allowed": false}));
    let decision = eval(&sources);
    assert!(!decision.delegation_restrictions.is_empty());
    assert!(decision.requires_approval);
    assert!(!decision.memory_restrictions.is_empty());
}

#[test]
fn null_source_is_noop() {
    let sources = PolicySources::new().with_system(serde_json::json!(null));
    let decision = eval(&sources);
    assert!(decision.allowed);
    assert!(!decision.requires_approval);
}

#[test]
fn empty_object_source_is_noop() {
    let sources = agent_policy(serde_json::json!({}));
    let decision = eval(&sources);
    assert!(decision.allowed);
}

// ── Approval dimension specifics ────────────────────────────────────

#[test]
fn approval_dimensions_populated_from_requirements() {
    let sources = system_policy(serde_json::json!({
        "approval_required": true,
        "approval_requirements": ["external_approval", "supervisor_signoff"]
    }));
    let decision = eval(&sources);
    assert!(decision.requires_approval);
    assert!(decision
        .approval_dimensions
        .iter()
        .any(|d| d == "external_approval"));
    assert!(decision
        .approval_dimensions
        .iter()
        .any(|d| d == "supervisor_signoff"));
}

#[test]
fn operator_escalation_adds_approval_dimension() {
    let sources = team_policy(serde_json::json!({
        "require_operator_escalation": true
    }));
    let decision = eval(&sources);
    assert!(decision
        .approval_dimensions
        .iter()
        .any(|d| d == "operator_escalation"));
}

// ── Delegation dimension specifics ──────────────────────────────────

#[test]
fn handoff_disallowed_adds_restriction() {
    let sources = agent_policy(serde_json::json!({"handoff_allowed": false}));
    let decision = eval(&sources);
    assert!(decision
        .delegation_restrictions
        .iter()
        .any(|r| r.contains("handoff_disallowed")));
}

// ── Regression: existing tool behavior ──────────────────────────────

#[test]
fn existing_tool_require_approval_still_works() {
    let sources = agent_policy(serde_json::json!({
        "require_approval_tools": ["file_write"]
    }));
    let decision = eval_with_tool("file_write", &sources);
    assert!(decision.requires_approval);
    assert!(decision.reasons.iter().any(|r| r.contains("file_write")));
}

#[test]
fn existing_tool_allow_all_still_works() {
    let sources = agent_policy(serde_json::json!({
        "allowed_tools": ["*"]
    }));
    let decision = eval(&sources);
    assert!(decision.allowed);
}

// ── PolicyDecision merge helper ─────────────────────────────────────

#[test]
fn merge_takes_most_restrictive() {
    let a = PolicyDecision::deny("source a");
    let b = PolicyDecision::require_approval("tool", "source b");
    let merged = a.merge(b);
    assert!(!merged.allowed); // deny wins
    assert!(merged.requires_approval); // approval carried over
}
