use axum::{extract::State, http::StatusCode, Json};
use crate::db::Database;
use crate::service::ServiceContainer;
use llm::OpenAiClient;
use std::sync::Arc;

pub async fn create(
    State((_, _, _services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
) -> StatusCode {
    StatusCode::NOT_IMPLEMENTED
}

pub async fn list(
    State((_, _, _services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
) -> StatusCode {
    StatusCode::NOT_IMPLEMENTED
}

pub async fn get(
    State((_, _, _services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
) -> StatusCode {
    StatusCode::NOT_IMPLEMENTED
}

pub async fn delete(
    State((_, _, _services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
) -> StatusCode {
    StatusCode::NOT_IMPLEMENTED
}
