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