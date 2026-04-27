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

pub struct RejectResultTool {
    delegation_repo: Arc<dyn DelegationRepository>,
}

impl RejectResultTool {
    pub fn new(delegation_repo: Arc<dyn DelegationRepository>) -> Self {
        Self { delegation_repo }
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
        let delegation_id_str = args
            .get("delegation_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("delegation_id required"))?;
        let delegation_id = Uuid::parse_str(delegation_id_str)
            .map_err(|e| anyhow::anyhow!("Invalid delegation_id format: {}", e))?;
        let reason = args
            .get("reason")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("reason required"))?;

        let updated = self.delegation_repo.reject(delegation_id, reason).await?;

        if updated {
            Ok(ToolResult {
                success: true,
                content: format!(
                    "Rejected delegation {}: reroute not implemented",
                    delegation_id
                ),
                error: None,
            })
        } else {
            Ok(ToolResult {
                success: false,
                content: String::new(),
                error: Some(format!("Delegation {} not found", delegation_id)),
            })
        }
    }
}
