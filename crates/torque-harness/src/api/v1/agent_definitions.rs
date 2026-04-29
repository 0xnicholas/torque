use crate::db::Database;
use crate::models::v1::agent_definition::{AgentDefinition, AgentDefinitionCreate};
use crate::models::v1::common::{ErrorBody, ListQuery, ListResponse, Pagination};
use crate::service::ServiceContainer;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use llm::LlmClient;
use std::sync::Arc;
use uuid::Uuid;

pub async fn create(
    State((_, _, services)): State<(Database, Arc<dyn LlmClient>, Arc<ServiceContainer>)>,
    Json(req): Json<AgentDefinitionCreate>,
) -> Result<(StatusCode, Json<AgentDefinition>), (StatusCode, Json<ErrorBody>)> {
    let def = services.agent_definition.create(req).await.map_err(|e| {
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
    Ok((StatusCode::CREATED, Json(def)))
}

pub async fn list(
    State((_, _, services)): State<(Database, Arc<dyn LlmClient>, Arc<ServiceContainer>)>,
    Query(q): Query<ListQuery>,
) -> Result<Json<ListResponse<AgentDefinition>>, (StatusCode, Json<ErrorBody>)> {
    let limit = q.limit.clamp(1, 100);
    let cursor = q.cursor.clone().and_then(|s| Uuid::parse_str(&s).ok());
    let mut rows = services
        .agent_definition
        .list(limit + 1, cursor, q.sort.as_deref())
        .await
        .map_err(ErrorBody::db_error)?;
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
    State((_, _, services)): State<(Database, Arc<dyn LlmClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
) -> Result<Json<AgentDefinition>, StatusCode> {
    match services.agent_definition.get(id).await {
        Ok(Some(def)) => Ok(Json(def)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn delete(
    State((_, _, services)): State<(Database, Arc<dyn LlmClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    match services.agent_definition.delete(id).await {
        Ok(true) => Ok(StatusCode::NO_CONTENT),
        Ok(false) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}
