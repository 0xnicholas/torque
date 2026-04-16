use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Serialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::api::middleware::extract_api_key;
use crate::db::Database;
use crate::models::SessionStatus;
use crate::service::ServiceContainer;
use llm::OpenAiClient;

#[derive(Debug, Serialize)]
pub struct CreateSessionResponse {
    pub id: Uuid,
    pub status: SessionStatus,
    pub project_scope: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct GetSessionResponse {
    pub id: Uuid,
    pub status: SessionStatus,
    pub project_scope: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct ListSessionsResponse {
    pub sessions: Vec<GetSessionResponse>,
}

fn to_get_session_response(session: crate::models::Session) -> GetSessionResponse {
    GetSessionResponse {
        id: session.id,
        status: session.status,
        project_scope: session.project_scope,
        created_at: session.created_at.to_rfc3339(),
        updated_at: session.updated_at.to_rfc3339(),
    }
}

pub async fn create(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    request: axum::extract::Request,
) -> Result<Json<CreateSessionResponse>, StatusCode> {
    let api_key = extract_api_key(&request).ok_or(StatusCode::UNAUTHORIZED)?;

    let session = services.session.create(&api_key, "default").await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(CreateSessionResponse {
        id: session.id,
        status: session.status,
        project_scope: session.project_scope,
        created_at: session.created_at.to_rfc3339(),
    }))
}

pub async fn list(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    request: axum::extract::Request,
) -> Result<Json<ListSessionsResponse>, StatusCode> {
    let api_key = extract_api_key(&request).ok_or(StatusCode::UNAUTHORIZED)?;

    let sessions = services.session.list(&api_key, 50).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let sessions = sessions.into_iter().map(to_get_session_response).collect();

    Ok(Json(ListSessionsResponse { sessions }))
}

pub async fn get(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
    request: axum::extract::Request,
) -> Result<Json<GetSessionResponse>, StatusCode> {
    let api_key = extract_api_key(&request).ok_or(StatusCode::UNAUTHORIZED)?;

    match services.session.get_by_id(id, &api_key).await {
        Ok(session) => Ok(Json(to_get_session_response(session))),
        Err(crate::service::session::SessionServiceError::NotFound) => Err(StatusCode::NOT_FOUND),
        Err(crate::service::session::SessionServiceError::Forbidden) => Err(StatusCode::FORBIDDEN),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}
