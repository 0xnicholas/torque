mod common;

use common::fake_llm::FakeLlm;
use common::setup_test_db_or_skip;
use serde_json::json;
use serial_test::serial;
use std::sync::Arc;
use tokio::sync::mpsc;
use torque_harness::agent::StreamEvent;
use torque_harness::models::MessageRole;
use torque_harness::repository::{
    PostgresMemoryRepository, PostgresMessageRepository, PostgresSessionRepository,
};
use torque_harness::service::{MemoryService, RuntimeFactory, SessionService, ToolService};
use torque_harness::tools::builtin::create_demo_builtin_tools;

async fn build_session_service(
    db: torque_harness::db::Database,
    llm: Arc<dyn llm::LlmClient>,
) -> SessionService {
    let session_repo = Arc::new(PostgresSessionRepository::new(db.clone()));
    let message_repo = Arc::new(PostgresMessageRepository::new(db.clone()));
    let memory_repo = Arc::new(PostgresMemoryRepository::new(db.clone()));

    let tool = Arc::new(ToolService::new());
    let memory_v1_repo = Arc::new(torque_harness::repository::PostgresMemoryRepositoryV1::new(
        db.clone(),
    ));
    let memory = Arc::new(MemoryService::new(memory_repo, memory_v1_repo, None));

    let event_repo = Arc::new(torque_harness::repository::PostgresEventRepository::new(db.clone()));
    let _checkpoint_repo = Arc::new(torque_harness::repository::PostgresCheckpointRepository::new(db.clone()));
    let checkpointer = Arc::new(torque_harness::runtime::checkpoint::PostgresCheckpointer::new(
        db.clone(),
    ));
    let runtime_factory = Arc::new(RuntimeFactory::new(
        event_repo,
        checkpointer,
    ));

    SessionService::new(
        session_repo,
        message_repo,
        runtime_factory,
        llm,
        tool,
        memory,
    )
}

#[tokio::test]
#[serial]
async fn chat_persists_messages_and_emits_start_chunk_done() {
    let Some(db) = setup_test_db_or_skip().await else {
        return;
    };

    let llm = Arc::new(FakeLlm::single_text("hello from fake model"));
    let svc = build_session_service(db.clone(), llm).await;

    let session = svc
        .create("runner-key", "test-scope")
        .await
        .expect("session should be created");

    let (tx, mut rx) = mpsc::channel::<StreamEvent>(64);

    svc.chat(session.id, "runner-key", "hello".to_string(), tx)
        .await
        .expect("chat should complete");

    let messages = svc
        .list_messages(session.id)
        .await
        .expect("messages should be listed");
    assert_eq!(messages.len(), 2);
    assert!(matches!(messages[0].role, MessageRole::User));
    assert_eq!(messages[0].content, "hello");
    assert!(matches!(messages[1].role, MessageRole::Assistant));
    assert_eq!(messages[1].content, "hello from fake model");

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
    assert!(events
        .iter()
        .any(|event| matches!(event, StreamEvent::Done { .. })));
}

#[tokio::test]
#[serial]
async fn chat_handles_tool_call_and_records_tool_log() {
    let Some(db) = setup_test_db_or_skip().await else {
        return;
    };

    let llm = Arc::new(FakeLlm::tool_call_then_text(
        "web_search",
        json!({ "query": "torque" }),
        "final tool-assisted answer",
    ));
    let svc = build_session_service(db.clone(), llm).await;

    let session = svc
        .create("runner-key", "test-scope")
        .await
        .expect("session should be created");

    for tool in create_demo_builtin_tools() {
        svc.tools().registry().register(Arc::from(tool)).await;
    }

    let (tx, mut rx) = mpsc::channel::<StreamEvent>(64);

    svc.chat(session.id, "runner-key", "search torque".to_string(), tx)
        .await
        .expect("chat should complete");

    let messages = svc
        .list_messages(session.id)
        .await
        .expect("messages should be listed");
    let assistant_msg = messages
        .iter()
        .find(|m| matches!(m.role, MessageRole::Assistant))
        .expect("assistant message should exist");
    assert_eq!(assistant_msg.content, "final tool-assisted answer");

    let tool_calls = assistant_msg
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
