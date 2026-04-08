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
use llm::OpenAiClient;
use crate::models::SessionStatus;

#[derive(Debug, Serialize)]
pub struct CreateSessionResponse {
    pub id: Uuid,
    pub status: SessionStatus,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct GetSessionResponse {
    pub id: Uuid,
    pub status: SessionStatus,
    pub created_at: String,
    pub updated_at: String,
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
        created_at: session.created_at.to_rfc3339(),
    }))
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

    if session.api_key != api_key {
        return Err(StatusCode::FORBIDDEN);
    }

    Ok(Json(GetSessionResponse {
        id: session.id,
        status: session.status,
        created_at: session.created_at.to_rfc3339(),
        updated_at: session.updated_at.to_rfc3339(),
    }))
}