use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Serialize;
use std::sync::Arc;
use subtle::ConstantTimeEq;
use uuid::Uuid;

use crate::api::middleware::extract_api_key;
use crate::db::Database;
use crate::models::SessionStatus;
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
    State((db, _)): State<(Database, Arc<OpenAiClient>)>,
    request: axum::extract::Request,
) -> Result<Json<CreateSessionResponse>, StatusCode> {
    let api_key = extract_api_key(&request).ok_or(StatusCode::UNAUTHORIZED)?;

    let session = crate::db::sessions::create(db.pool(), &api_key)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(CreateSessionResponse {
        id: session.id,
        status: session.status,
        project_scope: session.project_scope,
        created_at: session.created_at.to_rfc3339(),
    }))
}

pub async fn list(
    State((db, _)): State<(Database, Arc<OpenAiClient>)>,
    request: axum::extract::Request,
) -> Result<Json<ListSessionsResponse>, StatusCode> {
    let api_key = extract_api_key(&request).ok_or(StatusCode::UNAUTHORIZED)?;

    let sessions = crate::db::sessions::list_by_api_key(db.pool(), &api_key, 50, 0)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let sessions = sessions.into_iter().map(to_get_session_response).collect();

    Ok(Json(ListSessionsResponse { sessions }))
}

pub async fn get(
    State((db, _)): State<(Database, Arc<OpenAiClient>)>,
    Path(id): Path<Uuid>,
    request: axum::extract::Request,
) -> Result<Json<GetSessionResponse>, StatusCode> {
    let api_key = extract_api_key(&request).ok_or(StatusCode::UNAUTHORIZED)?;

    let session = crate::db::sessions::get_by_id(db.pool(), id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    if !bool::from(session.api_key.as_bytes().ct_eq(api_key.as_bytes())) {
        return Err(StatusCode::FORBIDDEN);
    }

    Ok(Json(to_get_session_response(session)))
}
