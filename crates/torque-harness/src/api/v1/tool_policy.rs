use crate::db::Database;
use crate::models::v1::common::{ErrorBody, ListResponse, Pagination};
use crate::models::v1::tool_policy::ToolPolicy;
use crate::service::ServiceContainer;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use llm::OpenAiClient;
use std::sync::Arc;

pub async fn list(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
) -> Result<Json<ListResponse<ToolPolicy>>, (StatusCode, Json<ErrorBody>)> {
    let data = services
        .tool_policy
        .list()
        .await
        .map_err(ErrorBody::db_error)?;

    Ok(Json(ListResponse {
        data,
        pagination: Pagination {
            next_cursor: None,
            prev_cursor: None,
            has_more: false,
        },
    }))
}

pub async fn get(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(tool_name): Path<String>,
) -> Result<Json<ToolPolicy>, StatusCode> {
    services
        .tool_policy
        .get(&tool_name)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

pub async fn upsert(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(tool_name): Path<String>,
    Json(policy): Json<ToolPolicy>,
) -> Result<StatusCode, StatusCode> {
    if policy.tool_name != tool_name {
        return Err(StatusCode::BAD_REQUEST);
    }
    let is_insert = services
        .tool_policy
        .upsert(&policy)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(if is_insert {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    })
}

pub async fn delete(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(tool_name): Path<String>,
) -> Result<StatusCode, StatusCode> {
    services
        .tool_policy
        .delete(&tool_name)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}
