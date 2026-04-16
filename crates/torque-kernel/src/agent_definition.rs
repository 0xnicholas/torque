use serde::{Deserialize, Serialize};

use crate::ids::AgentDefinitionId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentDefinition {
    pub id: AgentDefinitionId,
    pub name: String,
    pub system_prompt: String,
    pub tool_policy_ref: Option<String>,
    pub memory_policy_ref: Option<String>,
    pub delegation_policy_ref: Option<String>,
    pub default_model_policy_ref: Option<String>,
    pub limits: AgentLimits,
}

impl AgentDefinition {
    pub fn new(name: impl Into<String>, system_prompt: impl Into<String>) -> Self {
        Self {
            id: AgentDefinitionId::new(),
            name: name.into(),
            system_prompt: system_prompt.into(),
            tool_policy_ref: None,
            memory_policy_ref: None,
            delegation_policy_ref: None,
            default_model_policy_ref: None,
            limits: AgentLimits::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AgentLimits {
    pub max_turns: Option<u32>,
    pub max_child_delegations: Option<u32>,
}
