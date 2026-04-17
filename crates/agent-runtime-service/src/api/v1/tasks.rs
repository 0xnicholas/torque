use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use crate::db::Database;
use crate::models::v1::common::{ErrorBody, ListQuery, ListResponse, Pagination};
use crate::models::v1::task::Task;
use crate::service::ServiceContainer;
use llm::OpenAiClient;
use std::sync::Arc;
use uuid::Uuid;

pub async fn list(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Query(q): Query<ListQuery>,
) -> Result<Json<ListResponse<Task>>, (StatusCode, Json<ErrorBody>)> {
    let limit = q.limit.clamp(1, 100);
    let mut rows = services.task.list(limit + 1).await
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody { code: "DB_ERROR".into(), message: e.to_string(), details: None, request_id: None })
        ))?;
    let has_more = rows.len() > limit as usize;
    if has_more { rows.pop(); }
    let next_cursor = rows.last().map(|r| r.id.to_string());
    Ok(Json(ListResponse {
        data: rows,
        pagination: Pagination { next_cursor, prev_cursor: q.cursor, has_more },
    }))
}

pub async fn get(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
) -> Result<Json<Task>, StatusCode> {
    match services.task.get(id).await {
        Ok(Some(task)) => Ok(Json(task)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn cancel(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    match services.task.cancel(id).await {
        Ok(true) => Ok(StatusCode::NO_CONTENT),
        Ok(false) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn list_events(
    State((_, _, _services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(_id): Path<Uuid>,
) -> StatusCode {
    StatusCode::NOT_IMPLEMENTED
}

pub async fn list_approvals(
    State((_, _, _services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(_id): Path<Uuid>,
) -> StatusCode {
    StatusCode::NOT_IMPLEMENTED
}

pub async fn list_delegations(
    State((_, _, _services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(_id): Path<Uuid>,
) -> StatusCode {
    StatusCode::NOT_IMPLEMENTED
}
