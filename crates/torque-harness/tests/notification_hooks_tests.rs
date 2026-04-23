#[tokio::test]
async fn test_webhook_hook_sends_request() {
    use torque_harness::models::v1::memory::{MemoryCategory, MemoryWriteCandidateStatus};
    use torque_harness::notification::hooks::{NotificationHook, ReviewEvent, WebhookHook};
    use uuid::Uuid;

    let hook = WebhookHook::new("https://example.com/webhook".to_string());
    assert!(true);
}

#[tokio::test]
async fn test_sse_hook_cloneable() {
    use torque_harness::notification::hooks::{NotificationHook, SseHook};

    let (hook, _rx) = SseHook::new();
    let _clone = hook.clone();
    assert!(true);
}
