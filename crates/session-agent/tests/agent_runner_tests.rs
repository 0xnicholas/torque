mod common;

use common::fake_llm::FakeLlm;
use common::setup_test_db_or_skip;
use serde_json::json;
use session_agent::agent::{AgentRunner, StreamEvent};
use session_agent::models::{Message, MessageRole};
use session_agent::tools::ToolRegistry;
use std::sync::Arc;
use tokio::sync::mpsc;

#[tokio::test]
async fn runner_persists_messages_and_emits_start_chunk_done() {
    let Some(db) = setup_test_db_or_skip().await else {
        return;
    };

    let session = session_agent::db::sessions::create(db.pool(), "runner-key")
        .await
        .expect("session should be created");
    let user_message = Message::user(session.id, "hello".to_string());
    let llm = Arc::new(FakeLlm::single_text("hello from fake model"));
    let tools = Arc::new(ToolRegistry::new());
    let runner = AgentRunner::new(llm, db.clone(), tools);
    let (tx, mut rx) = mpsc::channel::<StreamEvent>(64);

    let saved = runner
        .run(&session, &user_message, tx)
        .await
        .expect("runner should complete");

    assert_eq!(saved.content, "hello from fake model");
    assert!(matches!(saved.role, MessageRole::Assistant));

    let messages = session_agent::db::messages::list_by_session(db.pool(), session.id, 10)
        .await
        .expect("messages should be listed");
    assert_eq!(messages.len(), 2);
    assert!(matches!(messages[0].role, MessageRole::User));
    assert!(matches!(messages[1].role, MessageRole::Assistant));

    let mut events = Vec::new();
    while let Some(event) = rx.recv().await {
        events.push(event);
    }

    assert!(matches!(events.first(), Some(StreamEvent::Start { .. })));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, StreamEvent::Chunk { content } if content == "hello from fake model"))
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, StreamEvent::Done { message_id, .. } if *message_id == saved.id))
    );
}

#[tokio::test]
async fn runner_handles_tool_call_and_records_tool_log() {
    let Some(db) = setup_test_db_or_skip().await else {
        return;
    };

    let session = session_agent::db::sessions::create(db.pool(), "runner-key")
        .await
        .expect("session should be created");
    let user_message = Message::user(session.id, "search torque".to_string());
    let llm = Arc::new(FakeLlm::tool_call_then_text(
        "web_search",
        json!({ "query": "torque" }),
        "final tool-assisted answer",
    ));
    let tools = Arc::new(ToolRegistry::new());
    for tool in session_agent::tools::builtin::create_builtin_tools() {
        tools.register(Arc::from(tool)).await;
    }
    let runner = AgentRunner::new(llm, db.clone(), tools);
    let (tx, mut rx) = mpsc::channel::<StreamEvent>(64);

    let saved = runner
        .run(&session, &user_message, tx)
        .await
        .expect("runner should complete");

    assert_eq!(saved.content, "final tool-assisted answer");

    let tool_calls = saved
        .tool_calls
        .as_ref()
        .and_then(|value| value.as_array())
        .expect("tool calls should be persisted");
    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls[0]["name"], "web_search");

    let mut saw_tool_call_event = false;
    let mut saw_tool_result_event = false;
    while let Some(event) = rx.recv().await {
        match event {
            StreamEvent::ToolCall { name, .. } => {
                if name == "web_search" {
                    saw_tool_call_event = true;
                }
            }
            StreamEvent::ToolResult {
                name,
                success,
                content,
                ..
            } => {
                if name == "web_search"
                    && success
                    && content.contains("Mock search results for: torque")
                {
                    saw_tool_result_event = true;
                }
            }
            _ => {}
        }
    }
    assert!(saw_tool_call_event, "tool_call event should be emitted");
    assert!(saw_tool_result_event, "tool_result event should be emitted");
}
