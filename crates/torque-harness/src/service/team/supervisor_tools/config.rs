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

use super::accept_result::AcceptResultTool;
use super::add_blocker::AddBlockerTool;
use super::complete_task::CompleteTeamTaskTool;
use super::delegate_task::DelegateTaskTool;
use super::delegation_status::GetDelegationStatusTool;
use super::fail_task::FailTeamTaskTool;
use super::get_shared_state::GetSharedStateTool;
use super::get_task_details::GetTaskDetailsTool;
use super::list_members::ListTeamMembersTool;
use super::publish_to_team::PublishToTeamTool;
use super::reject_result::RejectResultTool;
use super::request_approval::RequestApprovalTool;
use super::resolve_blocker::ResolveBlockerTool;
use super::update_fact::UpdateSharedFactTool;

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
        Arc::new(CompleteTeamTaskTool::new(config.team_task_repo.clone())) as ToolArc,
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
        Arc::new(FailTeamTaskTool::new(config.team_task_repo.clone())) as ToolArc,
        Arc::new(RequestApprovalTool::new()) as ToolArc,
        Arc::new(GetTaskDetailsTool::new(config.team_task_repo.clone())) as ToolArc,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::v1::delegation::{Delegation, DelegationStatus};
    use crate::repository::DelegationRepository;
    use async_trait::async_trait;
    use std::sync::Arc;
    use uuid::Uuid;

    struct MockDelegationRepository {
        pub delegation_id: Uuid,
        pub update_status_result: bool,
        pub update_status_id: std::sync::Mutex<Option<Uuid>>,
        pub update_status_status: std::sync::Mutex<Option<String>>,
    }

    impl MockDelegationRepository {
        fn new() -> Self {
            Self {
                delegation_id: Uuid::new_v4(),
                update_status_result: true,
                update_status_id: std::sync::Mutex::new(None),
                update_status_status: std::sync::Mutex::new(None),
            }
        }
    }

    #[async_trait]
    impl DelegationRepository for MockDelegationRepository {
        async fn create(
            &self,
            _task_id: Uuid,
            _parent_instance_id: Uuid,
            _selector: serde_json::Value,
        ) -> anyhow::Result<Delegation> {
            Ok(Delegation {
                id: self.delegation_id,
                task_id: Uuid::new_v4(),
                parent_agent_instance_id: Uuid::new_v4(),
                child_agent_definition_selector: serde_json::json!({}),
                status: DelegationStatus::Pending,
                result_artifact_id: None,
                error_message: None,
                rejection_reason: None,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            })
        }

        async fn list(&self, _limit: i64) -> anyhow::Result<Vec<Delegation>> {
            Ok(vec![])
        }

        async fn list_by_instance(
            &self,
            _instance_id: Uuid,
            _limit: i64,
        ) -> anyhow::Result<Vec<Delegation>> {
            Ok(vec![])
        }

        async fn list_by_task(
            &self,
            _task_id: Uuid,
            _limit: i64,
        ) -> anyhow::Result<Vec<Delegation>> {
            Ok(vec![])
        }

        async fn get(&self, _id: Uuid) -> anyhow::Result<Option<Delegation>> {
            Ok(Some(Delegation {
                id: self.delegation_id,
                task_id: Uuid::new_v4(),
                parent_agent_instance_id: Uuid::new_v4(),
                child_agent_definition_selector: serde_json::json!({}),
                status: DelegationStatus::Pending,
                result_artifact_id: None,
                error_message: None,
                rejection_reason: None,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            }))
        }

        async fn update_status(&self, id: Uuid, status: &str) -> anyhow::Result<bool> {
            *self.update_status_id.lock().unwrap() = Some(id);
            *self.update_status_status.lock().unwrap() = Some(status.to_string());
            Ok(self.update_status_result)
        }

        async fn complete(&self, _id: Uuid, _artifact_id: Uuid) -> anyhow::Result<bool> {
            Ok(true)
        }

        async fn fail(&self, _id: Uuid, _error: &str) -> anyhow::Result<bool> {
            Ok(true)
        }

        async fn reject(&self, _id: Uuid, _reason: &str) -> anyhow::Result<bool> {
            Ok(true)
        }

        async fn list_by_status(
            &self,
            _task_id: Uuid,
            _status: DelegationStatus,
        ) -> anyhow::Result<Vec<Delegation>> {
            Ok(vec![])
        }
    }

    fn create_mock_delegation(id: Uuid) -> Delegation {
        Delegation {
            id,
            task_id: Uuid::new_v4(),
            parent_agent_instance_id: Uuid::new_v4(),
            child_agent_definition_selector: serde_json::json!({}),
            status: DelegationStatus::Pending,
            result_artifact_id: None,
            error_message: None,
            rejection_reason: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_accept_result_tool_name() {
        let delegation_repo = Arc::new(MockDelegationRepository::new());
        let tool = AcceptResultTool::new(delegation_repo);
        assert_eq!(tool.name(), "accept_result");
    }

    #[tokio::test]
    async fn test_accept_result_tool_schema() {
        let delegation_repo = Arc::new(MockDelegationRepository::new());
        let tool = AcceptResultTool::new(delegation_repo);
        let schema = tool.parameters_schema();
        assert!(schema.pointer("/properties/delegation_id").is_some());
    }

    #[tokio::test]
    async fn test_accept_result_missing_delegation_id() {
        let delegation_repo = Arc::new(MockDelegationRepository::new());
        let tool = AcceptResultTool::new(delegation_repo);
        let result = tool.execute(serde_json::json!({})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_accept_result_success() {
        let delegation_repo = Arc::new(MockDelegationRepository::new());
        let tool = AcceptResultTool::new(delegation_repo);
        let delegation_id = Uuid::new_v4();

        let result = tool
            .execute(serde_json::json!({
                "delegation_id": delegation_id.to_string(),
                "summary": "Work completed"
            }))
            .await
            .unwrap();

        assert!(result.success);
    }

    #[tokio::test]
    async fn test_accept_result_updates_delegation_status() {
        let mock_repo = Arc::new(MockDelegationRepository::new());
        let tool = AcceptResultTool::new(mock_repo.clone());
        let delegation_id = Uuid::new_v4();

        tool.execute(serde_json::json!({
            "delegation_id": delegation_id.to_string(),
            "summary": "Done"
        }))
        .await
        .unwrap();

        let updated_id = mock_repo.update_status_id.lock().unwrap().unwrap();
        let updated_status = mock_repo
            .update_status_status
            .lock()
            .unwrap()
            .clone()
            .unwrap();
        assert_eq!(updated_id, delegation_id);
        assert_eq!(updated_status, "ACCEPTED");
    }

    #[tokio::test]
    async fn test_reject_result_tool_name() {
        let delegation_repo = Arc::new(MockDelegationRepository::new());
        let tool = RejectResultTool::new(delegation_repo);
        assert_eq!(tool.name(), "reject_result");
    }

    #[tokio::test]
    async fn test_reject_result_tool_schema() {
        let delegation_repo = Arc::new(MockDelegationRepository::new());
        let tool = RejectResultTool::new(delegation_repo);
        let schema = tool.parameters_schema();
        assert!(schema.pointer("/properties/delegation_id").is_some());
        assert!(schema.pointer("/properties/reason").is_some());
    }

    #[tokio::test]
    async fn test_reject_result_missing_delegation_id() {
        let delegation_repo = Arc::new(MockDelegationRepository::new());
        let tool = RejectResultTool::new(delegation_repo);
        let result = tool
            .execute(serde_json::json!({
                "reason": "Not satisfied"
            }))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_reject_result_success() {
        let delegation_repo = Arc::new(MockDelegationRepository::new());
        let tool = RejectResultTool::new(delegation_repo);
        let delegation_id = Uuid::new_v4();

        let result = tool
            .execute(serde_json::json!({
                "delegation_id": delegation_id.to_string(),
                "reason": "Not satisfied"
            }))
            .await
            .unwrap();

        assert!(result.success);
    }

    #[tokio::test]
    async fn test_get_delegation_status_tool_name() {
        let delegation_repo = Arc::new(MockDelegationRepository::new());
        let tool = GetDelegationStatusTool::new(delegation_repo);
        assert_eq!(tool.name(), "get_delegation_status");
    }

    #[tokio::test]
    async fn test_get_delegation_status_tool_schema() {
        let delegation_repo = Arc::new(MockDelegationRepository::new());
        let tool = GetDelegationStatusTool::new(delegation_repo);
        let schema = tool.parameters_schema();
        assert!(schema.pointer("/properties/delegation_id").is_some());
    }

    #[tokio::test]
    async fn test_get_delegation_status_missing_delegation_id() {
        let delegation_repo = Arc::new(MockDelegationRepository::new());
        let tool = GetDelegationStatusTool::new(delegation_repo);
        let result = tool.execute(serde_json::json!({})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_delegation_status_success() {
        let delegation_repo = Arc::new(MockDelegationRepository::new());
        let tool = GetDelegationStatusTool::new(delegation_repo);
        let delegation_id = Uuid::new_v4();

        let result = tool
            .execute(serde_json::json!({
                "delegation_id": delegation_id.to_string()
            }))
            .await
            .unwrap();

        assert!(result.success);
    }
}
