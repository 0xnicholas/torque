mod common;

use common::fake_llm::FakeLlm;
use std::sync::Arc;
use tokio::sync::mpsc;
use torque_harness::agent::StreamEvent;
use torque_harness::service::team::supervisor_agent::SupervisorAgent;

#[tokio::test]
async fn test_supervisor_agent_delegation_flow() {
    let llm = Arc::new(FakeLlm::single_text("complete"));
    let tools = torque_harness::service::team::supervisor_tools::create_supervisor_tools();
    let mut agent = SupervisorAgent::new(llm, tools).await;

    let (tx, mut rx) = mpsc::channel::<StreamEvent>(100);

    let task = "Delegate the task 'Write a report' to a writer. Accept the result when complete.";
    let result = agent.run(task, tx).await;

    match result {
        Ok(step) => {
            let history = agent.step_history();
            assert!(!history.is_empty(), "Agent should have some step history");
        }
        Err(e) => {
            tracing::info!("Agent run ended with: {:?}", e);
        }
    }

    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }
    tracing::debug!("Collected {} events", events.len());
}

#[tokio::test]
async fn test_supervisor_agent_tool_call_flow() {
    use serde_json::json;

    let llm = Arc::new(FakeLlm::tool_call_then_text(
        "delegate_task",
        json!({
            "member_selector": {"type": "specialist"},
            "goal": "Write a report"
        }),
        "Task delegated successfully",
    ));
    let tools = torque_harness::service::team::supervisor_tools::create_supervisor_tools();
    let mut agent = SupervisorAgent::new(llm, tools).await;

    let (tx, _rx) = mpsc::channel::<StreamEvent>(100);

    let task = "Delegate the task 'Write a report' to a writer.";
    let result = agent.run(task, tx).await;

    assert!(result.is_ok(), "Agent should complete without error");
    let history = agent.step_history();
    assert!(!history.is_empty(), "Agent should have step history after tool call");
}

#[tokio::test]
async fn test_supervisor_agent_lists_available_tools() {
    let llm = Arc::new(FakeLlm::single_text("complete"));
    let tools = torque_harness::service::team::supervisor_tools::create_supervisor_tools();
    let agent = SupervisorAgent::new(llm, tools).await;

    let tool_names = agent.list_tool_names().await;
    assert!(tool_names.contains(&"delegate_task".to_string()));
    assert!(tool_names.contains(&"accept_result".to_string()));
    assert!(tool_names.contains(&"reject_result".to_string()));
    assert!(tool_names.contains(&"publish_to_team".to_string()));
    assert!(tool_names.contains(&"get_shared_state".to_string()));
    assert!(tool_names.contains(&"complete_team_task".to_string()));
    assert!(tool_names.contains(&"list_team_members".to_string()));
}
