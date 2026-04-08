use axum::{Router, routing::{get, post}};
use crate::db::Database;
use llm::OpenAiClient;
use std::sync::Arc;

pub mod middleware;
pub mod sessions;
pub mod messages;

pub fn router(
    db: Database,
    llm: Arc<OpenAiClient>,
) -> Router {
    use axum::middleware;
    use crate::api::middleware::auth_middleware;

    Router::new()
        .route("/sessions", post(sessions::create))
        .route("/sessions/:id", get(sessions::get))
        .route("/sessions/:id/messages", get(messages::list))
        .route("/sessions/:id/chat", post(messages::chat))
        .layer(middleware::from_fn(auth_middleware))
        .with_state((db, llm))
}