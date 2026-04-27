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

pub struct GetTaskDetailsTool {
    task_repo: Arc<dyn TeamTaskRepository>,
}

impl GetTaskDetailsTool {
    pub fn new(task_repo: Arc<dyn TeamTaskRepository>) -> Self {
        Self { task_repo }
    }
}

#[async_trait]
impl Tool for GetTaskDetailsTool {
    fn name(&self) -> &str {
        "get_task_details"
    }

    fn description(&self) -> &str {
        "Get details of a team task"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "The task ID to get details for"
                }
            },
            "required": ["task_id"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let task_id_str = args
            .get("task_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("task_id required"))?;
        let task_id = Uuid::parse_str(task_id_str)
            .map_err(|e| anyhow::anyhow!("Invalid task_id format: {}", e))?;

        let task = self.task_repo.get(task_id).await?;

        match task {
            Some(t) => {
                let task_json = serde_json::to_string(&t)
                    .map_err(|e| anyhow::anyhow!("Failed to serialize task: {}", e))?;
                Ok(ToolResult {
                    success: true,
                    content: task_json,
                    error: None,
                })
            }
            None => Ok(ToolResult {
                success: false,
                content: String::new(),
                error: Some(format!("Task {} not found", task_id)),
            }),
        }
    }
}
