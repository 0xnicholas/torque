use crate::db::Database;
use crate::models::v1::common::ErrorBody;
use crate::models::v1::session::{
    CompactionAbortResponse, CompactJobResponse, Session, SessionChatRequest,
    SessionCompactRequest, SessionCreateRequest,
};
use crate::service::ServiceContainer;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use llm::LlmClient;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

type ApiState = (Database, Arc<dyn LlmClient>, Arc<ServiceContainer>);

#[derive(Serialize)]
pub struct SessionResponse {
    pub session: Session,
}

#[derive(Serialize)]
pub struct SessionListResponse {
    pub sessions: Vec<Session>,
    pub total: Option<i64>,
}

#[derive(Deserialize)]
pub struct ListParams {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

// ── CRUD ────────────────────────────────────────────────────────

/// POST /v1/sessions — Create a new session.
pub async fn create(
    State((_, _, services)): State<ApiState>,
    Json(req): Json<SessionCreateRequest>,
) -> Result<(StatusCode, Json<SessionResponse>), (StatusCode, Json<ErrorBody>)> {
    let tenant_id = Uuid::new_v4(); // TODO: extract from auth context
    let session = services
        .session_service
        .create(tenant_id, req)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    code: "SESSION_CREATE_FAILED".into(),
                    message: e.to_string(),
                    details: None,
                    request_id: None,
                }),
            )
        })?;

    Ok((StatusCode::CREATED, Json(SessionResponse { session })))
}

/// GET /v1/sessions — List sessions.
pub async fn list(
    State((_, _, services)): State<ApiState>,
    Query(params): Query<ListParams>,
) -> Result<Json<SessionListResponse>, (StatusCode, Json<ErrorBody>)> {
    let tenant_id = Uuid::new_v4();
    let limit = params.limit.unwrap_or(50).min(200);
    let offset = params.offset.unwrap_or(0);

    let sessions = services
        .session_service
        .list(tenant_id, limit, offset)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    code: "SESSION_LIST_FAILED".into(),
                    message: e.to_string(),
                    details: None,
                    request_id: None,
                }),
            )
        })?;

    Ok(Json(SessionListResponse {
        sessions,
        total: None,
    }))
}

/// GET /v1/sessions/:id — Get a session by ID.
pub async fn get(
    State((_, _, services)): State<ApiState>,
    Path(id): Path<Uuid>,
) -> Result<Json<SessionResponse>, (StatusCode, Json<ErrorBody>)> {
    let tenant_id = Uuid::new_v4();

    let session = services
        .session_service
        .get(id, tenant_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    code: "SESSION_GET_FAILED".into(),
                    message: e.to_string(),
                    details: None,
                    request_id: None,
                }),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorBody {
                    code: "SESSION_NOT_FOUND".into(),
                    message: format!("Session {} not found", id),
                    details: None,
                    request_id: None,
                }),
            )
        })?;

    Ok(Json(SessionResponse { session }))
}

/// DELETE /v1/sessions/:id — Delete a session.
pub async fn delete(
    State((_, _, services)): State<ApiState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ErrorBody>)> {
    let tenant_id = Uuid::new_v4();

    services
        .session_service
        .delete(id, tenant_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    code: "SESSION_DELETE_FAILED".into(),
                    message: e.to_string(),
                    details: None,
                    request_id: None,
                }),
            )
        })?;

    Ok(StatusCode::NO_CONTENT)
}

// ── Chat ────────────────────────────────────────────────────────

/// POST /v1/sessions/:id/chat — Send a message to a session.
pub async fn chat(
    State((_, _, services)): State<ApiState>,
    Path(id): Path<Uuid>,
    Json(req): Json<SessionChatRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorBody>)> {
    let tenant_id = Uuid::new_v4();

    let agent_instance_id = services
        .session_service
        .chat(id, tenant_id, req)
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorBody {
                    code: "SESSION_CHAT_FAILED".into(),
                    message: e.to_string(),
                    details: None,
                    request_id: None,
                }),
            )
        })?;

    Ok(Json(serde_json::json!({
        "session_id": id,
        "agent_instance_id": agent_instance_id,
        "status": "accepted"
    })))
}

// ── Compaction ──────────────────────────────────────────────────

/// POST /v1/sessions/:id/compact — Trigger context compaction.
pub async fn compact(
    State((_, _, services)): State<ApiState>,
    Path(id): Path<Uuid>,
    Json(req): Json<SessionCompactRequest>,
) -> Result<Json<CompactJobResponse>, (StatusCode, Json<ErrorBody>)> {
    let tenant_id = Uuid::new_v4();

    let response = services
        .session_service
        .compact(id, tenant_id, req)
        .await
        .map_err(|e| {
            (
                StatusCode::CONFLICT,
                Json(ErrorBody {
                    code: "SESSION_COMPACT_FAILED".into(),
                    message: e.to_string(),
                    details: None,
                    request_id: None,
                }),
            )
        })?;

    Ok(Json(response))
}

/// POST /v1/sessions/:id/compaction/abort — Abort in-flight compaction.
pub async fn abort_compaction(
    State((_, _, services)): State<ApiState>,
    Path(id): Path<Uuid>,
) -> Result<Json<CompactionAbortResponse>, (StatusCode, Json<ErrorBody>)> {
    let tenant_id = Uuid::new_v4();

    let response = services
        .session_service
        .abort_compaction(id, tenant_id)
        .await
        .map_err(|e| {
            (
                StatusCode::CONFLICT,
                Json(ErrorBody {
                    code: "SESSION_ABORT_COMPACT_FAILED".into(),
                    message: e.to_string(),
                    details: None,
                    request_id: None,
                }),
            )
        })?;

    Ok(Json(response))
}
