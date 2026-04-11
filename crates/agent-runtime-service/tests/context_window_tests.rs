use agent_runtime_service::agent::context::{ContextManager, DEFAULT_WINDOW_SIZE};
use agent_runtime_service::models::Message;
use uuid::Uuid;

fn build_history(count: usize) -> Vec<Message> {
    let session_id = Uuid::new_v4();
    (0..count)
        .map(|idx| Message::user(session_id, format!("message-{idx}")))
        .collect()
}

#[test]
fn context_manager_only_keeps_recent_window() {
    let history = build_history(25);
    let context = ContextManager::new().build_context(history);

    assert_eq!(context.messages.len(), DEFAULT_WINDOW_SIZE);
    assert_eq!(context.messages.first().expect("first message").content, "message-15");
    assert_eq!(context.messages.last().expect("last message").content, "message-24");
}

#[test]
fn context_manager_preserves_recent_order() {
    let history = build_history(12);
    let context = ContextManager::new().build_context(history);

    assert_eq!(context.messages.first().expect("first message").content, "message-2");
    assert_eq!(context.messages.last().expect("last message").content, "message-11");
}
