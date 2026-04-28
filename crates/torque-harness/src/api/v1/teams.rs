use crate::db::Database;
use crate::models::v1::common::{ErrorBody, ListQuery, ListResponse, Pagination};
use crate::models::v1::team::{
    PublishScope, TeamDefinition, TeamDefinitionCreate, TeamInstance, TeamInstanceCreate,
    TeamMember, TeamTask, TeamTaskCreate,
};
use crate::service::ServiceContainer;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use llm::OpenAiClient;
use std::sync::Arc;
use uuid::Uuid;

pub async fn create_definition(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Json(req): Json<TeamDefinitionCreate>,
) -> Result<(StatusCode, Json<TeamDefinition>), (StatusCode, Json<ErrorBody>)> {
    let def = services.team.create_definition(req).await.map_err(|e| {
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

pub async fn list_definitions(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Query(q): Query<ListQuery>,
) -> Result<Json<ListResponse<TeamDefinition>>, (StatusCode, Json<ErrorBody>)> {
    let limit = q.limit.clamp(1, 100);
    let mut rows = services
        .team
        .list_definitions(limit + 1)
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

pub async fn get_definition(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
) -> Result<Json<TeamDefinition>, StatusCode> {
    match services.team.get_definition(id).await {
        Ok(Some(def)) => Ok(Json(def)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn delete_definition(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    match services.team.delete_definition(id).await {
        Ok(true) => Ok(StatusCode::NO_CONTENT),
        Ok(false) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn create_instance(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Json(req): Json<TeamInstanceCreate>,
) -> Result<(StatusCode, Json<TeamInstance>), (StatusCode, Json<ErrorBody>)> {
    let instance = services.team.create_instance(req).await.map_err(|e| {
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

pub async fn list_instances(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Query(q): Query<ListQuery>,
) -> Result<Json<ListResponse<TeamInstance>>, (StatusCode, Json<ErrorBody>)> {
    let limit = q.limit.clamp(1, 100);
    let mut rows = services.team.list_instances(limit + 1).await.map_err(|e| {
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

pub async fn get_instance(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
) -> Result<Json<TeamInstance>, StatusCode> {
    match services.team.get_instance(id).await {
        Ok(Some(instance)) => Ok(Json(instance)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn delete_instance(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    match services.team.delete_instance(id).await {
        Ok(true) => Ok(StatusCode::NO_CONTENT),
        Ok(false) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn create_task(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
    Json(req): Json<TeamTaskCreate>,
) -> Result<(StatusCode, Json<TeamTask>), (StatusCode, Json<ErrorBody>)> {
    let task = services
        .team
        .create_team_task(id, &req)
        .await
        .map_err(ErrorBody::db_error)?;
    Ok((StatusCode::ACCEPTED, Json(task)))
}

pub async fn list_tasks(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
    Query(q): Query<ListQuery>,
) -> Result<Json<ListResponse<TeamTask>>, (StatusCode, Json<ErrorBody>)> {
    let limit = q.limit.clamp(1, 100);
    let mut rows = services
        .team
        .list_team_tasks(id, limit + 1)
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

pub async fn list_members(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
    Query(q): Query<ListQuery>,
) -> Result<Json<ListResponse<TeamMember>>, (StatusCode, Json<ErrorBody>)> {
    let limit = q.limit.clamp(1, 100);
    let mut rows = services
        .team
        .list_members(id, limit + 1)
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

pub async fn publish(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
    Json(body): Json<PublishRequest>,
) -> Result<StatusCode, (StatusCode, Json<ErrorBody>)> {
    let _instance = services
        .team
        .get_instance(id)
        .await
        .map_err(ErrorBody::db_error)?
        .ok_or(ErrorBody::not_found("Team instance not found"))?;

    let scope = body
        .scope
        .as_deref()
        .map(|s| match s {
            "private" => PublishScope::Private,
            "team_shared" => PublishScope::TeamShared,
            "external_published" => PublishScope::ExternalPublished,
            _ => PublishScope::TeamShared,
        })
        .unwrap_or(PublishScope::TeamShared);

    let published = services
        .team
        .publish_artifact(id, body.artifact_id, scope, "api")
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    code: "PUBLISH_ERROR".into(),
                    message: e.to_string(),
                    details: None,
                    request_id: None,
                }),
            )
        })?;

    if published {
        Ok(StatusCode::OK)
    } else {
        Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody {
                code: "PUBLISH_FAILED".into(),
                message: "Failed to publish artifact".into(),
                details: None,
                request_id: None,
            }),
        ))
    }
}

#[derive(serde::Deserialize)]
pub struct PublishRequest {
    pub artifact_id: Uuid,
    pub scope: Option<String>,
}

#[derive(serde::Serialize)]
pub struct SupervisorExecutionResponse {
    executed: bool,
    task_id: Option<Uuid>,
    success: bool,
    summary: String,
}

pub async fn execute_supervisor(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
) -> Result<Json<SupervisorExecutionResponse>, (StatusCode, Json<ErrorBody>)> {
    match services.team_supervisor.poll_and_execute(id).await {
        Ok(Some(result)) => Ok(Json(SupervisorExecutionResponse {
            executed: true,
            task_id: Some(result.task_id),
            success: result.success,
            summary: result.summary,
        })),
        Ok(None) => Ok(Json(SupervisorExecutionResponse {
            executed: false,
            task_id: None,
            success: true,
            summary: "No open tasks to execute".to_string(),
        })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody {
                code: "SUPERVISOR_ERROR".into(),
                message: e.to_string(),
                details: None,
                request_id: None,
            }),
        )),
    }
}
