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

pub struct CompleteTeamTaskTool {
    task_repo: Arc<dyn TeamTaskRepository>,
}

impl CompleteTeamTaskTool {
    pub fn new(task_repo: Arc<dyn TeamTaskRepository>) -> Self {
        Self { task_repo }
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
                "task_id": {
                    "type": "string",
                    "description": "The task ID to complete"
                },
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
            "required": ["task_id", "summary"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let task_id_str = args
            .get("task_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("task_id required"))?;
        let task_id = Uuid::parse_str(task_id_str)
            .map_err(|e| anyhow::anyhow!("Invalid task_id format: {}", e))?;
        let summary = args
            .get("summary")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("summary required"))?;

        let updated = self
            .task_repo
            .update_status(task_id, TeamTaskStatus::Completed)
            .await?;

        if updated {
            self.task_repo.mark_completed(task_id).await?;
            Ok(ToolResult {
                success: true,
                content: format!("Task {} completed: {}", task_id, summary),
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
