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

pub struct GetDelegationStatusTool {
    delegation_repo: Arc<dyn DelegationRepository>,
}

impl GetDelegationStatusTool {
    pub fn new(delegation_repo: Arc<dyn DelegationRepository>) -> Self {
        Self { delegation_repo }
    }
}

#[async_trait]
impl Tool for GetDelegationStatusTool {
    fn name(&self) -> &str {
        "get_delegation_status"
    }

    fn description(&self) -> &str {
        "Get the current status of a delegation"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "delegation_id": {
                    "type": "string",
                    "description": "The delegation ID to check"
                }
            },
            "required": ["delegation_id"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let delegation_id_str = args
            .get("delegation_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("delegation_id required"))?;
        let delegation_id = Uuid::parse_str(delegation_id_str)
            .map_err(|e| anyhow::anyhow!("Invalid delegation_id format: {}", e))?;

        let delegation = self.delegation_repo.get(delegation_id).await?;

        match delegation {
            Some(d) => {
                let delegation_json = serde_json::to_string(&d)
                    .map_err(|e| anyhow::anyhow!("Failed to serialize delegation: {}", e))?;
                Ok(ToolResult {
                    success: true,
                    content: delegation_json,
                    error: None,
                })
            }
            None => Ok(ToolResult {
                success: false,
                content: String::new(),
                error: Some(format!("Delegation {} not found", delegation_id)),
            }),
        }
    }
}
