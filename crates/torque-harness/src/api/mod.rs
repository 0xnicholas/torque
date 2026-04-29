use crate::db::Database;
use crate::service::ServiceContainer;
use axum::Router;
use llm::LlmClient;
use std::sync::Arc;

pub mod middleware;
pub mod v1;

pub fn router(db: Database, llm: Arc<dyn LlmClient>, services: Arc<ServiceContainer>) -> Router {
    use crate::api::middleware::auth_middleware;
    use axum::middleware;

    let v1_router = v1::router();

    Router::new()
        .nest("/", v1_router)
        .layer(middleware::from_fn(auth_middleware))
        .with_state((db, llm, services))
}
