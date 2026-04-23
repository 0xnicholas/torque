use crate::tools::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct DelegateTaskTool;

impl DelegateTaskTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DelegateTaskTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for DelegateTaskTool {
    fn name(&self) -> &str {
        "delegate_task"
    }

    fn description(&self) -> &str {
        "Delegate a task to a team member"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "member_selector": {
                    "type": "object",
                    "description": "Selector to identify the team member"
                },
                "goal": {
                    "type": "string",
                    "description": "Goal for the delegated task"
                },
                "instructions": {
                    "type": "string",
                    "description": "Detailed instructions for the task"
                },
                "return_contract": {
                    "type": "object",
                    "description": "Contract for expected return value"
                }
            },
            "required": ["member_selector", "goal"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let _member_selector = args
            .get("member_selector")
            .ok_or_else(|| anyhow::anyhow!("member_selector required"))?;
        let goal = args
            .get("goal")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("goal required"))?;
        let _instructions = args.get("instructions").and_then(|v| v.as_str());

        Ok(ToolResult {
            success: true,
            content: format!("Delegated task to member with goal: {}", goal),
            error: None,
        })
    }
}

pub struct AcceptResultTool;

impl AcceptResultTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AcceptResultTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for AcceptResultTool {
    fn name(&self) -> &str {
        "accept_result"
    }

    fn description(&self) -> &str {
        "Accept a member's delegation result"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "delegation_id": {
                    "type": "string",
                    "description": "The delegation ID to accept"
                }
            },
            "required": ["delegation_id"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let delegation_id = args
            .get("delegation_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("delegation_id required"))?;

        Ok(ToolResult {
            success: true,
            content: format!("Accepted delegation: {}", delegation_id),
            error: None,
        })
    }
}

pub struct RejectResultTool;

impl RejectResultTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RejectResultTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for RejectResultTool {
    fn name(&self) -> &str {
        "reject_result"
    }

    fn description(&self) -> &str {
        "Reject a member's delegation result, optionally rerouting"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "delegation_id": {
                    "type": "string",
                    "description": "The delegation ID to reject"
                },
                "reason": {
                    "type": "string",
                    "description": "Reason for rejection"
                },
                "reroute": {
                    "type": "boolean",
                    "description": "Whether to reroute to another member"
                }
            },
            "required": ["delegation_id", "reason"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let delegation_id = args
            .get("delegation_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("delegation_id required"))?;
        let reason = args
            .get("reason")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("reason required"))?;

        Ok(ToolResult {
            success: true,
            content: format!("Rejected delegation {}: {}", delegation_id, reason),
            error: None,
        })
    }
}
