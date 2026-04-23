use crate::tools::{Tool, ToolArc, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;

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

pub struct PublishToTeamTool;

impl PublishToTeamTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PublishToTeamTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for PublishToTeamTool {
    fn name(&self) -> &str {
        "publish_to_team"
    }

    fn description(&self) -> &str {
        "Publish an artifact to team shared state"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "artifact_ref": {
                    "type": "string",
                    "description": "Reference to the artifact"
                },
                "summary": {
                    "type": "string",
                    "description": "Summary of the artifact"
                },
                "scope": {
                    "type": "string",
                    "enum": ["private", "team_shared", "external_published"],
                    "description": "Visibility scope"
                }
            },
            "required": ["artifact_ref", "summary", "scope"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let artifact_ref = args
            .get("artifact_ref")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("artifact_ref required"))?;
        let summary = args
            .get("summary")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("summary required"))?;
        let scope = args
            .get("scope")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("scope required"))?;

        Ok(ToolResult {
            success: true,
            content: format!("Published artifact {} to {} scope: {}", artifact_ref, scope, summary),
            error: None,
        })
    }
}

pub struct GetSharedStateTool;

impl GetSharedStateTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GetSharedStateTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for GetSharedStateTool {
    fn name(&self) -> &str {
        "get_shared_state"
    }

    fn description(&self) -> &str {
        "Get the current team shared task state"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(&self, _args: Value) -> anyhow::Result<ToolResult> {
        Ok(ToolResult {
            success: true,
            content: r#"{"accepted_artifact_refs":[],"published_facts":[],"delegation_status":[],"open_blockers":[],"decisions":[]}"#.to_string(),
            error: None,
        })
    }
}

pub struct CompleteTeamTaskTool;

impl CompleteTeamTaskTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CompleteTeamTaskTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for CompleteTeamTaskTool {
    fn name(&self) -> &str {
        "complete_team_task"
    }

    fn description(&self) -> &str {
        "Mark a team task as complete"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "summary": {
                    "type": "string",
                    "description": "Summary of the completed task"
                },
                "output_artifacts": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "List of output artifact references"
                }
            },
            "required": ["summary"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let summary = args
            .get("summary")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("summary required"))?;

        Ok(ToolResult {
            success: true,
            content: format!("Task completed: {}", summary),
            error: None,
        })
    }
}

pub struct ListTeamMembersTool;

impl ListTeamMembersTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ListTeamMembersTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ListTeamMembersTool {
    fn name(&self) -> &str {
        "list_team_members"
    }

    fn description(&self) -> &str {
        "List available team members"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(&self, _args: Value) -> anyhow::Result<ToolResult> {
        Ok(ToolResult {
            success: true,
            content: "[]".to_string(),
            error: None,
        })
    }
}

pub fn create_supervisor_tools() -> Vec<ToolArc> {
    vec![
        Arc::new(DelegateTaskTool::new()) as ToolArc,
        Arc::new(AcceptResultTool::new()) as ToolArc,
        Arc::new(RejectResultTool::new()) as ToolArc,
        Arc::new(PublishToTeamTool::new()) as ToolArc,
        Arc::new(GetSharedStateTool::new()) as ToolArc,
        Arc::new(CompleteTeamTaskTool::new()) as ToolArc,
        Arc::new(ListTeamMembersTool::new()) as ToolArc,
    ]
}
