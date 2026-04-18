use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PolicyOutcome {
    Allow,
    Deny,
    RequireApproval,
    NarrowVisibility,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DimensionResult {
    pub allowed: bool,
    pub requires_followup: bool,
    pub restrictions: Vec<String>,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyDecision {
    pub allowed: bool,
    pub requires_approval: bool,
    pub approval_dimensions: Vec<String>,
    pub tool_restrictions: Vec<String>,
    pub visibility_restriction: Option<String>,
    pub delegation_restrictions: Vec<String>,
    pub resource_limits: Vec<String>,
    pub memory_restrictions: Vec<String>,
    pub reasons: Vec<String>,
}

impl Default for PolicyDecision {
    fn default() -> Self {
        Self {
            allowed: true,
            requires_approval: false,
            approval_dimensions: Vec::new(),
            tool_restrictions: Vec::new(),
            visibility_restriction: None,
            delegation_restrictions: Vec::new(),
            resource_limits: Vec::new(),
            memory_restrictions: Vec::new(),
            reasons: Vec::new(),
        }
    }
}

impl PolicyDecision {
    pub fn merge(mut self, other: PolicyDecision) -> Self {
        // Conservative merge: more restrictive wins
        self.allowed = self.allowed && other.allowed;
        self.requires_approval = self.requires_approval || other.requires_approval;
        self.approval_dimensions.extend(other.approval_dimensions);
        self.tool_restrictions.extend(other.tool_restrictions);

        if other.visibility_restriction.is_some() {
            self.visibility_restriction = other.visibility_restriction;
        }

        self.delegation_restrictions
            .extend(other.delegation_restrictions);
        self.resource_limits.extend(other.resource_limits);
        self.memory_restrictions.extend(other.memory_restrictions);
        self.reasons.extend(other.reasons);

        self
    }

    pub fn deny(reason: impl Into<String>) -> Self {
        Self {
            allowed: false,
            requires_approval: false,
            approval_dimensions: Vec::new(),
            tool_restrictions: Vec::new(),
            visibility_restriction: None,
            delegation_restrictions: Vec::new(),
            resource_limits: Vec::new(),
            memory_restrictions: Vec::new(),
            reasons: vec![reason.into()],
        }
    }

    pub fn require_approval(dimension: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            allowed: true,
            requires_approval: true,
            approval_dimensions: vec![dimension.into()],
            tool_restrictions: Vec::new(),
            visibility_restriction: None,
            delegation_restrictions: Vec::new(),
            resource_limits: Vec::new(),
            memory_restrictions: Vec::new(),
            reasons: vec![reason.into()],
        }
    }

    pub fn restrict_tool(tool_name: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            allowed: true,
            requires_approval: false,
            approval_dimensions: Vec::new(),
            tool_restrictions: vec![tool_name.into()],
            visibility_restriction: None,
            delegation_restrictions: Vec::new(),
            resource_limits: Vec::new(),
            memory_restrictions: Vec::new(),
            reasons: vec![reason.into()],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PolicyInput {
    pub agent_definition_id: Option<uuid::Uuid>,
    pub agent_instance_id: Option<uuid::Uuid>,
    pub team_instance_id: Option<uuid::Uuid>,
    pub task_id: Option<uuid::Uuid>,
    pub tool_name: Option<String>,
    pub action_type: String, // "tool_call", "delegation", "memory_write", "publish"
}

/// Multiple policy sources for dimensional evaluation.
/// Each source provides policy for specific dimensions.
#[derive(Debug, Clone, Default)]
pub struct PolicySources {
    pub system: Option<serde_json::Value>, // Global hard boundaries
    pub capability: Option<serde_json::Value>, // Capability-level rules
    pub agent: Option<serde_json::Value>,  // AgentDefinition policy
    pub team: Option<serde_json::Value>,   // Team collaboration policy
    pub selector: Option<serde_json::Value>, // Local binding constraints
    pub runtime: Option<serde_json::Value>, // Runtime signals
}

impl PolicySources {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_agent(mut self, policy: serde_json::Value) -> Self {
        self.agent = Some(policy);
        self
    }

    pub fn with_team(mut self, policy: serde_json::Value) -> Self {
        self.team = Some(policy);
        self
    }

    pub fn with_system(mut self, policy: serde_json::Value) -> Self {
        self.system = Some(policy);
        self
    }

    pub fn with_capability(mut self, policy: serde_json::Value) -> Self {
        self.capability = Some(policy);
        self
    }
}
