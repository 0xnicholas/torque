use axum::{Router, routing::{get, post}};
use crate::db::Database;
use llm::OpenAiClient;
use std::sync::Arc;

pub mod middleware;
pub mod memory;
pub mod metrics;
pub mod sessions;
pub mod messages;

pub fn router(
    db: Database,
    llm: Arc<OpenAiClient>,
) -> Router {
    use axum::middleware;
    use crate::api::middleware::auth_middleware;

    Router::new()
        .route("/sessions", post(sessions::create).get(sessions::list))
        .route("/sessions/:id", get(sessions::get))
        .route("/sessions/:id/messages", get(messages::list))
        .route("/sessions/:id/chat", post(messages::chat))
        .route("/sessions/:id/memory/candidates", post(memory::create_candidate))
        .route(
            "/sessions/:id/memory/candidates/:candidate_id/accept",
            post(memory::accept_candidate),
        )
        .route("/sessions/:id/memory", get(memory::list_entries))
        .route("/metrics", get(metrics::get))
        .layer(middleware::from_fn(auth_middleware))
        .with_state((db, llm))
}
