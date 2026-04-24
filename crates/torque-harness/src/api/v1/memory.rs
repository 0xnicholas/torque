use crate::db::Database;
use crate::models::v1::common::{ErrorBody, ListQuery, ListResponse, Pagination};
use crate::models::v1::memory::{
    MemoryDecisionLog, MemoryWriteCandidateCreate, MemoryWriteCandidateStatus, MergeCandidateRequest,
    RejectCandidateRequest, SemanticSearchQuery, SemanticSearchResult,
};
use crate::service::ServiceContainer;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::sse::{Event, Sse},
    Json,
};
use chrono::{DateTime, Utc};
use futures::StreamExt;
use llm::OpenAiClient;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Serialize)]
pub struct CandidateListResponse {
    pub data: Vec<crate::models::v1::memory::MemoryWriteCandidate>,
    pub pagination: Pagination,
    pub stats: Option<CandidateStats>,
}

#[derive(Serialize)]
pub struct CandidateStats {
    pub total: i64,
    pub pending: i64,
    pub review_required: i64,
    pub auto_approved: i64,
    pub approved: i64,
    pub rejected: i64,
    pub merged: i64,
}

pub async fn create_candidate(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Json(req): Json<MemoryWriteCandidateCreate>,
) -> Result<
    (
        StatusCode,
        Json<crate::models::v1::memory::MemoryWriteCandidate>,
    ),
    (StatusCode, Json<ErrorBody>),
> {
    let content_json = serde_json::to_value(&req.content).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorBody {
                code: "INVALID_CONTENT".into(),
                message: e.to_string(),
                details: None,
                request_id: None,
            }),
        )
    })?;

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

    let created = services
        .memory
        .v1_create_candidate(&candidate)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    code: "DB_ERROR".into(),
                    message: e.to_string(),
                    details: None,
                    request_id: None,
                }),
            )
        })?;
    Ok((StatusCode::CREATED, Json(created)))
}

pub async fn list_candidates(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Query(q): Query<ListQuery>,
) -> Result<Json<CandidateListResponse>, (StatusCode, Json<ErrorBody>)> {
    let limit = q.limit.clamp(1, 100);
    let offset = q
        .cursor
        .as_ref()
        .and_then(|c| c.parse::<i64>().ok())
        .unwrap_or(0);

    let status = q.filter_status.as_ref().and_then(|s| match s.as_str() {
        "pending" => Some(MemoryWriteCandidateStatus::Pending),
        "review_required" => Some(MemoryWriteCandidateStatus::ReviewRequired),
        "approved" => Some(MemoryWriteCandidateStatus::Approved),
        "rejected" => Some(MemoryWriteCandidateStatus::Rejected),
        "auto_approved" => Some(MemoryWriteCandidateStatus::AutoApproved),
        "merged" => Some(MemoryWriteCandidateStatus::Merged),
        _ => None,
    });

    let rows = services
        .memory
        .v1_list_candidates(status, limit, offset)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    code: "DB_ERROR".into(),
                    message: e.to_string(),
                    details: None,
                    request_id: None,
                }),
            )
        })?;

    let stats = services
        .memory
        .v1_count_candidates_by_status(None)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    code: "DB_ERROR".into(),
                    message: e.to_string(),
                    details: None,
                    request_id: None,
                }),
            )
        })?;

    let stats = stats.map(|counts| {
        let mut total = 0i64;
        let mut pending = 0i64;
        let mut review_required = 0i64;
        let mut auto_approved = 0i64;
        let mut approved = 0i64;
        let mut rejected = 0i64;
        let mut merged = 0i64;

        for (status_str, count) in counts {
            total += count;
            match status_str.as_str() {
                "pending" => pending = count,
                "review_required" => review_required = count,
                "auto_approved" => auto_approved = count,
                "approved" => approved = count,
                "rejected" => rejected = count,
                "merged" => merged = count,
                _ => {}
            }
        }

        CandidateStats {
            total,
            pending,
            review_required,
            auto_approved,
            approved,
            rejected,
            merged,
        }
    });

    let has_more = rows.len() >= limit as usize;
    let next_cursor = if has_more {
        Some((offset + limit).to_string())
    } else {
        None
    };
    Ok(Json(CandidateListResponse {
        data: rows,
        pagination: Pagination {
            next_cursor,
            prev_cursor: q.cursor,
            has_more,
        },
        stats,
    }))
}

#[derive(Debug, Deserialize)]
pub struct DecisionListQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    pub cursor: Option<String>,
    pub agent_instance_id: Option<Uuid>,
    pub decision_type: Option<String>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
}

fn default_limit() -> i64 {
    20
}

pub async fn list_decisions(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Query(q): Query<DecisionListQuery>,
) -> Result<Json<ListResponse<MemoryDecisionLog>>, (StatusCode, Json<ErrorBody>)> {
    let limit = q.limit.clamp(1, 100);
    let offset = q
        .cursor
        .as_ref()
        .and_then(|c| c.parse::<i64>().ok())
        .unwrap_or(0);

    let rows = services
        .memory
        .list_decisions(
            q.agent_instance_id,
            q.decision_type.as_deref(),
            q.start_date,
            q.end_date,
            limit,
            offset,
        )
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    code: "DB_ERROR".into(),
                    message: e.to_string(),
                    details: None,
                    request_id: None,
                }),
            )
        })?;

    let has_more = rows.len() >= limit as usize;
    let next_cursor = if has_more {
        Some((offset + limit).to_string())
    } else {
        None
    };

    Ok(Json(ListResponse {
        data: rows,
        pagination: Pagination {
            next_cursor,
            prev_cursor: q.cursor,
            has_more,
        },
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
    let candidate = services.memory.v1_get_candidate(id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody {
                code: "DB_ERROR".into(),
                message: e.to_string(),
                details: None,
                request_id: None,
            }),
        )
    })?;

    let candidate = match candidate {
        Some(c) => c,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorBody {
                    code: "NOT_FOUND".into(),
                    message: "Candidate not found".into(),
                    details: None,
                    request_id: None,
                }),
            ))
        }
    };

    if candidate.status != MemoryWriteCandidateStatus::Pending
        && candidate.status != MemoryWriteCandidateStatus::ReviewRequired
        && candidate.status != MemoryWriteCandidateStatus::AutoApproved
    {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorBody {
                code: "ALREADY_PROCESSED".into(),
                message: format!(
                    "Candidate is already {}",
                    serde_json::to_string(&candidate.status).unwrap_or_default()
                ),
                details: None,
                request_id: None,
            }),
        ));
    }

    let content: crate::models::v1::memory::MemoryContent =
        serde_json::from_value(candidate.content.clone()).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorBody {
                    code: "INVALID_CONTENT".into(),
                    message: e.to_string(),
                    details: None,
                    request_id: None,
                }),
            )
        })?;

    let entry = services
        .memory
        .v1_create_entry(
            Some(candidate.agent_instance_id),
            candidate.team_instance_id,
            content.category,
            &content.key,
            content.value,
            Some(candidate.id),
        )
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    code: "DB_ERROR".into(),
                    message: e.to_string(),
                    details: None,
                    request_id: None,
                }),
            )
        })?;

    services
        .memory
        .v1_update_candidate_status(
            id,
            MemoryWriteCandidateStatus::Approved,
            Some("api".to_string()),
            Some(entry.id),
        )
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    code: "DB_ERROR".into(),
                    message: e.to_string(),
                    details: None,
                    request_id: None,
                }),
            )
        })?;

    let _ = services
        .notification_service
        .notify_candidate_approved(id)
        .await;

    Ok(Json(entry))
}

pub async fn reject_candidate(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
    Json(req): Json<RejectCandidateRequest>,
) -> Result<StatusCode, (StatusCode, Json<ErrorBody>)> {
    match services
        .memory
        .v1_update_candidate_status(
            id,
            MemoryWriteCandidateStatus::Rejected,
            Some("api".to_string()),
            None,
        )
        .await
    {
        Ok(Some(_)) => {
            let _ = services
                .notification_service
                .notify_candidate_rejected(id)
                .await;
            Ok(StatusCode::NO_CONTENT)
        }
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorBody {
                code: "NOT_FOUND".into(),
                message: "Candidate not found".into(),
                details: None,
                request_id: None,
            }),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody {
                code: "DB_ERROR".into(),
                message: e.to_string(),
                details: None,
                request_id: None,
            }),
        )),
    }
}

pub async fn merge_candidate(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
    Json(req): Json<MergeCandidateRequest>,
) -> Result<Json<crate::models::v1::memory::MemoryEntry>, (StatusCode, Json<ErrorBody>)> {
    let candidate = services.memory.v1_get_candidate(id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody {
                code: "DB_ERROR".into(),
                message: e.to_string(),
                details: None,
                request_id: None,
            }),
        )
    })?;

    let candidate = match candidate {
        Some(c) => c,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorBody {
                    code: "NOT_FOUND".into(),
                    message: "Candidate not found".into(),
                    details: None,
                    request_id: None,
                }),
            ))
        }
    };

    if candidate.status != MemoryWriteCandidateStatus::Pending
        && candidate.status != MemoryWriteCandidateStatus::ReviewRequired
    {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorBody {
                code: "ALREADY_PROCESSED".into(),
                message: format!(
                    "Candidate is already {}",
                    serde_json::to_string(&candidate.status).unwrap_or_default()
                ),
                details: None,
                request_id: None,
            }),
        ));
    }

    let target_entry = services
        .memory
        .v1_get_entry(req.target_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    code: "DB_ERROR".into(),
                    message: e.to_string(),
                    details: None,
                    request_id: None,
                }),
            )
        })?;

    let target_entry = match target_entry {
        Some(e) => e,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorBody {
                    code: "TARGET_NOT_FOUND".into(),
                    message: "Target memory entry not found".into(),
                    details: None,
                    request_id: None,
                }),
            ))
        }
    };

    let content: crate::models::v1::memory::MemoryContent =
        serde_json::from_value(candidate.content.clone()).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorBody {
                    code: "INVALID_CONTENT".into(),
                    message: e.to_string(),
                    details: None,
                    request_id: None,
                }),
            )
        })?;

    let merged_value = match req.strategy.as_str() {
        "append" => {
            let mut existing = target_entry.value.clone();
            if let Some(obj) = existing.as_object_mut() {
                let new_val = content.value.clone();
                if let Some(new_obj) = new_val.as_object() {
                    for (k, v) in new_obj {
                        obj.insert(k.clone(), v.clone());
                    }
                } else {
                    obj.insert("appended".to_string(), new_val);
                }
            } else {
                existing = serde_json::json!([existing, content.value]);
            }
            existing
        }
        "summarize" => {
            serde_json::json!({
                "original": target_entry.value,
                "merged": content.value,
                "strategy": "summarize"
            })
        }
        _ => {
            serde_json::json!({
                "original": target_entry.value,
                "merged": content.value
            })
        }
    };

    let entry = services
        .memory
        .v1_create_entry(
            target_entry.agent_instance_id,
            target_entry.team_instance_id,
            target_entry.category,
            &target_entry.key,
            merged_value,
            Some(candidate.id),
        )
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    code: "DB_ERROR".into(),
                    message: e.to_string(),
                    details: None,
                    request_id: None,
                }),
            )
        })?;

    services
        .memory
        .v1_update_candidate_status(
            id,
            MemoryWriteCandidateStatus::Merged,
            Some("api".to_string()),
            Some(entry.id),
        )
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    code: "DB_ERROR".into(),
                    message: e.to_string(),
                    details: None,
                    request_id: None,
                }),
            )
        })?;

    let _ = services
        .notification_service
        .notify_candidate_merged(entry.id)
        .await;

    Ok(Json(entry))
}

pub async fn list_entries(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Query(q): Query<ListQuery>,
) -> Result<Json<ListResponse<crate::models::v1::memory::MemoryEntry>>, (StatusCode, Json<ErrorBody>)>
{
    let limit = q.limit.clamp(1, 100);
    let offset = q
        .cursor
        .as_ref()
        .and_then(|c| c.parse::<i64>().ok())
        .unwrap_or(0);
    let rows = services
        .memory
        .v1_list_entries(limit, offset)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    code: "DB_ERROR".into(),
                    message: e.to_string(),
                    details: None,
                    request_id: None,
                }),
            )
        })?;
    let has_more = rows.len() >= limit as usize;
    let next_cursor = if has_more {
        Some((offset + limit).to_string())
    } else {
        None
    };
    Ok(Json(ListResponse {
        data: rows,
        pagination: Pagination {
            next_cursor,
            prev_cursor: q.cursor,
            has_more,
        },
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

    let rows = services
        .memory
        .v1_semantic_search(&query)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    code: "SEARCH_ERROR".into(),
                    message: e.to_string(),
                    details: None,
                    request_id: None,
                }),
            )
        })?;

    Ok(Json(ListResponse {
        data: rows,
        pagination: Pagination {
            next_cursor: None,
            prev_cursor: None,
            has_more: false,
        },
    }))
}

pub async fn force_write(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Json(req): Json<crate::models::v1::memory::MemoryContent>,
) -> Result<(StatusCode, Json<crate::models::v1::memory::MemoryEntry>), (StatusCode, Json<ErrorBody>)>
{
    let entry = services
        .memory
        .v1_create_entry(None, None, req.category, &req.key, req.value, None)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    code: "DB_ERROR".into(),
                    message: e.to_string(),
                    details: None,
                    request_id: None,
                }),
            )
        })?;

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

    let entries = services
        .memory
        .get_entries_without_embedding(batch_size)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    code: "DB_ERROR".into(),
                    message: e.to_string(),
                    details: None,
                    request_id: None,
                }),
            )
        })?;

    let mut processed = 0;
    let mut failed = 0;

    for entry in entries {
        match services
            .memory
            .backfill_embedding(entry.id, entry.category.clone(), &entry.key, &entry.value)
            .await
        {
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

pub async fn review_notifications_sse(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
) -> Result<
    Sse<impl tokio_stream::Stream<Item = Result<Event, std::convert::Infallible>>>,
    StatusCode,
> {
    let notification_service = services.notification_service.as_ref();

    let mut receiver = match notification_service.subscribe() {
        Some(rx) => rx,
        None => return Err(StatusCode::NOT_FOUND),
    };

    let stream = async_stream::stream! {
        loop {
            let result = receiver.recv().await;
            match result {
                Ok(event) => {
                    let data = match &event {
                        crate::notification::ReviewEvent::CandidateCreated(c) => {
                            serde_json::json!({"type": "created", "id": c.id.to_string()})
                        }
                        crate::notification::ReviewEvent::CandidateNeedsReview(c) => {
                            serde_json::json!({"type": "review", "id": c.id.to_string()})
                        }
                        crate::notification::ReviewEvent::CandidateApproved(id) => {
                            serde_json::json!({"type": "approved", "id": id.to_string()})
                        }
                        crate::notification::ReviewEvent::CandidateRejected(id) => {
                            serde_json::json!({"type": "rejected", "id": id.to_string()})
                        }
                        crate::notification::ReviewEvent::CandidateMerged(id) => {
                            serde_json::json!({"type": "merged", "id": id.to_string()})
                        }
                    };
                    yield Ok::<_, std::convert::Infallible>(Event::default().event("memory-review").data(data.to_string()));
                }
                Err(_) => {
                    break;
                }
            }
        }
    };

    Ok(Sse::new(stream))
}
