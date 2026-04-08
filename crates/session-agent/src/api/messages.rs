use axum::{
    body::Body,
    extract::{Extension, Path, State},
    http::StatusCode,
    response::Response,
    Json,
};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use subtle::ConstantTimeEq;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

use crate::api::middleware::extract_api_key;
use crate::agent::{AgentRunner, StreamEvent};
use crate::db::Database;
use llm::OpenAiClient;
use crate::models::{Message, SessionStatus};
use crate::tools::ToolRegistry;

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct MessageResponse {
    pub id: Uuid,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

pub async fn list(
    State((db, _)): State<(Database, Arc<OpenAiClient>)>,
    Path(session_id): Path<Uuid>,
    request: axum::extract::Request,
) -> Result<Json<Vec<MessageResponse>>, StatusCode> {
    let api_key = extract_api_key(&request).ok_or(StatusCode::UNAUTHORIZED)?;

    let session = crate::db::sessions::get_by_id(db.pool(), session_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    if !bool::from(session.api_key.as_bytes().ct_eq(api_key.as_bytes())) {
        return Err(StatusCode::FORBIDDEN);
    }

    let messages = crate::db::messages::list_by_session(db.pool(), session_id, 100)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let responses = messages
        .into_iter()
        .map(|m| MessageResponse {
            id: m.id,
            role: format!("{:?}", m.role).to_lowercase(),
            content: m.content,
            created_at: m.created_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(responses))
}

#[axum::debug_handler]
pub async fn chat(
    State((db, llm)): State<(Database, Arc<OpenAiClient>)>,
    Path(session_id): Path<Uuid>,
    Extension(api_key): Extension<String>,
    Json(req): Json<ChatRequest>,
) -> Result<Response, StatusCode> {

    let session = crate::db::sessions::get_by_id(db.pool(), session_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    if !bool::from(session.api_key.as_bytes().ct_eq(api_key.as_bytes())) {
        return Err(StatusCode::FORBIDDEN);
    }

    if !session.can_receive_message() {
        return Err(StatusCode::CONFLICT);
    }

    let tools = Arc::new(ToolRegistry::new());
    for tool in crate::tools::builtin::create_builtin_tools() {
        tools.register(Arc::from(tool)).await;
    }

    let user_message = Message::user(session_id, req.message);

    let (tx, rx) = mpsc::channel::<StreamEvent>(100);

    crate::db::sessions::update_status(
        db.pool(),
        session.id,
        SessionStatus::Running,
        None,
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let runner = AgentRunner::new(llm.clone(), db.clone(), tools);
    let session_clone = session.clone();
    let user_message_clone = user_message.clone();

    tokio::spawn(async move {
        match runner.run(&session_clone, &user_message_clone, tx.clone()).await {
            Ok(_) => {
                let _ = crate::db::sessions::update_status(
                    db.pool(),
                    session_clone.id,
                    SessionStatus::Completed,
                    None,
                )
                .await;
            }
            Err(e) => {
                let _ = tx
                    .send(StreamEvent::Error {
                        code: "AGENT_ERROR".to_string(),
                        message: e.to_string(),
                    })
                    .await;

                let _ = crate::db::sessions::update_status(
                    db.pool(),
                    session_clone.id,
                    SessionStatus::Error,
                    Some(&e.to_string()),
                )
                .await;
            }
        }
    });

    let stream = ReceiverStream::new(rx).map(|event| {
        Ok::<_, std::convert::Infallible>(event.to_sse())
    });

    let response = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/event-stream")
        .header("cache-control", "no-cache")
        .body(Body::from_stream(stream))
        .unwrap();

    Ok(response)
}
