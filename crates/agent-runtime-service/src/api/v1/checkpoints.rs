use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use crate::db::Database;
use crate::models::v1::checkpoint::Checkpoint;
use crate::models::v1::common::{ListQuery, ListResponse, Pagination, ErrorBody};
use crate::service::ServiceContainer;
use llm::OpenAiClient;
use std::sync::Arc;
use uuid::Uuid;

pub async fn list(
    State((_, _, _services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Query(_q): Query<ListQuery>,
) -> StatusCode {
    StatusCode::NOT_IMPLEMENTED
}

pub async fn get(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
) -> Result<Json<Checkpoint>, StatusCode> {
    match services.checkpoint.get(id).await {
        Ok(Some(cp)) => Ok(Json(cp)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn restore(
    State((_, _, _services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(_id): Path<Uuid>,
) -> StatusCode {
    StatusCode::NOT_IMPLEMENTED
}
