use crate::db::Database;
use crate::service::ServiceContainer;
use axum::{
    routing::{get, post},
    Router,
};
use llm::OpenAiClient;
use std::sync::Arc;

pub mod agent_definitions;
pub mod agent_instances;
pub mod approvals;
pub mod artifacts;
pub mod capabilities;
pub mod checkpoints;
pub mod delegations;
pub mod events;
pub mod escalations;
pub mod memory;
pub mod runs;
pub mod tasks;
pub mod teams;

pub fn router() -> Router<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)> {
    Router::new()
        .route(
            "/v1/agent-definitions",
            post(agent_definitions::create).get(agent_definitions::list),
        )
        .route(
            "/v1/agent-definitions/:id",
            get(agent_definitions::get).delete(agent_definitions::delete),
        )
        .route(
            "/v1/agent-instances",
            post(agent_instances::create).get(agent_instances::list),
        )
        .route(
            "/v1/agent-instances/:id",
            get(agent_instances::get).delete(agent_instances::delete),
        )
        .route(
            "/v1/agent-instances/:id/cancel",
            post(agent_instances::cancel),
        )
        .route(
            "/v1/agent-instances/:id/resume",
            post(agent_instances::resume),
        )
        .route(
            "/v1/agent-instances/:id/time-travel",
            post(agent_instances::time_travel),
        )
        .route(
            "/v1/agent-instances/:id/delegations",
            get(agent_instances::list_delegations),
        )
        .route(
            "/v1/agent-instances/:id/artifacts",
            get(agent_instances::list_artifacts),
        )
        .route(
            "/v1/agent-instances/:id/events",
            get(agent_instances::list_events),
        )
        .route(
            "/v1/agent-instances/:id/checkpoints",
            get(agent_instances::list_checkpoints),
        )
        .route("/v1/agent-instances/:id/runs", post(runs::run))
        .route("/v1/runs", post(runs::create))
        .route("/v1/tasks", get(tasks::list))
        .route("/v1/tasks/:id", get(tasks::get))
        .route("/v1/tasks/:id/cancel", post(tasks::cancel))
        .route("/v1/tasks/:id/events", get(tasks::list_events))
        .route("/v1/tasks/:id/approvals", get(tasks::list_approvals))
        .route("/v1/tasks/:id/delegations", get(tasks::list_delegations))
        .route(
            "/v1/artifacts",
            post(artifacts::create).get(artifacts::list),
        )
        .route(
            "/v1/artifacts/:id",
            get(artifacts::get).delete(artifacts::delete),
        )
        .route("/v1/artifacts/:id/content", get(artifacts::get_content))
        .route("/v1/artifacts/:id/publish", post(artifacts::publish))
        .route("/v1/events", get(events::list))
        .route(
            "/v1/capability-profiles",
            post(capabilities::create_profile).get(capabilities::list_profiles),
        )
        .route(
            "/v1/capability-profiles/:id",
            get(capabilities::get_profile).delete(capabilities::delete_profile),
        )
        .route(
            "/v1/capabilities/resolve",
            post(capabilities::resolve),
        )
        .route(
            "/v1/capability-registry-bindings",
            post(capabilities::create_binding).get(capabilities::list_bindings),
        )
        .route(
            "/v1/capability-registry-bindings/:id",
            get(capabilities::get_binding).delete(capabilities::delete_binding),
        )
        .route(
            "/v1/team-definitions",
            post(teams::create_definition).get(teams::list_definitions),
        )
        .route(
            "/v1/team-definitions/:id",
            get(teams::get_definition).delete(teams::delete_definition),
        )
        .route(
            "/v1/team-instances",
            post(teams::create_instance).get(teams::list_instances),
        )
        .route(
            "/v1/team-instances/:id",
            get(teams::get_instance).delete(teams::delete_instance),
        )
        .route(
            "/v1/team-instances/:id/tasks",
            get(teams::list_tasks).post(teams::create_task),
        )
        .route("/v1/team-instances/:id/members", get(teams::list_members))
        .route("/v1/team-instances/:id/publish", post(teams::publish))
        .route(
            "/v1/team-instances/:id/supervisor/execute",
            post(teams::execute_supervisor),
        )
        .route(
            "/v1/delegations",
            post(delegations::create).get(delegations::list),
        )
        .route("/v1/delegations/:id", get(delegations::get))
        .route("/v1/delegations/:id/accept", post(delegations::accept))
        .route("/v1/delegations/:id/reject", post(delegations::reject))
        .route("/v1/delegations/:id/complete", post(delegations::complete))
        .route("/v1/delegations/:id/fail", post(delegations::fail))
        .route("/v1/approvals", get(approvals::list))
        .route("/v1/approvals/:id", get(approvals::get))
        .route("/v1/approvals/:id/resolve", post(approvals::resolve))
        .route("/v1/checkpoints", get(checkpoints::list))
        .route("/v1/checkpoints/:id", get(checkpoints::get))
        .route("/v1/checkpoints/:id/restore", post(checkpoints::restore))
        .route("/v1/checkpoints/:id/messages", get(checkpoints::get_messages))
        .route(
            "/v1/memory-write-candidates",
            post(memory::create_candidate).get(memory::list_candidates),
        )
        .route(
            "/v1/memory-write-candidates/:id",
            get(memory::get_candidate),
        )
        .route(
            "/v1/memory-write-candidates/:id/approve",
            post(memory::approve_candidate),
        )
        .route(
            "/v1/memory-write-candidates/:id/reject",
            post(memory::reject_candidate),
        )
        .route(
            "/v1/memory-write-candidates/:id/merge",
            post(memory::merge_candidate),
        )
        .route("/v1/memory-entries", get(memory::list_entries))
        .route("/v1/memory-entries/:id", get(memory::get_entry))
        .route("/v1/memory-entries/search", post(memory::search))
        .route("/v1/memory-entries/force", post(memory::force_write))
        .route("/v1/memory-entries/backfill", post(memory::backfill))
        .route(
            "/v1/memory-notifications/sse",
            get(memory::review_notifications_sse),
        )
        .route("/v1/memory/decisions", get(memory::list_decisions))
        .route(
            "/v1/memory/decisions/stats",
            get(memory::get_decision_stats),
        )
        .route("/v1/memory/compact", post(memory::trigger_compaction))
        .route("/v1/escalations", get(escalations::list))
        .route("/v1/escalations/:id", get(escalations::get))
        .route("/v1/escalations/:id/resolve", post(escalations::resolve))
}
