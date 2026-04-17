use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
};
use crate::db::Database;
use crate::models::v1::common::ListQuery;
use crate::service::ServiceContainer;
use llm::OpenAiClient;
use std::sync::Arc;
use uuid::Uuid;

pub async fn create_candidate(
    State((_, _, _services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
) -> StatusCode {
    StatusCode::NOT_IMPLEMENTED
}

pub async fn list_candidates(
    State((_, _, _services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Query(_q): Query<ListQuery>,
) -> StatusCode {
    StatusCode::NOT_IMPLEMENTED
}

pub async fn get_candidate(
    State((_, _, _services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(_id): Path<Uuid>,
) -> StatusCode {
    StatusCode::NOT_IMPLEMENTED
}

pub async fn approve_candidate(
    State((_, _, _services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(_id): Path<Uuid>,
) -> StatusCode {
    StatusCode::NOT_IMPLEMENTED
}

pub async fn reject_candidate(
    State((_, _, _services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(_id): Path<Uuid>,
) -> StatusCode {
    StatusCode::NOT_IMPLEMENTED
}

pub async fn list_entries(
    State((_, _, _services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Query(_q): Query<ListQuery>,
) -> StatusCode {
    StatusCode::NOT_IMPLEMENTED
}

pub async fn get_entry(
    State((_, _, _services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(_id): Path<Uuid>,
) -> StatusCode {
    StatusCode::NOT_IMPLEMENTED
}

pub async fn search(
    State((_, _, _services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
) -> StatusCode {
    StatusCode::NOT_IMPLEMENTED
}
