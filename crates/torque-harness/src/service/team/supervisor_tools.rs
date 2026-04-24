use crate::models::v1::team::{MemberSelector, PublishScope, TeamTaskStatus};
use crate::repository::{
    DelegationRepository, TeamMemberRepository, TeamTaskRepository,
};
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

        let selector: MemberSelector =
            serde_json::from_value(member_selector.clone()).map_err(|e| {
                anyhow::anyhow!("Invalid member_selector format: {}", e)
            })?;

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
        let delegation = self
            .delegation_repo
            .create(task_id, selected_member.agent_instance_id, member_selector.clone())
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

pub struct AcceptResultTool {
    delegation_repo: Arc<dyn DelegationRepository>,
}

impl AcceptResultTool {
    pub fn new(delegation_repo: Arc<dyn DelegationRepository>) -> Self {
        Self { delegation_repo }
    }
}

#[async_trait]
impl Tool for AcceptResultTool {
    fn name(&self) -> &str {
        "accept_result"
    }

    fn description(&self) -> &str {
        "Accept a member's delegation result"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "delegation_id": {
                    "type": "string",
                    "description": "The delegation ID to accept"
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

        let updated = self
            .delegation_repo
            .update_status(delegation_id, "ACCEPTED")
            .await?;

        if updated {
            Ok(ToolResult {
                success: true,
                content: format!("Accepted delegation: {}", delegation_id),
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
                content: format!("Rejected delegation {}: reroute not implemented", delegation_id),
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
                content: format!("Published artifact {} to {} scope: {}", artifact_id, scope_str, summary),
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

pub struct CompleteTeamTaskTool {
    task_repo: Arc<dyn TeamTaskRepository>,
}

impl CompleteTeamTaskTool {
    pub fn new(task_repo: Arc<dyn TeamTaskRepository>) -> Self {
        Self { task_repo }
    }
}

#[async_trait]
impl Tool for CompleteTeamTaskTool {
    fn name(&self) -> &str {
        "complete_team_task"
    }

    fn description(&self) -> &str {
        "Mark a team task as complete"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "The task ID to complete"
                },
                "summary": {
                    "type": "string",
                    "description": "Summary of the completed task"
                },
                "output_artifacts": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "List of output artifact references"
                }
            },
            "required": ["task_id", "summary"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let task_id_str = args
            .get("task_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("task_id required"))?;
        let task_id = Uuid::parse_str(task_id_str)
            .map_err(|e| anyhow::anyhow!("Invalid task_id format: {}", e))?;
        let summary = args
            .get("summary")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("summary required"))?;

        let updated = self
            .task_repo
            .update_status(task_id, TeamTaskStatus::Completed)
            .await?;

        if updated {
            self.task_repo.mark_completed(task_id).await?;
            Ok(ToolResult {
                success: true,
                content: format!("Task {} completed: {}", task_id, summary),
                error: None,
            })
        } else {
            Ok(ToolResult {
                success: false,
                content: String::new(),
                error: Some(format!("Task {} not found", task_id)),
            })
        }
    }
}

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
        let members_json =
            serde_json::to_string(&members).map_err(|e| anyhow::anyhow!("Failed to serialize team members: {}", e))?;
        Ok(ToolResult {
            success: true,
            content: members_json,
            error: None,
        })
    }
}

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
        let source = args.get("source").and_then(|v| v.as_str()).unwrap_or("supervisor");

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

pub struct FailTeamTaskTool {
    task_repo: Arc<dyn TeamTaskRepository>,
}

impl FailTeamTaskTool {
    pub fn new(task_repo: Arc<dyn TeamTaskRepository>) -> Self {
        Self { task_repo }
    }
}

#[async_trait]
impl Tool for FailTeamTaskTool {
    fn name(&self) -> &str {
        "fail_team_task"
    }

    fn description(&self) -> &str {
        "Mark a team task as failed"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "The task ID to fail"
                },
                "reason": {
                    "type": "string",
                    "description": "Reason for failure"
                }
            },
            "required": ["task_id", "reason"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let task_id_str = args
            .get("task_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("task_id required"))?;
        let task_id = Uuid::parse_str(task_id_str)
            .map_err(|e| anyhow::anyhow!("Invalid task_id format: {}", e))?;
        let reason = args
            .get("reason")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("reason required"))?;

        let updated = self
            .task_repo
            .update_status(task_id, TeamTaskStatus::Failed)
            .await?;

        if updated {
            Ok(ToolResult {
                success: true,
                content: format!("Task {} failed: {}", task_id, reason),
                error: None,
            })
        } else {
            Ok(ToolResult {
                success: false,
                content: String::new(),
                error: Some(format!("Task {} not found", task_id)),
            })
        }
    }
}

pub struct RequestApprovalTool;

impl RequestApprovalTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RequestApprovalTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for RequestApprovalTool {
    fn name(&self) -> &str {
        "request_approval"
    }

    fn description(&self) -> &str {
        "Request team-level approval"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "tool_name": {
                    "type": "string",
                    "description": "Name of the tool requiring approval"
                },
                "reason": {
                    "type": "string",
                    "description": "Reason for the approval request"
                }
            },
            "required": ["tool_name", "reason"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let tool_name = args
            .get("tool_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("tool_name required"))?;
        let reason = args
            .get("reason")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("reason required"))?;

        Ok(ToolResult {
            success: true,
            content: format!("Requested approval for {}: {}", tool_name, reason),
            error: None,
        })
    }
}

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
                let task_json =
                    serde_json::to_string(&t).map_err(|e| anyhow::anyhow!("Failed to serialize task: {}", e))?;
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

pub struct SupervisorToolsConfig {
    pub delegation_repo: Arc<dyn DelegationRepository>,
    pub selector_resolver: Arc<SelectorResolver>,
    pub shared_state: Arc<SharedTaskStateManager>,
    pub team_member_repo: Arc<dyn TeamMemberRepository>,
    pub team_task_repo: Arc<dyn TeamTaskRepository>,
    pub team_instance_id: Uuid,
}

pub fn create_supervisor_tools(config: SupervisorToolsConfig) -> Vec<ToolArc> {
    vec![
        Arc::new(DelegateTaskTool::new(
            config.delegation_repo.clone(),
            config.selector_resolver.clone(),
            config.team_instance_id,
        )) as ToolArc,
        Arc::new(AcceptResultTool::new(config.delegation_repo.clone())) as ToolArc,
        Arc::new(RejectResultTool::new(config.delegation_repo.clone())) as ToolArc,
        Arc::new(PublishToTeamTool::new(
            config.shared_state.clone(),
            config.team_instance_id,
        )) as ToolArc,
        Arc::new(GetSharedStateTool::new(
            config.shared_state.clone(),
            config.team_instance_id,
        )) as ToolArc,
        Arc::new(CompleteTeamTaskTool::new(
            config.team_task_repo.clone(),
        )) as ToolArc,
        Arc::new(ListTeamMembersTool::new(
            config.team_member_repo.clone(),
            config.team_instance_id,
        )) as ToolArc,
        Arc::new(GetDelegationStatusTool::new(config.delegation_repo.clone())) as ToolArc,
        Arc::new(UpdateSharedFactTool::new(
            config.shared_state.clone(),
            config.team_instance_id,
        )) as ToolArc,
        Arc::new(AddBlockerTool::new(
            config.shared_state.clone(),
            config.team_instance_id,
        )) as ToolArc,
        Arc::new(ResolveBlockerTool::new(
            config.shared_state.clone(),
            config.team_instance_id,
        )) as ToolArc,
        Arc::new(FailTeamTaskTool::new(
            config.team_task_repo.clone(),
        )) as ToolArc,
        Arc::new(RequestApprovalTool::new()) as ToolArc,
        Arc::new(GetTaskDetailsTool::new(config.team_task_repo.clone())) as ToolArc,
    ]
}