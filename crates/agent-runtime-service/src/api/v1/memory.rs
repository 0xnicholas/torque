use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use crate::db::Database;
use crate::models::v1::common::{ErrorBody, ListQuery, ListResponse, Pagination};
use crate::models::v1::memory::{
    MemoryWriteCandidateCreate, MemoryWriteCandidateStatus, SemanticSearchQuery,
    SemanticSearchResult,
};
use crate::service::ServiceContainer;
use llm::OpenAiClient;
use std::sync::Arc;
use uuid::Uuid;

pub async fn create_candidate(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Json(req): Json<MemoryWriteCandidateCreate>,
) -> Result<(StatusCode, Json<crate::models::v1::memory::MemoryWriteCandidate>), (StatusCode, Json<ErrorBody>)> {
    let content_json = serde_json::to_value(&req.content)
        .map_err(|e| (
            StatusCode::BAD_REQUEST,
            Json(ErrorBody { code: "INVALID_CONTENT".into(), message: e.to_string(), details: None, request_id: None })
        ))?;
    
    let candidate = crate::models::v1::memory::MemoryWriteCandidate {
        id: Uuid::new_v4(),
        agent_instance_id: req.agent_instance_id,
        team_instance_id: req.team_instance_id,
        content: content_json,
        reasoning: req.reasoning,
        status: MemoryWriteCandidateStatus::Pending,
        memory_entry_id: None,
        reviewed_by: None,
        created_at: chrono::Utc::now(),
        reviewed_at: None,
        updated_at: chrono::Utc::now(),
    };
    
    let created = services.memory.v1_create_candidate(&candidate
    ).await.map_err(|e| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorBody { code: "DB_ERROR".into(), message: e.to_string(), details: None, request_id: None })
    ))?;
    Ok((StatusCode::CREATED, Json(created)))
}

pub async fn list_candidates(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Query(q): Query<ListQuery>,
) -> Result<Json<ListResponse<crate::models::v1::memory::MemoryWriteCandidate>>, (StatusCode, Json<ErrorBody>)> {
    let limit = q.limit.clamp(1, 100);
    let offset = q.cursor.as_ref().and_then(|c| c.parse::<i64>().ok()).unwrap_or(0);
    
    let status = q.cursor.as_ref()
        .and_then(|c| {
            if c.starts_with("status:") {
                match c.strip_prefix("status:") {
                    Some("pending") => Some(MemoryWriteCandidateStatus::Pending),
                    Some("review_required") => Some(MemoryWriteCandidateStatus::ReviewRequired),
                    Some("approved") => Some(MemoryWriteCandidateStatus::Approved),
                    Some("rejected") => Some(MemoryWriteCandidateStatus::Rejected),
                    _ => None,
                }
            } else {
                None
            }
        });
    
    let rows = services.memory.v1_list_candidates(status, limit, offset).await
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
) -> Result<Json<crate::models::v1::memory::MemoryWriteCandidate>, StatusCode> {
    match services.memory.v1_get_candidate(id).await {
        Ok(Some(candidate)) => Ok(Json(candidate)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn approve_candidate(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
) -> Result<Json<crate::models::v1::memory::MemoryEntry>, (StatusCode, Json<ErrorBody>)> {
    let candidate = services.memory.v1_get_candidate(id).await
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody { code: "DB_ERROR".into(), message: e.to_string(), details: None, request_id: None })
        ))?;
    
    let candidate = match candidate {
        Some(c) => c,
        None => return Err((StatusCode::NOT_FOUND, Json(ErrorBody {
            code: "NOT_FOUND".into(),
            message: "Candidate not found".into(),
            details: None,
            request_id: None,
        }))),
    };
    
    let content: crate::models::v1::memory::MemoryContent = serde_json::from_value(
        candidate.content.clone()
    ).map_err(|e| (
        StatusCode::BAD_REQUEST,
        Json(ErrorBody { code: "INVALID_CONTENT".into(), message: e.to_string(), details: None, request_id: None })
    ))?;
    
    let entry = services.memory.v1_create_entry(
        Some(candidate.agent_instance_id),
        candidate.team_instance_id,
        content.category,
        &content.key,
        content.value,
        Some(candidate.id),
    ).await.map_err(|e| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorBody { code: "DB_ERROR".into(), message: e.to_string(), details: None, request_id: None })
    ))?;
    
    services.memory.v1_update_candidate_status(
        id,
        MemoryWriteCandidateStatus::Approved,
        Some("api".to_string()),
        Some(entry.id),
    ).await.map_err(|e| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorBody { code: "DB_ERROR".into(), message: e.to_string(), details: None, request_id: None })
    ))?;
    
    Ok(Json(entry))
}

pub async fn reject_candidate(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    match services.memory.v1_update_candidate_status(
        id, MemoryWriteCandidateStatus::Rejected, Some("api".to_string()), None
    ).await {
        Ok(Some(_)) => Ok(StatusCode::NO_CONTENT),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn list_entries(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Query(q): Query<ListQuery>,
) -> Result<Json<ListResponse<crate::models::v1::memory::MemoryEntry>>, (StatusCode, Json<ErrorBody>)> {
    let limit = q.limit.clamp(1, 100);
    let offset = q.cursor.as_ref().and_then(|c| c.parse::<i64>().ok()).unwrap_or(0);
    let rows = services.memory.v1_list_entries(limit, offset).await
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
) -> Result<Json<crate::models::v1::memory::MemoryEntry>, StatusCode> {
    match services.memory.v1_get_entry(id).await {
        Ok(Some(entry)) => Ok(Json(entry)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn search(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Json(req): Json<SemanticSearchQuery>,
) -> Result<Json<ListResponse<SemanticSearchResult>>, (StatusCode, Json<ErrorBody>)> {
    let limit = req.limit.unwrap_or(10).clamp(1, 100);
    let query = SemanticSearchQuery {
        limit: Some(limit),
        ..req
    };
    
    let rows = services.memory.v1_semantic_search(&query).await
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody { code: "SEARCH_ERROR".into(), message: e.to_string(), details: None, request_id: None })
        ))?;
    
    Ok(Json(ListResponse {
        data: rows,
        pagination: Pagination { next_cursor: None, prev_cursor: None, has_more: false },
    }))
}

pub async fn force_write(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Json(req): Json<crate::models::v1::memory::MemoryContent>,
) -> Result<(StatusCode, Json<crate::models::v1::memory::MemoryEntry>), (StatusCode, Json<ErrorBody>)> {
    let entry = services.memory.v1_create_entry(
        None,
        None,
        req.category,
        &req.key,
        req.value,
        None,
    ).await.map_err(|e| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorBody { code: "DB_ERROR".into(), message: e.to_string(), details: None, request_id: None })
    ))?;
    
    Ok((StatusCode::CREATED, Json(entry)))
}

#[derive(Debug, serde::Deserialize)]
pub struct BackfillRequest {
    pub batch_size: Option<i64>,
}

#[derive(Debug, serde::Serialize)]
pub struct BackfillResponse {
    pub processed: usize,
    pub failed: usize,
}

pub async fn backfill(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Json(req): Json<BackfillRequest>,
) -> Result<Json<BackfillResponse>, (StatusCode, Json<ErrorBody>)> {
    let batch_size = req.batch_size.unwrap_or(100).clamp(1, 1000);
    
    let entries = services.memory.get_entries_without_embedding(batch_size).await
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody { code: "DB_ERROR".into(), message: e.to_string(), details: None, request_id: None })
        ))?;
    
    let mut processed = 0;
    let mut failed = 0;
    
    for entry in entries {
        match services.memory.backfill_embedding(
            entry.id,
            entry.category.clone(),
            &entry.key,
            &entry.value,
        ).await {
            Ok(Some(_)) => processed += 1,
            Ok(None) => failed += 1,
            Err(e) => {
                tracing::warn!("Failed to backfill embedding for entry {}: {}", entry.id, e);
                failed += 1;
            }
        }
    }
    
    Ok(Json(BackfillResponse { processed, failed }))
}
