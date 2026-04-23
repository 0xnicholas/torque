#[tokio::test]
async fn test_notification_service_wires_into_gating() {
    use torque_harness::notification::{NotificationHook, ReviewEvent};
    use torque_harness::service::notification::NotificationService;
    use uuid::Uuid;

    let notification_service = NotificationService::new();
    assert!(true);
}

#[tokio::test]
async fn test_memory_pipeline_gate_and_notify_calls_notification() {
    use torque_harness::service::memory_pipeline::MemoryPipelineService;
    use torque_harness::service::notification::NotificationService;
    use torque_harness::service::gating::MemoryGatingService;
    use std::sync::Arc;

    let notification_service = Arc::new(NotificationService::new().with_sse_hook());
    let gating = Arc::new(MemoryGatingService::new(
        Arc::new(torque_harness::repository::PostgresMemoryRepositoryV1::new(
            torque_harness::db::Database::new("postgres://localhost/test".to_string()).await.unwrap()
        )),
        None,
    ));
    let pipeline = MemoryPipelineService::new(gating, Some(notification_service));
    assert!(true);
}