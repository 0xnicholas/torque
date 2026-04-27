use crate::models::v1::team::{MemberSelector, PublishScope, TeamTaskStatus};
use crate::repository::{DelegationRepository, TeamMemberRepository, TeamTaskRepository};
use crate::service::build_delegation_packet;
use crate::service::team::selector::SelectorResolver;
use crate::service::team::shared_state::SharedTaskStateManager;
use crate::tools::{Tool, ToolArc, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

pub struct FailTeamTaskTool {
    task_repo: Arc<dyn TeamTaskRepository>,
}

impl FailTeamTaskTool {
    pub fn new(task_repo: Arc<dyn TeamTaskRepository>) -> Self {
        Self { task_repo }
    }
}

#[async_trait]
impl Tool for FailTeamTaskTool {
    fn name(&self) -> &str {
        "fail_team_task"
    }

    fn description(&self) -> &str {
        "Mark a team task as failed"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "The task ID to fail"
                },
                "reason": {
                    "type": "string",
                    "description": "Reason for failure"
                }
            },
            "required": ["task_id", "reason"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let task_id_str = args
            .get("task_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("task_id required"))?;
        let task_id = Uuid::parse_str(task_id_str)
            .map_err(|e| anyhow::anyhow!("Invalid task_id format: {}", e))?;
        let reason = args
            .get("reason")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("reason required"))?;

        let updated = self
            .task_repo
            .update_status(task_id, TeamTaskStatus::Failed)
            .await?;

        if updated {
            Ok(ToolResult {
                success: true,
                content: format!("Task {} failed: {}", task_id, reason),
                error: None,
            })
        } else {
            Ok(ToolResult {
                success: false,
                content: String::new(),
                error: Some(format!("Task {} not found", task_id)),
            })
        }
    }
}
