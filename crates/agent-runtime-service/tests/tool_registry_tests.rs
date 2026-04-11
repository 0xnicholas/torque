use agent_runtime_service::tools::builtin::create_builtin_tools;

#[test]
fn builtin_tools_are_demo_safe_and_minimal() {
    let tools = create_builtin_tools();

    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name(), "web_search");
}
