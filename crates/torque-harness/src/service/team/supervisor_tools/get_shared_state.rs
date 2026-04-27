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

pub struct GetSharedStateTool {
    shared_state: Arc<SharedTaskStateManager>,
    team_instance_id: Uuid,
}

impl GetSharedStateTool {
    pub fn new(shared_state: Arc<SharedTaskStateManager>, team_instance_id: Uuid) -> Self {
        Self {
            shared_state,
            team_instance_id,
        }
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
        let state = self
            .shared_state
            .get_or_create(self.team_instance_id)
            .await?;
        let state_json = serde_json::to_string(&state)
            .map_err(|e| anyhow::anyhow!("Failed to serialize shared state: {}", e))?;
        Ok(ToolResult {
            success: true,
            content: state_json,
            error: None,
        })
    }
}
