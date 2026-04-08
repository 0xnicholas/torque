mod common;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use llm::OpenAiClient;
use serde_json::Value;
use session_agent::app::build_app;
use tower::util::ServiceExt;
use std::sync::Arc;

use common::setup_test_db_or_skip;

#[tokio::test]
async fn chat_endpoint_emits_start_event_in_sse_stream() {
    let Some(db) = setup_test_db_or_skip().await else {
        return;
    };

    let llm = Arc::new(OpenAiClient::new(
        "http://127.0.0.1:1/v1".to_string(),
        "test-key".to_string(),
        "gpt-4o-mini".to_string(),
    ));
    let app = build_app(db, llm);

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/sessions")
                .header("x-api-key", "test-api-key")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("create session request should succeed");
    assert_eq!(create_response.status(), StatusCode::OK);

    let create_body = to_bytes(create_response.into_body(), usize::MAX)
        .await
        .expect("create response body should be readable");
    let create_json: Value = serde_json::from_slice(&create_body)
        .expect("create response should be valid json");
    let session_id = create_json["id"]
        .as_str()
        .expect("session id should exist")
        .to_string();

    let chat_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/sessions/{session_id}/chat"))
                .header("x-api-key", "test-api-key")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"message":"hello"}"#))
                .expect("chat request should build"),
        )
        .await
        .expect("chat request should succeed");
    assert_eq!(chat_response.status(), StatusCode::OK);

    let body = to_bytes(chat_response.into_body(), usize::MAX)
        .await
        .expect("chat response body should be readable");
    let sse = String::from_utf8(body.to_vec()).expect("sse should be utf8");

    assert!(sse.contains(r#""event":"start""#));
    assert!(
        sse.contains(r#""event":"done""#) || sse.contains(r#""event":"error""#),
        "stream should terminate with done or error event"
    );
}
