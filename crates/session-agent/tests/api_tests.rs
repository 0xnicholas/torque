mod common;

use common::{setup_test_db, test_api_key};

#[tokio::test]
async fn test_create_session() {
    let db = setup_test_db().await;
    
    let api_key = test_api_key();
    let session = session_agent::db::sessions::create(db.pool(), &api_key)
        .await
        .expect("Failed to create session");

    assert_eq!(session.api_key, api_key);
    assert!(matches!(session.status, session_agent::models::SessionStatus::Idle));
}

#[tokio::test]
async fn test_create_and_get_message() {
    let db = setup_test_db().await;
    let api_key = test_api_key();
    
    let session = session_agent::db::sessions::create(db.pool(), &api_key)
        .await
        .expect("Failed to create session");

    let message = session_agent::models::Message::user(
        session.id,
        "Hello, world!".to_string(),
    );

    let saved = session_agent::db::messages::create(db.pool(), &message)
        .await
        .expect("Failed to create message");

    assert_eq!(saved.content, "Hello, world!");
    assert_eq!(saved.session_id, session.id);

    let messages = session_agent::db::messages::list_by_session(db.pool(), session.id, 10)
        .await
        .expect("Failed to list messages");

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].content, "Hello, world!");
}

#[tokio::test]
async fn test_session_status_transitions() {
    let db = setup_test_db().await;
    let api_key = test_api_key();
    
    let session = session_agent::db::sessions::create(db.pool(), &api_key)
        .await
        .expect("Failed to create session");

    session_agent::db::sessions::update_status(
        db.pool(),
        session.id,
        session_agent::models::SessionStatus::Running,
        None,
    )
    .await
    .expect("Failed to update status");

    let updated = session_agent::db::sessions::get_by_id(db.pool(), session.id)
        .await
        .expect("Failed to get session")
        .expect("Session not found");

    assert!(matches!(updated.status, session_agent::models::SessionStatus::Running));
}

#[tokio::test]
async fn test_api_key_isolation() {
    let db = setup_test_db().await;
    
    let api_key_1 = "key-1".to_string();
    let api_key_2 = "key-2".to_string();

    let session_1 = session_agent::db::sessions::create(db.pool(), &api_key_1)
        .await
        .expect("Failed to create session 1");

    let _session_2 = session_agent::db::sessions::create(db.pool(), &api_key_2)
        .await
        .expect("Failed to create session 2");

    let sessions_1 = session_agent::db::sessions::list_by_api_key(db.pool(), &api_key_1, 10, 0)
        .await
        .expect("Failed to list sessions");

    assert_eq!(sessions_1.len(), 1);
    assert_eq!(sessions_1[0].id, session_1.id);
}