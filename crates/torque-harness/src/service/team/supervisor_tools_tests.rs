use crate::service::team::supervisor_tools::*;
use crate::tools::Tool;
use serde_json::json;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_delegate_task_tool_name() {
        let tool = DelegateTaskTool::new();
        assert_eq!(tool.name(), "delegate_task");
    }

    #[tokio::test]
    async fn test_delegate_task_tool_description() {
        let tool = DelegateTaskTool::new();
        assert_eq!(tool.description(), "Delegate a task to a team member");
    }

    #[tokio::test]
    async fn test_delegate_task_success() {
        let tool = DelegateTaskTool::new();
        let result = tool
            .execute(json!({
                "member_selector": {"selector_type": "any"},
                "goal": "test goal"
            }))
            .await;
        assert!(result.is_ok());
        let tool_result = result.unwrap();
        assert!(tool_result.success);
        assert!(tool_result.content.contains("Delegated task to member"));
    }

    #[tokio::test]
    async fn test_delegate_task_missing_goal() {
        let tool = DelegateTaskTool::new();
        let result = tool
            .execute(json!({
                "member_selector": {"selector_type": "any"}
            }))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delegate_task_missing_member_selector() {
        let tool = DelegateTaskTool::new();
        let result = tool
            .execute(json!({
                "goal": "test goal"
            }))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_accept_result_tool_name() {
        let tool = AcceptResultTool::new();
        assert_eq!(tool.name(), "accept_result");
    }

    #[tokio::test]
    async fn test_accept_result_success() {
        let tool = AcceptResultTool::new();
        let result = tool
            .execute(json!({
                "delegation_id": "123e4567-e89b-12d3-a456-426614174000"
            }))
            .await;
        assert!(result.is_ok());
        let tool_result = result.unwrap();
        assert!(tool_result.success);
        assert!(tool_result.content.contains("Accepted delegation"));
    }

    #[tokio::test]
    async fn test_accept_result_missing_delegation_id() {
        let tool = AcceptResultTool::new();
        let result = tool.execute(json!({})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_reject_result_tool_name() {
        let tool = RejectResultTool::new();
        assert_eq!(tool.name(), "reject_result");
    }

    #[tokio::test]
    async fn test_reject_result_success() {
        let tool = RejectResultTool::new();
        let result = tool
            .execute(json!({
                "delegation_id": "123e4567-e89b-12d3-a456-426614174000",
                "reason": "not satisfactory"
            }))
            .await;
        assert!(result.is_ok());
        let tool_result = result.unwrap();
        assert!(tool_result.success);
        assert!(tool_result.content.contains("Rejected delegation"));
        assert!(tool_result.content.contains("not satisfactory"));
    }

    #[tokio::test]
    async fn test_reject_result_missing_reason() {
        let tool = RejectResultTool::new();
        let result = tool
            .execute(json!({
                "delegation_id": "123e4567-e89b-12d3-a456-426614174000"
            }))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_reject_result_missing_delegation_id() {
        let tool = RejectResultTool::new();
        let result = tool
            .execute(json!({
                "reason": "not satisfactory"
            }))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_publish_to_team_tool_name() {
        let tool = PublishToTeamTool::new();
        assert_eq!(tool.name(), "publish_to_team");
    }

    #[tokio::test]
    async fn test_publish_to_team_success() {
        let tool = PublishToTeamTool::new();
        let result = tool
            .execute(json!({
                "artifact_ref": "artifact-123",
                "summary": "test summary",
                "scope": "team_shared"
            }))
            .await;
        assert!(result.is_ok());
        let tool_result = result.unwrap();
        assert!(tool_result.success);
        assert!(tool_result.content.contains("Published artifact"));
        assert!(tool_result.content.contains("team_shared"));
    }

    #[tokio::test]
    async fn test_publish_to_team_missing_artifact_ref() {
        let tool = PublishToTeamTool::new();
        let result = tool
            .execute(json!({
                "summary": "test summary",
                "scope": "team_shared"
            }))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_publish_to_team_missing_summary() {
        let tool = PublishToTeamTool::new();
        let result = tool
            .execute(json!({
                "artifact_ref": "artifact-123",
                "scope": "team_shared"
            }))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_publish_to_team_missing_scope() {
        let tool = PublishToTeamTool::new();
        let result = tool
            .execute(json!({
                "artifact_ref": "artifact-123",
                "summary": "test summary"
            }))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_shared_state_tool_name() {
        let tool = GetSharedStateTool::new();
        assert_eq!(tool.name(), "get_shared_state");
    }

    #[tokio::test]
    async fn test_get_shared_state_success() {
        let tool = GetSharedStateTool::new();
        let result = tool.execute(json!({})).await;
        assert!(result.is_ok());
        let tool_result = result.unwrap();
        assert!(tool_result.success);
        assert!(tool_result.content.contains("accepted_artifact_refs"));
    }

    #[tokio::test]
    async fn test_complete_team_task_tool_name() {
        let tool = CompleteTeamTaskTool::new();
        assert_eq!(tool.name(), "complete_team_task");
    }

    #[tokio::test]
    async fn test_complete_team_task_success() {
        let tool = CompleteTeamTaskTool::new();
        let result = tool
            .execute(json!({
                "summary": "task completed successfully"
            }))
            .await;
        assert!(result.is_ok());
        let tool_result = result.unwrap();
        assert!(tool_result.success);
        assert!(tool_result.content.contains("Task completed"));
    }

    #[tokio::test]
    async fn test_complete_team_task_missing_summary() {
        let tool = CompleteTeamTaskTool::new();
        let result = tool.execute(json!({})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_team_members_tool_name() {
        let tool = ListTeamMembersTool::new();
        assert_eq!(tool.name(), "list_team_members");
    }

    #[tokio::test]
    async fn test_list_team_members_success() {
        let tool = ListTeamMembersTool::new();
        let result = tool.execute(json!({})).await;
        assert!(result.is_ok());
        let tool_result = result.unwrap();
        assert!(tool_result.success);
        assert_eq!(tool_result.content, "[]");
    }

    #[tokio::test]
    async fn test_get_delegation_status_tool_name() {
        let tool = GetDelegationStatusTool::new();
        assert_eq!(tool.name(), "get_delegation_status");
    }

    #[tokio::test]
    async fn test_get_delegation_status_success() {
        let tool = GetDelegationStatusTool::new();
        let result = tool
            .execute(json!({
                "delegation_id": "123e4567-e89b-12d3-a456-426614174000"
            }))
            .await;
        assert!(result.is_ok());
        let tool_result = result.unwrap();
        assert!(tool_result.success);
        assert!(tool_result.content.contains("pending"));
    }

    #[tokio::test]
    async fn test_get_delegation_status_missing_delegation_id() {
        let tool = GetDelegationStatusTool::new();
        let result = tool.execute(json!({})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_update_shared_fact_tool_name() {
        let tool = UpdateSharedFactTool::new();
        assert_eq!(tool.name(), "update_shared_fact");
    }

    #[tokio::test]
    async fn test_update_shared_fact_success() {
        let tool = UpdateSharedFactTool::new();
        let result = tool
            .execute(json!({
                "key": "test_key",
                "value": "test_value"
            }))
            .await;
        assert!(result.is_ok());
        let tool_result = result.unwrap();
        assert!(tool_result.success);
        assert!(tool_result.content.contains("Updated shared fact"));
        assert!(tool_result.content.contains("test_key"));
    }

    #[tokio::test]
    async fn test_update_shared_fact_missing_key() {
        let tool = UpdateSharedFactTool::new();
        let result = tool
            .execute(json!({
                "value": "test_value"
            }))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_update_shared_fact_missing_value() {
        let tool = UpdateSharedFactTool::new();
        let result = tool
            .execute(json!({
                "key": "test_key"
            }))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_add_blocker_tool_name() {
        let tool = AddBlockerTool::new();
        assert_eq!(tool.name(), "add_blocker");
    }

    #[tokio::test]
    async fn test_add_blocker_success() {
        let tool = AddBlockerTool::new();
        let result = tool
            .execute(json!({
                "description": "test blocker"
            }))
            .await;
        assert!(result.is_ok());
        let tool_result = result.unwrap();
        assert!(tool_result.success);
        assert!(tool_result.content.contains("Added blocker"));
        assert!(tool_result.content.contains("test blocker"));
    }

    #[tokio::test]
    async fn test_add_blocker_with_source() {
        let tool = AddBlockerTool::new();
        let result = tool
            .execute(json!({
                "description": "test blocker",
                "source": "test_source"
            }))
            .await;
        assert!(result.is_ok());
        let tool_result = result.unwrap();
        assert!(tool_result.success);
        assert!(tool_result.content.contains("test blocker"));
        assert!(tool_result.content.contains("test_source"));
    }

    #[tokio::test]
    async fn test_add_blocker_missing_description() {
        let tool = AddBlockerTool::new();
        let result = tool.execute(json!({})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_resolve_blocker_tool_name() {
        let tool = ResolveBlockerTool::new();
        assert_eq!(tool.name(), "resolve_blocker");
    }

    #[tokio::test]
    async fn test_resolve_blocker_success() {
        let tool = ResolveBlockerTool::new();
        let result = tool
            .execute(json!({
                "blocker_id": "123e4567-e89b-12d3-a456-426614174000"
            }))
            .await;
        assert!(result.is_ok());
        let tool_result = result.unwrap();
        assert!(tool_result.success);
        assert!(tool_result.content.contains("Resolved blocker"));
    }

    #[tokio::test]
    async fn test_resolve_blocker_missing_blocker_id() {
        let tool = ResolveBlockerTool::new();
        let result = tool.execute(json!({})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_fail_team_task_tool_name() {
        let tool = FailTeamTaskTool::new();
        assert_eq!(tool.name(), "fail_team_task");
    }

    #[tokio::test]
    async fn test_fail_team_task_success() {
        let tool = FailTeamTaskTool::new();
        let result = tool
            .execute(json!({
                "reason": "something went wrong"
            }))
            .await;
        assert!(result.is_ok());
        let tool_result = result.unwrap();
        assert!(tool_result.success);
        assert!(tool_result.content.contains("Task failed"));
        assert!(tool_result.content.contains("something went wrong"));
    }

    #[tokio::test]
    async fn test_fail_team_task_missing_reason() {
        let tool = FailTeamTaskTool::new();
        let result = tool.execute(json!({})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_request_approval_tool_name() {
        let tool = RequestApprovalTool::new();
        assert_eq!(tool.name(), "request_approval");
    }

    #[tokio::test]
    async fn test_request_approval_success() {
        let tool = RequestApprovalTool::new();
        let result = tool
            .execute(json!({
                "tool_name": "dangerous_tool",
                "reason": "need to delete production data"
            }))
            .await;
        assert!(result.is_ok());
        let tool_result = result.unwrap();
        assert!(tool_result.success);
        assert!(tool_result.content.contains("Requested approval"));
        assert!(tool_result.content.contains("dangerous_tool"));
    }

    #[tokio::test]
    async fn test_request_approval_missing_tool_name() {
        let tool = RequestApprovalTool::new();
        let result = tool
            .execute(json!({
                "reason": "need approval"
            }))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_request_approval_missing_reason() {
        let tool = RequestApprovalTool::new();
        let result = tool
            .execute(json!({
                "tool_name": "dangerous_tool"
            }))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_task_details_tool_name() {
        let tool = GetTaskDetailsTool::new();
        assert_eq!(tool.name(), "get_task_details");
    }

    #[tokio::test]
    async fn test_get_task_details_success() {
        let tool = GetTaskDetailsTool::new();
        let result = tool
            .execute(json!({
                "task_id": "123e4567-e89b-12d3-a456-426614174000"
            }))
            .await;
        assert!(result.is_ok());
        let tool_result = result.unwrap();
        assert!(tool_result.success);
        assert!(tool_result.content.contains("Task details for"));
        assert!(tool_result.content.contains("123e4567-e89b-12d3-a456-426614174000"));
    }

    #[tokio::test]
    async fn test_get_task_details_missing_task_id() {
        let tool = GetTaskDetailsTool::new();
        let result = tool.execute(json!({})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_create_supervisor_tools_returns_all_tools() {
        let tools = create_supervisor_tools();
        assert_eq!(tools.len(), 14);

        let tool_names: Vec<&str> = tools.iter().map(|t| t.name()).collect();
        assert!(tool_names.contains(&"delegate_task"));
        assert!(tool_names.contains(&"accept_result"));
        assert!(tool_names.contains(&"reject_result"));
        assert!(tool_names.contains(&"publish_to_team"));
        assert!(tool_names.contains(&"get_shared_state"));
        assert!(tool_names.contains(&"complete_team_task"));
        assert!(tool_names.contains(&"list_team_members"));
        assert!(tool_names.contains(&"get_delegation_status"));
        assert!(tool_names.contains(&"update_shared_fact"));
        assert!(tool_names.contains(&"add_blocker"));
        assert!(tool_names.contains(&"resolve_blocker"));
        assert!(tool_names.contains(&"fail_team_task"));
        assert!(tool_names.contains(&"request_approval"));
        assert!(tool_names.contains(&"get_task_details"));
    }

    #[tokio::test]
    async fn test_publish_to_team_with_private_scope() {
        let tool = PublishToTeamTool::new();
        let result = tool
            .execute(json!({
                "artifact_ref": "artifact-123",
                "summary": "private artifact",
                "scope": "private"
            }))
            .await;
        assert!(result.is_ok());
        let tool_result = result.unwrap();
        assert!(tool_result.success);
        assert!(tool_result.content.contains("private"));
    }

    #[tokio::test]
    async fn test_publish_to_team_with_external_scope() {
        let tool = PublishToTeamTool::new();
        let result = tool
            .execute(json!({
                "artifact_ref": "artifact-123",
                "summary": "external artifact",
                "scope": "external_published"
            }))
            .await;
        assert!(result.is_ok());
        let tool_result = result.unwrap();
        assert!(tool_result.success);
        assert!(tool_result.content.contains("external_published"));
    }
}