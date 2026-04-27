use crate::repository::{DelegationRepository, TeamTaskRepository};
use crate::service::team::shared_state::SharedTaskStateManager;
use crate::tools::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

pub struct ResolveBlockerTool {
    shared_state: Arc<SharedTaskStateManager>,
    team_instance_id: Uuid,
}

impl ResolveBlockerTool {
    pub fn new(shared_state: Arc<SharedTaskStateManager>, team_instance_id: Uuid) -> Self {
        Self {
            shared_state,
            team_instance_id,
        }
    }
}

#[async_trait]
impl Tool for ResolveBlockerTool {
    fn name(&self) -> &str {
        "resolve_blocker"
    }

    fn description(&self) -> &str {
        "Resolve a blocker in shared state"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "blocker_id": {
                    "type": "string",
                    "description": "The blocker ID to resolve"
                }
            },
            "required": ["blocker_id"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let blocker_id_str = args
            .get("blocker_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("blocker_id required"))?;
        let blocker_id = Uuid::parse_str(blocker_id_str)
            .map_err(|e| anyhow::anyhow!("Invalid blocker_id format: {}", e))?;

        let resolved = self
            .shared_state
            .resolve_blocker(self.team_instance_id, blocker_id)
            .await?;

        if resolved {
            Ok(ToolResult {
                success: true,
                content: format!("Resolved blocker: {}", blocker_id),
                error: None,
            })
        } else {
            Ok(ToolResult {
                success: false,
                content: String::new(),
                error: Some("Failed to resolve blocker".to_string()),
            })
        }
    }
}
