use super::{PolicyDecision, PolicyInput};

pub struct PolicyEvaluator;

impl PolicyEvaluator {
    pub fn new() -> Self {
        Self
    }

    pub fn evaluate(
        &self,
        input: &PolicyInput,
        agent_tool_policy: &serde_json::Value,
    ) -> PolicyDecision {
        let mut decision = PolicyDecision::default();

        // Evaluate tool policy dimension
        if input.action_type == "tool_call" {
            if let Some(tool_name) = &input.tool_name {
                let tool_decision = self.evaluate_tool_policy(tool_name, agent_tool_policy);
                decision = decision.merge(tool_decision);
            }
        }

        decision
    }

    fn evaluate_tool_policy(&self, tool_name: &str, policy: &serde_json::Value) -> PolicyDecision {
        // Default: allow
        if policy.is_null() || policy == &serde_json::json!({}) {
            return PolicyDecision::default();
        }

        // Check forbidden tools
        if let Some(forbidden) = policy.get("forbidden_tools").and_then(|v| v.as_array()) {
            for forbidden_tool in forbidden {
                if let Some(name) = forbidden_tool.as_str() {
                    if name == tool_name || name == "*" {
                        return PolicyDecision::deny(format!(
                            "Tool '{}' is forbidden by policy",
                            tool_name
                        ));
                    }
                }
            }
        }

        // Check require_approval tools
        if let Some(require_approval) = policy
            .get("require_approval_tools")
            .and_then(|v| v.as_array())
        {
            for approval_tool in require_approval {
                if let Some(name) = approval_tool.as_str() {
                    if name == tool_name || name == "*" {
                        return PolicyDecision::require_approval(
                            "tool",
                            format!("Tool '{}' requires approval", tool_name),
                        );
                    }
                }
            }
        }

        // Check allowed tools (if whitelist exists, tool must be in it)
        if let Some(allowed) = policy.get("allowed_tools").and_then(|v| v.as_array()) {
            let mut is_allowed = false;
            for allowed_tool in allowed {
                if let Some(name) = allowed_tool.as_str() {
                    if name == tool_name || name == "*" {
                        is_allowed = true;
                        break;
                    }
                }
            }
            if !is_allowed {
                return PolicyDecision::deny(format!(
                    "Tool '{}' is not in allowed tools list",
                    tool_name
                ));
            }
        }

        PolicyDecision::default()
    }
}

impl Default for PolicyEvaluator {
    fn default() -> Self {
        Self::new()
    }
}
