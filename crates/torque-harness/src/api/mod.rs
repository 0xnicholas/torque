use crate::db::Database;
use crate::service::ServiceContainer;
use axum::{
    routing::{get, post},
    Router,
};
use llm::OpenAiClient;
use std::sync::Arc;

pub mod memory;
pub mod messages;
pub mod metrics;
pub mod middleware;
pub mod sessions;
pub mod v1;

pub fn router(db: Database, llm: Arc<OpenAiClient>, services: Arc<ServiceContainer>) -> Router {
    use crate::api::middleware::auth_middleware;
    use axum::middleware;

    let v1_router = v1::router();

    Router::new()
        .route("/sessions", post(sessions::create).get(sessions::list))
        .route("/sessions/:id", get(sessions::get))
        .route("/sessions/:id/messages", get(messages::list))
        .route("/sessions/:id/chat", post(messages::chat))
        .route(
            "/sessions/:id/memory/candidates",
            post(memory::create_candidate),
        )
        .route(
            "/sessions/:id/memory/candidates/:candidate_id/accept",
            post(memory::accept_candidate),
        )
        .route("/sessions/:id/memory", get(memory::list_entries))
        .route("/sessions/:id/memory/search", get(memory::search_entries))
        .route(
            "/sessions/:id/memory/:entry_id/replace",
            post(memory::replace_entry),
        )
        .route(
            "/sessions/:id/memory/:entry_id/invalidate",
            post(memory::invalidate_entry),
        )
        .route("/metrics", get(metrics::get))
        .nest("/", v1_router)
        .layer(middleware::from_fn(auth_middleware))
        .with_state((db, llm, services))
}
