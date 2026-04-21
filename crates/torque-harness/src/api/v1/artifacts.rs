use crate::db::Database;
use crate::models::v1::artifact::{Artifact, ArtifactCreate, ArtifactScope};
use crate::models::v1::common::{ErrorBody, ListQuery, ListResponse, Pagination};
use crate::service::ServiceContainer;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use llm::OpenAiClient;
use std::sync::Arc;
use uuid::Uuid;

pub async fn create(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Json(req): Json<ArtifactCreate>,
) -> Result<(StatusCode, Json<Artifact>), (StatusCode, Json<ErrorBody>)> {
    let artifact = services
        .artifact
        .create(&req.kind, req.scope, &req.mime_type, req.content)
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
    Ok((StatusCode::CREATED, Json(artifact)))
}

pub async fn list(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Query(q): Query<ListQuery>,
) -> Result<Json<ListResponse<Artifact>>, (StatusCode, Json<ErrorBody>)> {
    let limit = q.limit.clamp(1, 100);
    let mut rows = services.artifact.list(limit + 1).await.map_err(|e| {
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
) -> Result<Json<Artifact>, StatusCode> {
    match services.artifact.get(id).await {
        Ok(Some(artifact)) => Ok(Json(artifact)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn delete(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    match services.artifact.delete(id).await {
        Ok(true) => Ok(StatusCode::NO_CONTENT),
        Ok(false) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn get_content(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match services.artifact.get(id).await {
        Ok(Some(artifact)) => Ok(Json(artifact.content)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn publish(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    match services
        .artifact
        .update_scope(id, ArtifactScope::TeamShared)
        .await
    {
        Ok(true) => Ok(StatusCode::NO_CONTENT),
        Ok(false) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}
