use axum::{Router, routing::{get, post, delete}};
use crate::db::Database;
use crate::service::ServiceContainer;
use llm::OpenAiClient;
use std::sync::Arc;

pub mod agent_definitions;
pub mod agent_instances;
pub mod runs;
pub mod tasks;
pub mod artifacts;
pub mod memory;
pub mod capabilities;
pub mod teams;
pub mod delegations;
pub mod approvals;
pub mod checkpoints;
pub mod events;

pub fn router() -> Router<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)> {
    Router::new()
        .route("/v1/agent-definitions", post(agent_definitions::create).get(agent_definitions::list))
        .route("/v1/agent-definitions/:id", get(agent_definitions::get).delete(agent_definitions::delete))
}
