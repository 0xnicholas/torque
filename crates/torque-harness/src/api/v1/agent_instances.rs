use crate::db::Database;
use crate::models::v1::agent_instance::{
    AgentInstance, AgentInstanceCreate, AgentInstanceStatus, TimeTravelRequest,
};
use crate::models::v1::artifact::Artifact;
use crate::models::v1::checkpoint::Checkpoint;
use crate::models::v1::common::{ErrorBody, ListQuery, ListResponse, Pagination};
use crate::models::v1::delegation::Delegation;
use crate::models::v1::event::Event;
use crate::service::ServiceContainer;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use checkpointer::r#trait::Message;
use llm::OpenAiClient;
use std::sync::Arc;
use uuid::Uuid;

#[derive(serde::Serialize)]
pub struct ResumeResponse {
    pub instance: AgentInstance,
    pub messages: Vec<Message>,
}

pub async fn create(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Json(req): Json<AgentInstanceCreate>,
) -> Result<(StatusCode, Json<AgentInstance>), (StatusCode, Json<ErrorBody>)> {
    let instance = services.agent_instance.create(req).await.map_err(|e| {
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
    Ok((StatusCode::CREATED, Json(instance)))
}

pub async fn list(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Query(q): Query<ListQuery>,
) -> Result<Json<ListResponse<AgentInstance>>, (StatusCode, Json<ErrorBody>)> {
    let limit = q.limit.clamp(1, 100);
    let mut rows = services.agent_instance.list(limit + 1).await.map_err(|e| {
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
    let has_more = rows.len() > limit as usize;
    if has_more {
        rows.pop();
    }
    let next_cursor = rows.last().map(|r| r.id.to_string());
    Ok(Json(ListResponse {
        data: rows,
        pagination: Pagination {
            next_cursor,
            prev_cursor: q.cursor,
            has_more,
        },
    }))
}

pub async fn get(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
) -> Result<Json<AgentInstance>, StatusCode> {
    match services.agent_instance.get(id).await {
        Ok(Some(instance)) => Ok(Json(instance)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn delete(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    match services.agent_instance.delete(id).await {
        Ok(true) => Ok(StatusCode::NO_CONTENT),
        Ok(false) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn cancel(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    match services
        .agent_instance
        .update_status(id, AgentInstanceStatus::Cancelled)
        .await
    {
        Ok(true) => Ok(StatusCode::NO_CONTENT),
        Ok(false) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn resume(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
) -> Result<Json<ResumeResponse>, (StatusCode, Json<ErrorBody>)> {
    let (instance, messages) = services.recovery.resume_instance(id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody {
                code: "RESUME_ERROR".into(),
                message: e.to_string(),
                details: None,
                request_id: None,
            }),
        )
    })?;
    Ok(Json(ResumeResponse { instance, messages }))
}

pub async fn time_travel(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
    Json(req): Json<TimeTravelRequest>,
) -> Result<Json<AgentInstance>, (StatusCode, Json<ErrorBody>)> {
    let instance = services
        .recovery
        .time_travel(id, req.checkpoint_id, req.branch_name)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    code: "TIME_TRAVEL_ERROR".into(),
                    message: e.to_string(),
                    details: None,
                    request_id: None,
                }),
            )
        })?;
    Ok(Json(instance))
}

pub async fn list_delegations(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
    Query(q): Query<ListQuery>,
) -> Result<Json<ListResponse<Delegation>>, (StatusCode, Json<ErrorBody>)> {
    let limit = q.limit.clamp(1, 100);
    let mut rows = services
        .delegation
        .list_by_instance(id, limit + 1)
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
    let has_more = rows.len() > limit as usize;
    if has_more {
        rows.pop();
    }
    let next_cursor = rows.last().map(|r| r.id.to_string());
    Ok(Json(ListResponse {
        data: rows,
        pagination: Pagination {
            next_cursor,
            prev_cursor: q.cursor,
            has_more,
        },
    }))
}

pub async fn list_artifacts(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
    Query(q): Query<ListQuery>,
) -> Result<Json<ListResponse<Artifact>>, (StatusCode, Json<ErrorBody>)> {
    let limit = q.limit.clamp(1, 100);
    let mut rows = services
        .artifact
        .list_by_instance(id, limit + 1)
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
    let has_more = rows.len() > limit as usize;
    if has_more {
        rows.pop();
    }
    let next_cursor = rows.last().map(|r| r.id.to_string());
    Ok(Json(ListResponse {
        data: rows,
        pagination: Pagination {
            next_cursor,
            prev_cursor: q.cursor,
            has_more,
        },
    }))
}

pub async fn list_events(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
    Query(q): Query<ListQuery>,
) -> Result<Json<ListResponse<Event>>, (StatusCode, Json<ErrorBody>)> {
    let limit = q.limit.clamp(1, 100);
    let rows = services
        .event
        .list_by_resource("agent_instance", id, &[], limit)
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
    Ok(Json(ListResponse {
        data: rows,
        pagination: Pagination {
            next_cursor: None,
            prev_cursor: None,
            has_more: false,
        },
    }))
}

pub async fn list_checkpoints(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
    Query(q): Query<ListQuery>,
) -> Result<Json<ListResponse<Checkpoint>>, (StatusCode, Json<ErrorBody>)> {
    let limit = q.limit.clamp(1, 100);
    let mut rows = services
        .checkpoint
        .list_by_instance(id, limit + 1)
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
    let has_more = rows.len() > limit as usize;
    if has_more {
        rows.pop();
    }
    let next_cursor = rows.last().map(|r| r.id.to_string());
    Ok(Json(ListResponse {
        data: rows,
        pagination: Pagination {
            next_cursor,
            prev_cursor: q.cursor,
            has_more,
        },
    }))
}
