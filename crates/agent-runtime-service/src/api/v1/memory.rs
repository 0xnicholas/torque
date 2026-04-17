use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use crate::db::Database;
use crate::models::v1::common::{ErrorBody, ListQuery, ListResponse, Pagination};
use crate::models::{MemoryCandidate, MemoryCandidateStatus, MemoryEntry};
use crate::service::ServiceContainer;
use llm::OpenAiClient;
use std::sync::Arc;
use uuid::Uuid;

const DEFAULT_PROJECT_SCOPE: &str = "default";

pub async fn create_candidate(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Json(req): Json<crate::models::v1::memory::MemoryWriteCandidateCreate>,
) -> Result<(StatusCode, Json<MemoryCandidate>), (StatusCode, Json<ErrorBody>)> {
    let content_str = serde_json::to_string(&req.content)
        .map_err(|e| (
            StatusCode::BAD_REQUEST,
            Json(ErrorBody { code: "INVALID_CONTENT".into(), message: e.to_string(), details: None, request_id: None })
        ))?;
    let candidate = MemoryCandidate::new(
        DEFAULT_PROJECT_SCOPE.to_string(),
        crate::models::MemoryLayer::L0,
        content_str,
    );
    let created = services.memory.create_candidate(&candidate
    ).await.map_err(|e| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorBody { code: "DB_ERROR".into(), message: e.to_string(), details: None, request_id: None })
    ))?;
    Ok((StatusCode::CREATED, Json(created)))
}

pub async fn list_candidates(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Query(q): Query<ListQuery>,
) -> Result<Json<ListResponse<MemoryCandidate>>, (StatusCode, Json<ErrorBody>)> {
    let limit = q.limit.clamp(1, 100);
    let offset = q.cursor.as_ref().and_then(|c| c.parse::<i64>().ok()).unwrap_or(0);
    let rows = services.memory.list_candidates(DEFAULT_PROJECT_SCOPE, limit, offset).await
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody { code: "DB_ERROR".into(), message: e.to_string(), details: None, request_id: None })
        ))?;
    let has_more = rows.len() >= limit as usize;
    let next_cursor = if has_more {
        Some((offset + limit).to_string())
    } else {
        None
    };
    Ok(Json(ListResponse {
        data: rows,
        pagination: Pagination { next_cursor, prev_cursor: q.cursor, has_more },
    }))
}

pub async fn get_candidate(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
) -> Result<Json<MemoryCandidate>, StatusCode> {
    match services.memory.get_candidate_by_id(DEFAULT_PROJECT_SCOPE, id).await {
        Ok(Some(candidate)) => Ok(Json(candidate)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn approve_candidate(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
) -> Result<Json<MemoryEntry>, (StatusCode, Json<ErrorBody>)> {
    let result = services.memory.accept_candidate(DEFAULT_PROJECT_SCOPE, id).await
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody { code: "DB_ERROR".into(), message: e.to_string(), details: None, request_id: None })
        ))?;
    match result {
        Some((_, entry)) => Ok(Json(entry)),
        None => Err((StatusCode::NOT_FOUND, Json(ErrorBody {
            code: "NOT_FOUND".into(),
            message: "Candidate not found".into(),
            details: None,
            request_id: None,
        }))),
    }
}

pub async fn reject_candidate(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    match services.memory.update_candidate_status(DEFAULT_PROJECT_SCOPE, id, MemoryCandidateStatus::Rejected).await {
        Ok(Some(_)) => Ok(StatusCode::NO_CONTENT),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn list_entries(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Query(q): Query<ListQuery>,
) -> Result<Json<ListResponse<MemoryEntry>>, (StatusCode, Json<ErrorBody>)> {
    let limit = q.limit.clamp(1, 100);
    let offset = q.cursor.as_ref().and_then(|c| c.parse::<i64>().ok()).unwrap_or(0);
    let rows = services.memory.list_entries(DEFAULT_PROJECT_SCOPE, limit, offset).await
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody { code: "DB_ERROR".into(), message: e.to_string(), details: None, request_id: None })
        ))?;
    let has_more = rows.len() >= limit as usize;
    let next_cursor = if has_more {
        Some((offset + limit).to_string())
    } else {
        None
    };
    Ok(Json(ListResponse {
        data: rows,
        pagination: Pagination { next_cursor, prev_cursor: q.cursor, has_more },
    }))
}

pub async fn get_entry(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
) -> Result<Json<MemoryEntry>, StatusCode> {
    match services.memory.get_entry_by_id(DEFAULT_PROJECT_SCOPE, id).await {
        Ok(Some(entry)) => Ok(Json(entry)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn search(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Query(q): Query<ListQuery>,
) -> Result<Json<ListResponse<MemoryEntry>>, (StatusCode, Json<ErrorBody>)> {
    let limit = q.limit.clamp(1, 100);
    let query_str = q.cursor.as_deref().unwrap_or("");
    let rows = services.memory.search_entries(DEFAULT_PROJECT_SCOPE, query_str, limit).await
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody { code: "DB_ERROR".into(), message: e.to_string(), details: None, request_id: None })
        ))?;
    Ok(Json(ListResponse {
        data: rows,
        pagination: Pagination { next_cursor: None, prev_cursor: None, has_more: false },
    }))
}
