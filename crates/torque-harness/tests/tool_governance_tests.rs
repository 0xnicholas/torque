#[cfg(test)]
mod tests {
    use torque_harness::models::v1::tool_policy::{ToolGovernanceConfig, ToolRiskLevel};
    use torque_harness::policy::tool_governance::ToolGovernanceService;

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

    #[tokio::test]
    async fn test_should_block_blocked_tool() {
        let config = ToolGovernanceConfig {
            default_risk_level: ToolRiskLevel::Medium,
            approval_required_above: ToolRiskLevel::High,
            blocked_tools: vec!["evil_tool".to_string()],
            privileged_tools: vec![],
            side_effect_tracking: true,
        };

        let service = ToolGovernanceService::new(config);
        let blocked = service.should_block("evil_tool").await;

        assert!(blocked.is_some());
        assert!(blocked.unwrap().contains("evil_tool"));
    }

    #[tokio::test]
    async fn test_should_not_block_non_blocked_tool() {
        let config = ToolGovernanceConfig {
            default_risk_level: ToolRiskLevel::Medium,
            approval_required_above: ToolRiskLevel::High,
            blocked_tools: vec!["evil_tool".to_string()],
            privileged_tools: vec![],
            side_effect_tracking: true,
        };

        let service = ToolGovernanceService::new(config);
        let blocked = service.should_block("good_tool").await;

        assert!(blocked.is_none());
    }
}
