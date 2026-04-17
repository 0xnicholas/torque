use axum::{Router, routing::{get, post}};
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
        .route("/v1/agent-instances", post(agent_instances::create).get(agent_instances::list))
        .route("/v1/agent-instances/:id", get(agent_instances::get).delete(agent_instances::delete))
        .route("/v1/agent-instances/:id/cancel", post(agent_instances::cancel))
        .route("/v1/agent-instances/:id/resume", post(agent_instances::resume))
        .route("/v1/agent-instances/:id/time-travel", post(agent_instances::time_travel))
        .route("/v1/agent-instances/:id/delegations", get(agent_instances::list_delegations))
        .route("/v1/agent-instances/:id/artifacts", get(agent_instances::list_artifacts))
        .route("/v1/agent-instances/:id/events", get(agent_instances::list_events))
        .route("/v1/agent-instances/:id/checkpoints", get(agent_instances::list_checkpoints))
        .route("/v1/agent-instances/:id/runs", post(runs::run))
}
