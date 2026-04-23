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
}
