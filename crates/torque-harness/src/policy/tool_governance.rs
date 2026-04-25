use crate::models::v1::tool_policy::{ToolGovernanceConfig, ToolRiskLevel};
use std::collections::HashMap;
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

        let config = self.config.read().await;
        let risk = if config.blocked_tools.contains(&tool_name.to_string()) {
            ToolRiskLevel::Critical
        } else if config.privileged_tools.contains(&tool_name.to_string()) {
            ToolRiskLevel::High
        } else {
            config.default_risk_level
        };

        self.risk_cache
            .write()
            .await
            .insert(tool_name.to_string(), risk);
        risk
    }

    pub async fn should_block(&self, tool_name: &str) -> Option<String> {
        let config = self.config.read().await;
        if config.blocked_tools.contains(&tool_name.to_string()) {
            Some(format!(
                "Tool '{}' is blocked by governance policy",
                tool_name
            ))
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
