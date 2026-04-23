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