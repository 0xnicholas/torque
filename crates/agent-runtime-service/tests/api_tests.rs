mod common;

use common::{setup_test_db_or_skip, test_api_key};
use serial_test::serial;
use agent_runtime_service::repository::{
    MessageRepository, PostgresMessageRepository, PostgresSessionRepository, SessionRepository,
};

#[tokio::test]
#[serial]
async fn test_create_session() {
    let Some(db) = setup_test_db_or_skip().await else {
        return;
    };

    let repo = PostgresSessionRepository::new(db);
    let api_key = test_api_key();
    let session = repo.create(&api_key, "test-scope")
        .await
        .expect("Failed to create session");

    assert_eq!(session.api_key, api_key);
    assert!(matches!(session.status, agent_runtime_service::models::SessionStatus::Idle));
}

#[tokio::test]
#[serial]
async fn test_create_and_get_message() {
    let Some(db) = setup_test_db_or_skip().await else {
        return;
    };
    let session_repo = PostgresSessionRepository::new(db.clone());
    let message_repo = PostgresMessageRepository::new(db);
    let api_key = test_api_key();

    let session = session_repo.create(&api_key, "test-scope")
        .await
        .expect("Failed to create session");

    let message = agent_runtime_service::models::Message::user(
        session.id,
        "Hello, world!".to_string(),
    );

    let saved = message_repo.create(&message)
        .await
        .expect("Failed to create message");

    assert_eq!(saved.content, "Hello, world!");
    assert_eq!(saved.session_id, session.id);

    let messages = message_repo.list_by_session(session.id, 10)
        .await
        .expect("Failed to list messages");

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].content, "Hello, world!");
}

#[tokio::test]
#[serial]
async fn test_session_status_transitions() {
    let Some(db) = setup_test_db_or_skip().await else {
        return;
    };
    let repo = PostgresSessionRepository::new(db);
    let api_key = test_api_key();

    let session = repo.create(&api_key, "test-scope")
        .await
        .expect("Failed to create session");

    repo.update_status(
        session.id,
        agent_runtime_service::models::SessionStatus::Running,
        None,
    )
    .await
    .expect("Failed to update status");

    let updated = repo.get_by_id(session.id)
        .await
        .expect("Failed to get session")
        .expect("Session not found");

    assert!(matches!(updated.status, agent_runtime_service::models::SessionStatus::Running));
}

#[tokio::test]
#[serial]
async fn test_api_key_isolation() {
    let Some(db) = setup_test_db_or_skip().await else {
        return;
    };

    let repo = PostgresSessionRepository::new(db);
    let api_key_1 = "key-1".to_string();
    let api_key_2 = "key-2".to_string();

    let session_1 = repo.create(&api_key_1, "scope-1")
        .await
        .expect("Failed to create session 1");

    let _session_2 = repo.create(&api_key_2, "scope-2")
        .await
        .expect("Failed to create session 2");

    let sessions_1 = repo.list(10)
        .await
        .expect("Failed to list sessions");

    let sessions_1: Vec<_> = sessions_1.into_iter().filter(|s| s.api_key == api_key_1).collect();
    assert_eq!(sessions_1.len(), 1);
    assert_eq!(sessions_1[0].id, session_1.id);
}

#[tokio::test]
#[serial]
async fn test_try_mark_running_is_atomic_gate() {
    let Some(db) = setup_test_db_or_skip().await else {
        return;
    };

    let repo = PostgresSessionRepository::new(db);
    let session = repo.create("key-1", "test-scope")
        .await
        .expect("Failed to create session");

    let first = repo.try_mark_running(session.id)
        .await
        .expect("Failed to mark running first time");
    let second = repo.try_mark_running(session.id)
        .await
        .expect("Failed to mark running second time");

    assert!(first);
    assert!(!second);
}

#[tokio::test]
#[serial]
async fn test_try_mark_running_is_atomic_under_concurrency() {
    let Some(db) = setup_test_db_or_skip().await else {
        return;
    };

    let repo = PostgresSessionRepository::new(db.clone());
    let session = repo.create("key-1", "test-scope")
        .await
        .expect("Failed to create session");

    let session_id = session.id;

    let (first, second) = tokio::join!(
        async {
            let r = PostgresSessionRepository::new(db.clone());
            r.try_mark_running(session_id).await
        },
        async {
            let r = PostgresSessionRepository::new(db.clone());
            r.try_mark_running(session_id).await
        },
    );

    let first = first.expect("first mark should return");
    let second = second.expect("second mark should return");

    assert_ne!(first, second, "exactly one concurrent caller should win");
}
