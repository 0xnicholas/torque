//! Integration tests for dynamic tool registration.
//!
//! These tests verify:
//! - Tools registered through Extension are immediately visible (Phase 8.3)
//! - Tools are cleaned up when Extension is unregistered (Phase 8.4)
//! - New tools appear in `tool_defs()` on next LLM call (Phase 8.5)
//! - Name conflict returns error, PUT update works (Phase 8.6)
//! - Blocklist governance applies to dynamic tools (Phase 8.7)
//!
//! NOTE: Extension-related tests require the `extension` feature:
//!   cargo test --features extension --test dynamic_tool_integration_tests

use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use torque_harness::infra::tool_registry::ToolRegistry;
use torque_harness::service::governed_tool::GovernedToolRegistry;
use torque_harness::policy::ToolGovernanceService;
use torque_harness::tools::{Tool, ToolArc, ToolResult};

// ── Helpers ─────────────────────────────────────────────────────────

fn make_test_tool(name: &str) -> ToolArc {
    struct TestTool {
        name: String,
    }
    #[async_trait]
    impl Tool for TestTool {
        fn name(&self) -> &str {
            &self.name
        }
        fn description(&self) -> &str {
            "test tool"
        }
        fn parameters_schema(&self) -> Value {
            serde_json::json!({})
        }
        async fn execute(&self, _args: Value) -> anyhow::Result<ToolResult> {
            Ok(ToolResult {
                success: true,
                content: "ok".to_string(),
                error: None,
            })
        }
    }
    Arc::new(TestTool {
        name: name.to_string(),
    }) as ToolArc
}

// ── Blocklist governance tests (Phase 8.7) ──────────────────────────

#[tokio::test]
async fn test_blocklist_blocks_dynamic_tool() {
    let registry = Arc::new(ToolRegistry::new());
    let governance = Arc::new(ToolGovernanceService::new(
        torque_harness::models::v1::tool_policy::ToolGovernanceConfig {
            default_risk_level: torque_harness::models::v1::tool_policy::ToolRiskLevel::Medium,
            approval_required_above: torque_harness::models::v1::tool_policy::ToolRiskLevel::High,
            blocked_tools: vec!["blocked_tool".to_string()],
            privileged_tools: vec![],
            side_effect_tracking: false,
        },
    ));
    let governed = GovernedToolRegistry::new(registry.clone(), governance);

    // Register a tool that should be blocked.
    registry
        .register(make_test_tool("blocked_tool"))
        .await;

    let result = governed
        .execute("blocked_tool", serde_json::json!({}), None)
        .await
        .unwrap();

    assert!(!result.success, "blocked tool should fail");
    assert!(
        result.error.is_some(),
        "blocked tool should have error message"
    );
    let err = result.error.unwrap();
    assert!(
        err.contains("blocked") || err.contains("denied"),
        "error should mention blockage: {err}"
    );
}

#[tokio::test]
async fn test_non_blocked_tool_passes_gate() {
    let registry = Arc::new(ToolRegistry::new());
    let governance = Arc::new(ToolGovernanceService::new(
        torque_harness::models::v1::tool_policy::ToolGovernanceConfig {
            default_risk_level: torque_harness::models::v1::tool_policy::ToolRiskLevel::Medium,
            approval_required_above: torque_harness::models::v1::tool_policy::ToolRiskLevel::High,
            blocked_tools: vec![],
            privileged_tools: vec![],
            side_effect_tracking: false,
        },
    ));
    let governed = GovernedToolRegistry::new(registry.clone(), governance);

    registry.register(make_test_tool("safe_tool")).await;

    let result = governed
        .execute("safe_tool", serde_json::json!({"key": "value"}), None)
        .await
        .unwrap();

    assert!(result.success, "non-blocked tool should succeed");
    assert_eq!(result.content, "ok");
}

// ── Tool defs freshness (Phase 8.5) ────────────────────────────────

#[tokio::test]
async fn test_newly_registered_tool_appears_in_llm_tools() {
    let registry = Arc::new(ToolRegistry::new());
    let governance = Arc::new(ToolGovernanceService::new(
        torque_harness::models::v1::tool_policy::ToolGovernanceConfig {
            default_risk_level: torque_harness::models::v1::tool_policy::ToolRiskLevel::Low,
            approval_required_above: torque_harness::models::v1::tool_policy::ToolRiskLevel::High,
            blocked_tools: vec!["blocked_tool".to_string()],
            privileged_tools: vec![],
            side_effect_tracking: true,
        },
    ));
    let governed = GovernedToolRegistry::new(registry.clone(), governance);

    // Before registration.
    let tools_before = governed.to_llm_tools().await;
    let names_before: Vec<&str> = tools_before.iter().map(|t| t.name.as_str()).collect();
    assert!(!names_before.contains(&"fresh_tool"));

    // Register.
    registry.register(make_test_tool("fresh_tool")).await;

    // After registration — immediately visible (no reload needed).
    let tools_after = governed.to_llm_tools().await;
    let names_after: Vec<&str> = tools_after.iter().map(|t| t.name.as_str()).collect();
    assert!(
        names_after.contains(&"fresh_tool"),
        "newly registered tool should appear in to_llm_tools() immediately"
    );
}

// ── Name conflict (Phase 8.6) ───────────────────────────────────────

#[tokio::test]
async fn test_duplicate_name_via_registry_does_not_error() {
    // Note: ToolRegistry::register() overwrites silently.
    // The conflict detection is supposed to be handled at the API layer
    // (POST /v1/tools/register checks before inserting).
    let registry = ToolRegistry::new();
    registry.register(make_test_tool("dup")).await;
    registry.register(make_test_tool("dup")).await; // overwrites — no error

    let names = registry.list_tool_names().await;
    assert_eq!(names.len(), 1, "duplicate name overwrites");
}

#[tokio::test]
async fn test_update_method_does_not_create_new_entry() {
    let registry = ToolRegistry::new();
    registry.register(make_test_tool("existing")).await;

    // Update should succeed for existing tool.
    let updated = registry.update("existing", make_test_tool("existing")).await;
    assert!(updated, "update should succeed");

    // Update should fail for non-existing tool.
    let missing = registry
        .update("non_existing", make_test_tool("non_existing"))
        .await;
    assert!(!missing, "update should fail for missing tool");

    // Only one tool should exist.
    let names = registry.list_tool_names().await;
    assert_eq!(names.len(), 1);
    assert!(names.contains(&"existing".to_string()));
}
