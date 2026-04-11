use axum::Router;
use llm::OpenAiClient;
use std::sync::Arc;

use crate::api;
use crate::db::Database;

pub fn build_app(db: Database, llm: Arc<OpenAiClient>) -> Router {
    api::router(db, llm)
}
