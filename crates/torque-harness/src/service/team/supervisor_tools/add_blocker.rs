use crate::repository::{DelegationRepository, TeamTaskRepository};
use crate::service::team::shared_state::SharedTaskStateManager;
use crate::tools::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

pub struct AddBlockerTool {
    shared_state: Arc<SharedTaskStateManager>,
    team_instance_id: Uuid,
}

impl AddBlockerTool {
    pub fn new(shared_state: Arc<SharedTaskStateManager>, team_instance_id: Uuid) -> Self {
        Self {
            shared_state,
            team_instance_id,
        }
    }
}

#[async_trait]
impl Tool for AddBlockerTool {
    fn name(&self) -> &str {
        "add_blocker"
    }

    fn description(&self) -> &str {
        "Add a blocker to shared state"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "description": {
                    "type": "string",
                    "description": "Description of the blocker"
                },
                "source": {
                    "type": "string",
                    "description": "Source of the blocker"
                }
            },
            "required": ["description"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let description = args
            .get("description")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("description required"))?;
        let source = args
            .get("source")
            .and_then(|v| v.as_str())
            .unwrap_or("supervisor");

        let added = self
            .shared_state
            .add_blocker(self.team_instance_id, description, source)
            .await?;

        if added {
            Ok(ToolResult {
                success: true,
                content: format!("Added blocker: {} (source: {})", description, source),
                error: None,
            })
        } else {
            Ok(ToolResult {
                success: false,
                content: String::new(),
                error: Some("Failed to add blocker".to_string()),
            })
        }
    }
}
