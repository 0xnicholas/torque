use crate::repository::{DelegationRepository, TeamTaskRepository};
use crate::tools::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct RequestApprovalTool;

impl RequestApprovalTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RequestApprovalTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for RequestApprovalTool {
    fn name(&self) -> &str {
        "request_approval"
    }

    fn description(&self) -> &str {
        "Request team-level approval"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "tool_name": {
                    "type": "string",
                    "description": "Name of the tool requiring approval"
                },
                "reason": {
                    "type": "string",
                    "description": "Reason for the approval request"
                }
            },
            "required": ["tool_name", "reason"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let tool_name = args
            .get("tool_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("tool_name required"))?;
        let reason = args
            .get("reason")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("reason required"))?;

        Ok(ToolResult {
            success: true,
            content: format!("Requested approval for {}: {}", tool_name, reason),
            error: None,
        })
    }
}
