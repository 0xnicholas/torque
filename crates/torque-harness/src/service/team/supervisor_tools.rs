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
                    "type": "string",
                    "description": "Selector to identify the team member"
                },
                "goal": {
                    "type": "string",
                    "description": "Goal for the delegated task"
                },
                "instructions": {
                    "type": "string",
                    "description": "Detailed instructions for the task"
                }
            },
            "required": ["member_selector", "goal", "instructions"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let member_selector = args.get("member_selector")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let goal = args.get("goal")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let _instructions = args.get("instructions")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        Ok(ToolResult {
            success: true,
            content: format!("Delegated task to {} with goal: {}", member_selector, goal),
            error: None,
        })
    }
}