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

#[tokio::test]
async fn test_supervisor_agent_triage_returns_structured_result() {
    let triage_json = serde_json::json!({
        "complexity": "Medium",
        "processing_path": "GuidedDelegate",
        "selected_mode": "Route",
        "lead_member_ref": null,
        "rationale": "Task involves coordinating multiple specialists"
    }).to_string();

    let llm = Arc::new(FakeLlm::json_response(triage_json));
    let tools = torque_harness::service::team::supervisor_tools::create_supervisor_tools();
    let agent = SupervisorAgent::new(llm, tools).await;

    let result = agent.triage("Coordinate a marketing campaign across design, content, and media teams").await;

    assert!(result.is_ok(), "Triage should succeed");
    let triage_result = result.unwrap();
    assert_eq!(triage_result.complexity, torque_harness::models::v1::team::TaskComplexity::Medium);
    assert_eq!(triage_result.processing_path, torque_harness::models::v1::team::ProcessingPath::GuidedDelegate);
    assert_eq!(triage_result.selected_mode, torque_harness::models::v1::team::TeamMode::Route);
}

#[tokio::test]
async fn test_supervisor_agent_triage_simple_task() {
    let triage_json = serde_json::json!({
        "complexity": "Simple",
        "processing_path": "SingleRoute",
        "selected_mode": "Route",
        "lead_member_ref": null,
        "rationale": "Simple straightforward task"
    }).to_string();

    let llm = Arc::new(FakeLlm::json_response(triage_json));
    let tools = torque_harness::service::team::supervisor_tools::create_supervisor_tools();
    let agent = SupervisorAgent::new(llm, tools).await;

    let result = agent.triage("Send an email to John").await;

    assert!(result.is_ok(), "Triage should succeed");
    let triage_result = result.unwrap();
    assert_eq!(triage_result.complexity, torque_harness::models::v1::team::TaskComplexity::Simple);
}

#[tokio::test]
async fn test_supervisor_agent_triage_complex_task() {
    let triage_json = serde_json::json!({
        "complexity": "Complex",
        "processing_path": "StructuredOrchestration",
        "selected_mode": "Tasks",
        "lead_member_ref": "senior-engineer",
        "rationale": "Complex multi-team coordination required"
    }).to_string();

    let llm = Arc::new(FakeLlm::json_response(triage_json));
    let tools = torque_harness::service::team::supervisor_tools::create_supervisor_tools();
    let agent = SupervisorAgent::new(llm, tools).await;

    let result = agent.triage("Build a distributed system with multiple microservices").await;

    assert!(result.is_ok(), "Triage should succeed");
    let triage_result = result.unwrap();
    assert_eq!(triage_result.complexity, torque_harness::models::v1::team::TaskComplexity::Complex);
    assert_eq!(triage_result.processing_path, torque_harness::models::v1::team::ProcessingPath::StructuredOrchestration);
    assert_eq!(triage_result.selected_mode, torque_harness::models::v1::team::TeamMode::Tasks);
    assert_eq!(triage_result.lead_member_ref, Some("senior-engineer".to_string()));
}
