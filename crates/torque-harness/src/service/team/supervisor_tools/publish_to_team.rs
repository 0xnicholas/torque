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

pub struct PublishToTeamTool {
    shared_state: Arc<SharedTaskStateManager>,
    team_instance_id: Uuid,
}

impl PublishToTeamTool {
    pub fn new(shared_state: Arc<SharedTaskStateManager>, team_instance_id: Uuid) -> Self {
        Self {
            shared_state,
            team_instance_id,
        }
    }
}

#[async_trait]
impl Tool for PublishToTeamTool {
    fn name(&self) -> &str {
        "publish_to_team"
    }

    fn description(&self) -> &str {
        "Publish an artifact to team shared state"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "artifact_id": {
                    "type": "string",
                    "description": "The artifact ID to publish"
                },
                "summary": {
                    "type": "string",
                    "description": "Summary of the artifact"
                },
                "scope": {
                    "type": "string",
                    "enum": ["private", "team_shared", "external_published"],
                    "description": "Visibility scope"
                }
            },
            "required": ["artifact_id", "summary", "scope"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let artifact_id_str = args
            .get("artifact_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("artifact_id required"))?;
        let artifact_id = Uuid::parse_str(artifact_id_str)
            .map_err(|e| anyhow::anyhow!("Invalid artifact_id format: {}", e))?;
        let summary = args
            .get("summary")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("summary required"))?;
        let scope_str = args
            .get("scope")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("scope required"))?;

        let scope = match scope_str {
            "private" => PublishScope::Private,
            "team_shared" => PublishScope::TeamShared,
            "external_published" => PublishScope::ExternalPublished,
            _ => {
                return Ok(ToolResult {
                    success: false,
                    content: String::new(),
                    error: Some(format!("Invalid scope: {}", scope_str)),
                });
            }
        };

        let published = self
            .shared_state
            .publish_artifact(self.team_instance_id, artifact_id, scope, "supervisor")
            .await?;

        if published {
            Ok(ToolResult {
                success: true,
                content: format!(
                    "Published artifact {} to {} scope: {}",
                    artifact_id, scope_str, summary
                ),
                error: None,
            })
        } else {
            Ok(ToolResult {
                success: false,
                content: String::new(),
                error: Some("Failed to publish artifact".to_string()),
            })
        }
    }
}
