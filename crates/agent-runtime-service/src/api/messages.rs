use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{sse::Sse, IntoResponse, Response},
    Json,
};
use futures::StreamExt;
use serde::Deserialize;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

use crate::agent::stream::StreamEvent;
use crate::service::ServiceContainer;

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub message: String,
}

pub async fn list(
    State((_, _, services)): State<(crate::db::Database, Arc<llm::OpenAiClient>, Arc<ServiceContainer>)>,
    Path(session_id): Path<Uuid>,
    request: axum::extract::Request,
) -> Result<Json<Vec<crate::models::Message>>, StatusCode> {
    let api_key = crate::api::middleware::extract_api_key(&request).ok_or(StatusCode::UNAUTHORIZED)?;

    match services.session.get_by_id(session_id, &api_key).await {
        Ok(_) => {}
        Err(crate::service::session::SessionServiceError::NotFound) => return Err(StatusCode::NOT_FOUND),
        Err(crate::service::session::SessionServiceError::Forbidden) => return Err(StatusCode::FORBIDDEN),
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    }

    let messages = services.session.list_messages(session_id).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(messages))
}

pub async fn chat(
    State((_, _, services)): State<(crate::db::Database, Arc<llm::OpenAiClient>, Arc<ServiceContainer>)>,
    Path(session_id): Path<Uuid>,
    Extension(api_key): Extension<String>,
    Json(req): Json<ChatRequest>,
) -> Result<Response, StatusCode> {
    let (tx, rx) = mpsc::channel::<StreamEvent>(100);

    let session_svc = services.session.clone();
    tokio::spawn(async move {
        let _ = session_svc.chat(session_id, &api_key, req.message, tx).await;
    });

    let stream = ReceiverStream::new(rx).map(|event| {
        Ok::<_, Infallible>(crate::infra::stream::event_to_sse(event).unwrap())
    });

    Ok(Sse::new(stream).into_response())
}
