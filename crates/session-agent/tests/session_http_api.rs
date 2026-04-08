use axum::body::Body;
use axum::http::{Request, StatusCode};
use llm::OpenAiClient;
use serde_json::json;
use session_agent::db::Database;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tower::util::ServiceExt;
use uuid::Uuid;

#[tokio::test]
async fn create_session_route_works_through_app_builder() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://postgres:postgres@localhost/session_agent_test")
        .expect("lazy pool should build");
    let db = Database::new(pool);
    let llm = Arc::new(OpenAiClient::new(
        "http://127.0.0.1:1/v1".to_string(),
        "test-key".to_string(),
        "gpt-4o-mini".to_string(),
    ));

    let app = session_agent::app::build_app(db, llm);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/sessions")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("app should respond");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn list_sessions_route_is_wired_and_protected_by_auth() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://postgres:postgres@localhost/session_agent_test")
        .expect("lazy pool should build");
    let db = Database::new(pool);
    let llm = Arc::new(OpenAiClient::new(
        "http://127.0.0.1:1/v1".to_string(),
        "test-key".to_string(),
        "gpt-4o-mini".to_string(),
    ));

    let app = session_agent::app::build_app(db, llm);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/sessions")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("app should respond");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn list_messages_route_is_wired_and_protected_by_auth() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://postgres:postgres@localhost/session_agent_test")
        .expect("lazy pool should build");
    let db = Database::new(pool);
    let llm = Arc::new(OpenAiClient::new(
        "http://127.0.0.1:1/v1".to_string(),
        "test-key".to_string(),
        "gpt-4o-mini".to_string(),
    ));

    let app = session_agent::app::build_app(db, llm);
    let session_id = Uuid::new_v4();

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/sessions/{session_id}/messages"))
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("app should respond");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn chat_route_is_wired_and_protected_by_auth() {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://postgres:postgres@localhost/session_agent_test")
        .expect("lazy pool should build");
    let db = Database::new(pool);
    let llm = Arc::new(OpenAiClient::new(
        "http://127.0.0.1:1/v1".to_string(),
        "test-key".to_string(),
        "gpt-4o-mini".to_string(),
    ));

    let app = session_agent::app::build_app(db, llm);
    let session_id = Uuid::new_v4();
    let body = Body::from(json!({ "message": "hello" }).to_string());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/sessions/{session_id}/chat"))
                .header("content-type", "application/json")
                .body(body)
                .expect("request should build"),
        )
        .await
        .expect("app should respond");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
