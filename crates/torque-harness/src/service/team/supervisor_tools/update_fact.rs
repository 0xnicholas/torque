use crate::repository::{DelegationRepository, TeamTaskRepository};
use crate::service::team::shared_state::SharedTaskStateManager;
use crate::tools::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

pub struct UpdateSharedFactTool {
    shared_state: Arc<SharedTaskStateManager>,
    team_instance_id: Uuid,
}

impl UpdateSharedFactTool {
    pub fn new(shared_state: Arc<SharedTaskStateManager>, team_instance_id: Uuid) -> Self {
        Self {
            shared_state,
            team_instance_id,
        }
    }
}

#[async_trait]
impl Tool for UpdateSharedFactTool {
    fn name(&self) -> &str {
        "update_shared_fact"
    }

    fn description(&self) -> &str {
        "Update a coordination fact in shared state"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "key": {
                    "type": "string",
                    "description": "The fact key to update"
                },
                "value": {
                    "type": "string",
                    "description": "The fact value"
                }
            },
            "required": ["key", "value"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let key = args
            .get("key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("key required"))?;
        let value = args
            .get("value")
            .ok_or_else(|| anyhow::anyhow!("value required"))?;

        let published = self
            .shared_state
            .publish_fact(self.team_instance_id, key, value.clone(), "supervisor")
            .await?;

        if published {
            Ok(ToolResult {
                success: true,
                content: format!("Updated shared fact: {} = {}", key, value),
                error: None,
            })
        } else {
            Ok(ToolResult {
                success: false,
                content: String::new(),
                error: Some("Failed to update shared fact".to_string()),
            })
        }
    }
}
