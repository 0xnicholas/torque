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

pub struct DelegateTaskTool {
    delegation_repo: Arc<dyn DelegationRepository>,
    selector_resolver: Arc<SelectorResolver>,
    team_instance_id: Uuid,
}

impl DelegateTaskTool {
    pub fn new(
        delegation_repo: Arc<dyn DelegationRepository>,
        selector_resolver: Arc<SelectorResolver>,
        team_instance_id: Uuid,
    ) -> Self {
        Self {
            delegation_repo,
            selector_resolver,
            team_instance_id,
        }
    }
}

#[async_trait]
impl Tool for DelegateTaskTool {
    fn name(&self) -> &str {
        "delegate_task"
    }

    fn description(&self) -> &str {
        "Delegate a task to a team member"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "member_selector": {
                    "type": "object",
                    "description": "Selector to identify the team member"
                },
                "goal": {
                    "type": "string",
                    "description": "Goal for the delegated task"
                },
                "instructions": {
                    "type": "string",
                    "description": "Detailed instructions for the task"
                },
                "return_contract": {
                    "type": "object",
                    "description": "Contract for expected return value"
                }
            },
            "required": ["member_selector", "goal"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let member_selector = args
            .get("member_selector")
            .ok_or_else(|| anyhow::anyhow!("member_selector required"))?;
        let goal = args
            .get("goal")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("goal required"))?;
        let _instructions = args.get("instructions").and_then(|v| v.as_str());

        let selector: MemberSelector = serde_json::from_value(member_selector.clone())
            .map_err(|e| anyhow::anyhow!("Invalid member_selector format: {}", e))?;

        let candidates = self
            .selector_resolver
            .resolve(&selector, self.team_instance_id)
            .await?;

        if candidates.is_empty() {
            return Ok(ToolResult {
                success: false,
                content: String::new(),
                error: Some("No matching team members found for selector".to_string()),
            });
        }

        let selected_member = &candidates[0];

        let task_id = Uuid::new_v4();
        let delegation_packet = build_delegation_packet(
            goal,
            _instructions,
            vec![],
            vec![],
            vec![],
            None,
            vec![],
        );

        let delegation = self
            .delegation_repo
            .create(
                task_id,
                selected_member.agent_instance_id,
                delegation_packet,
            )
            .await?;

        Ok(ToolResult {
            success: true,
            content: format!(
                "Delegated task {} to member with goal: {}. Delegation ID: {}",
                task_id, goal, delegation.id
            ),
            error: None,
        })
    }
}
