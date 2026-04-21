mod common;

use torque_harness::db::Database;
use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use common::{setup_test_db_or_skip, test_api_key};
use llm::OpenAiClient;
use serde_json::Value;
use serial_test::serial;
use std::sync::Arc;
use tower::util::ServiceExt;
use uuid::Uuid;

async fn setup_app() -> Option<(Database, axum::Router)> {
    let Some(db) = setup_test_db_or_skip().await else {
        return None;
    };

    let llm = Arc::new(OpenAiClient::new(
        "http://127.0.0.1:1/v1".to_string(),
        "test-key".to_string(),
        "gpt-4o-mini".to_string(),
    ));

    let app = torque_harness::app::build_app(Database::new(db.pool().clone()), llm);

    Some((db, app))
}

fn expected_project_scope() -> String {
    let cwd = std::env::current_dir().expect("current dir should be available");
    let scope = cwd
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| cwd.display().to_string());

    assert!(!scope.is_empty());
    assert_ne!(scope, "default");

    scope
}

async fn read_json(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body should be readable");

    serde_json::from_slice(&body).expect("response body should be valid json")
}

#[tokio::test]
#[serial]
async fn project_scope_tests_create_session_derives_project_scope_and_persists_it() {
    let Some((db, app)) = setup_app().await else {
        return;
    };

    let expected_project_scope = expected_project_scope();

    let api_key = test_api_key();
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/sessions")
                .header("x-api-key", api_key.as_str())
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("app should respond");

    assert_eq!(response.status(), StatusCode::OK);

    let json = read_json(response).await;
    let session_id = Uuid::parse_str(json["id"].as_str().expect("id should be a string"))
        .expect("id should be a uuid");

    assert_eq!(json["project_scope"], expected_project_scope);

    let row = sqlx::query_scalar::<_, String>("SELECT project_scope FROM sessions WHERE id = $1")
        .bind(session_id)
        .fetch_one(db.pool())
        .await
        .expect("project_scope should be persisted");

    assert_eq!(row, expected_project_scope);
}

#[tokio::test]
#[serial]
async fn project_scope_tests_list_and_get_session_return_derived_project_scope() {
    let Some((_db, app)) = setup_app().await else {
        return;
    };
    let expected_project_scope = expected_project_scope();

    let api_key = test_api_key();

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/sessions")
                .header("x-api-key", api_key.as_str())
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("app should respond");

    assert_eq!(create_response.status(), StatusCode::OK);
    let create_json = read_json(create_response).await;
    let session_id = Uuid::parse_str(create_json["id"].as_str().expect("id should be a string"))
        .expect("id should be a uuid");

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/sessions")
                .header("x-api-key", api_key.as_str())
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("app should respond");

    assert_eq!(list_response.status(), StatusCode::OK);
    let list_json = read_json(list_response).await;
    let sessions = list_json["sessions"]
        .as_array()
        .expect("sessions should be an array");

    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0]["project_scope"], expected_project_scope);

    let get_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/sessions/{session_id}"))
                .header("x-api-key", api_key.as_str())
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("app should respond");

    assert_eq!(get_response.status(), StatusCode::OK);
    let get_json = read_json(get_response).await;

    assert_eq!(get_json["project_scope"], expected_project_scope);
}
