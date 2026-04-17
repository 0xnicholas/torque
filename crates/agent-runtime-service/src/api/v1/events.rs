use axum::{
    extract::{Query, State},
    http::StatusCode,
};
use crate::db::Database;
use crate::models::v1::common::{ListQuery};
use crate::service::ServiceContainer;
use llm::OpenAiClient;
use std::sync::Arc;

pub async fn list(
    State((_, _, _services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Query(_q): Query<ListQuery>,
) -> StatusCode {
    StatusCode::NOT_IMPLEMENTED
}
