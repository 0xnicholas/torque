use torque_harness::tools::Tool;

#[tokio::test]
async fn test_delegate_task_tool_schema() {
    let tool = torque_harness::service::team::supervisor_tools::DelegateTaskTool::new();
    let schema = tool.parameters_schema();

    assert_eq!(tool.name(), "delegate_task");
    assert!(schema.pointer("/properties/member_selector").is_some());
    assert!(schema.pointer("/properties/goal").is_some());
    assert!(schema.pointer("/properties/instructions").is_some());
}

#[tokio::test]
async fn test_accept_result_tool() {
    let tool = torque_harness::service::team::supervisor_tools::AcceptResultTool::new();
    let schema = tool.parameters_schema();

    assert_eq!(tool.name(), "accept_result");
    assert!(schema.pointer("/properties/delegation_id").is_some());
}

#[tokio::test]
async fn test_reject_result_tool() {
    let tool = torque_harness::service::team::supervisor_tools::RejectResultTool::new();
    let schema = tool.parameters_schema();

    assert_eq!(tool.name(), "reject_result");
    assert!(schema.pointer("/properties/delegation_id").is_some());
    assert!(schema.pointer("/properties/reason").is_some());
    assert!(schema.pointer("/properties/reroute").is_some());
}

#[tokio::test]
async fn test_publish_to_team_tool() {
    let tool = torque_harness::service::team::supervisor_tools::PublishToTeamTool::new();
    let schema = tool.parameters_schema();

    assert_eq!(tool.name(), "publish_to_team");
    assert!(schema.pointer("/properties/artifact_ref").is_some());
    assert!(schema.pointer("/properties/summary").is_some());
    assert!(schema.pointer("/properties/scope").is_some());
}

#[tokio::test]
async fn test_get_shared_state_tool() {
    let tool = torque_harness::service::team::supervisor_tools::GetSharedStateTool::new();
    let schema = tool.parameters_schema();

    assert_eq!(tool.name(), "get_shared_state");
}

#[tokio::test]
async fn test_complete_team_task_tool() {
    let tool = torque_harness::service::team::supervisor_tools::CompleteTeamTaskTool::new();
    let schema = tool.parameters_schema();

    assert_eq!(tool.name(), "complete_team_task");
    assert!(schema.pointer("/properties/summary").is_some());
    assert!(schema.pointer("/properties/output_artifacts").is_some());
}

#[tokio::test]
async fn test_list_team_members_tool() {
    let tool = torque_harness::service::team::supervisor_tools::ListTeamMembersTool::new();
    let schema = tool.parameters_schema();

    assert_eq!(tool.name(), "list_team_members");
}

#[tokio::test]
async fn test_get_delegation_status_tool() {
    let tool = torque_harness::service::team::supervisor_tools::GetDelegationStatusTool::new();
    let schema = tool.parameters_schema();

    assert_eq!(tool.name(), "get_delegation_status");
    assert!(schema.pointer("/properties/delegation_id").is_some());
}

#[tokio::test]
async fn test_update_shared_fact_tool() {
    let tool = torque_harness::service::team::supervisor_tools::UpdateSharedFactTool::new();
    let schema = tool.parameters_schema();

    assert_eq!(tool.name(), "update_shared_fact");
    assert!(schema.pointer("/properties/key").is_some());
    assert!(schema.pointer("/properties/value").is_some());
}

#[tokio::test]
async fn test_add_blocker_tool() {
    let tool = torque_harness::service::team::supervisor_tools::AddBlockerTool::new();
    let schema = tool.parameters_schema();

    assert_eq!(tool.name(), "add_blocker");
    assert!(schema.pointer("/properties/description").is_some());
    assert!(schema.pointer("/properties/source").is_some());
}

#[tokio::test]
async fn test_resolve_blocker_tool() {
    let tool = torque_harness::service::team::supervisor_tools::ResolveBlockerTool::new();
    let schema = tool.parameters_schema();

    assert_eq!(tool.name(), "resolve_blocker");
    assert!(schema.pointer("/properties/blocker_id").is_some());
}

#[tokio::test]
async fn test_fail_team_task_tool() {
    let tool = torque_harness::service::team::supervisor_tools::FailTeamTaskTool::new();
    let schema = tool.parameters_schema();

    assert_eq!(tool.name(), "fail_team_task");
    assert!(schema.pointer("/properties/reason").is_some());
}

#[tokio::test]
async fn test_request_approval_tool() {
    let tool = torque_harness::service::team::supervisor_tools::RequestApprovalTool::new();
    let schema = tool.parameters_schema();

    assert_eq!(tool.name(), "request_approval");
    assert!(schema.pointer("/properties/tool_name").is_some());
    assert!(schema.pointer("/properties/reason").is_some());
}

#[tokio::test]
async fn test_get_task_details_tool() {
    let tool = torque_harness::service::team::supervisor_tools::GetTaskDetailsTool::new();
    let schema = tool.parameters_schema();

    assert_eq!(tool.name(), "get_task_details");
    assert!(schema.pointer("/properties/task_id").is_some());
}

#[tokio::test]
async fn test_supervisor_tools_registry() {
    use torque_harness::service::team::supervisor_tools::create_supervisor_tools;
    use torque_harness::tools::Tool;

    let tools = create_supervisor_tools();

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
    assert_eq!(tool_names.len(), 14);
}

#[tokio::test]
async fn test_tools_in_registry() {
    use torque_harness::tools::ToolRegistry;
    use torque_harness::service::team::supervisor_tools::create_supervisor_tools;

    let mut registry = ToolRegistry::new();
    let supervisor_tools = create_supervisor_tools();

    for tool in supervisor_tools {
        registry.register(tool).await;
    }

    let tools = registry.list().await;
    let tool_names: Vec<&str> = tools.iter().map(|t| t.name()).collect();
    assert!(tool_names.contains(&"delegate_task"));
    assert!(tool_names.contains(&"accept_result"));
}
