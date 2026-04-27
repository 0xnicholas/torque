use crate::repository::{TeamMemberRepository, TeamTaskRepository};
use crate::tools::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

pub struct ListTeamMembersTool {
    team_member_repo: Arc<dyn TeamMemberRepository>,
    team_instance_id: Uuid,
}

impl ListTeamMembersTool {
    pub fn new(team_member_repo: Arc<dyn TeamMemberRepository>, team_instance_id: Uuid) -> Self {
        Self {
            team_member_repo,
            team_instance_id,
        }
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
        let members = self
            .team_member_repo
            .list_by_team(self.team_instance_id, 100)
            .await?;
        let members_json = serde_json::to_string(&members)
            .map_err(|e| anyhow::anyhow!("Failed to serialize team members: {}", e))?;
        Ok(ToolResult {
            success: true,
            content: members_json,
            error: None,
        })
    }
}
