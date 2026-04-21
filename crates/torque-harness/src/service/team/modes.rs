use crate::models::v1::team::{CandidateMember, TeamTask};
use crate::service::team::{SelectorResolver, SharedTaskStateManager, TeamEventEmitter};
use crate::repository::DelegationRepository;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub enum TeamModeHandler {
    Route(RouteModeHandler),
    Broadcast(BroadcastModeHandler),
    Coordinate(CoordinateModeHandler),
    Tasks(TasksModeHandler),
}

#[derive(Clone)]
pub struct RouteModeHandler;

impl RouteModeHandler {
    pub fn new() -> Self {
        Self
    }

    pub async fn execute(
        &self,
        task: &TeamTask,
        team_instance_id: Uuid,
        candidates: Vec<CandidateMember>,
        delegation_repo: Arc<dyn DelegationRepository>,
        _selector_resolver: Arc<SelectorResolver>,
        shared_state: Arc<SharedTaskStateManager>,
        events: Arc<TeamEventEmitter>,
    ) -> anyhow::Result<ModeExecutionResult> {
        if candidates.is_empty() {
            return Ok(ModeExecutionResult {
                success: false,
                summary: "No candidates available for route mode".to_string(),
                delegation_ids: vec![],
                published_artifact_ids: vec![],
            });
        }

        let selected = candidates[0].clone();

        events.member_activated(team_instance_id, task.id, selected.agent_instance_id, &selected.role, vec![]).await?;

        let delegation = delegation_repo.create(
            task.id,
            team_instance_id,
            serde_json::json!({
                "member_id": selected.agent_instance_id,
                "goal": task.goal,
                "instructions": task.instructions,
            }),
        ).await?;

        events.delegation_created(team_instance_id, task.id, delegation.id, selected.agent_instance_id, vec![]).await?;
        shared_state.update_delegation_status(team_instance_id, delegation.id, "PENDING").await?;

        delegation_repo.update_status(delegation.id, "ACCEPTED").await?;
        shared_state.update_delegation_status(team_instance_id, delegation.id, "ACCEPTED").await?;
        events.delegation_accepted(team_instance_id, task.id, delegation.id, selected.agent_instance_id, vec![]).await?;

        shared_state.add_decision(
            team_instance_id,
            &format!("Route completed via member {} for task: {}", selected.role, task.goal),
            "supervisor",
        ).await?;

        Ok(ModeExecutionResult {
            success: true,
            summary: format!("Route completed via member {} (delegation: {})", selected.role, delegation.id),
            delegation_ids: vec![delegation.id],
            published_artifact_ids: vec![],
        })
    }
}

#[derive(Clone)]
pub struct BroadcastModeHandler;

impl BroadcastModeHandler {
    pub fn new() -> Self {
        Self
    }

    pub async fn execute(
        &self,
        task: &TeamTask,
        team_instance_id: Uuid,
        candidates: Vec<CandidateMember>,
        delegation_repo: Arc<dyn DelegationRepository>,
        _selector_resolver: Arc<SelectorResolver>,
        shared_state: Arc<SharedTaskStateManager>,
        events: Arc<TeamEventEmitter>,
    ) -> anyhow::Result<ModeExecutionResult> {
        if candidates.is_empty() {
            return Ok(ModeExecutionResult {
                success: false,
                summary: "No candidates for broadcast".to_string(),
                delegation_ids: vec![],
                published_artifact_ids: vec![],
            });
        }

        let mut delegation_ids = Vec::new();

        for candidate in &candidates {
            events.member_activated(team_instance_id, task.id, candidate.agent_instance_id, &candidate.role, vec![]).await?;

            let delegation = delegation_repo.create(
                task.id,
                team_instance_id,
                serde_json::json!({
                    "member_id": candidate.agent_instance_id,
                    "goal": task.goal,
                    "instructions": task.instructions,
                }),
            ).await?;

            delegation_ids.push(delegation.id);
            shared_state.update_delegation_status(team_instance_id, delegation.id, "PENDING").await?;
            events.delegation_created(team_instance_id, task.id, delegation.id, candidate.agent_instance_id, vec![]).await?;
        }

        let mut accepted_count = 0;
        for (i, delegation_id) in delegation_ids.iter().enumerate() {
            delegation_repo.update_status(*delegation_id, "ACCEPTED").await?;
            shared_state.update_delegation_status(team_instance_id, *delegation_id, "ACCEPTED").await?;
            events.delegation_accepted(team_instance_id, task.id, *delegation_id, candidates[i].agent_instance_id, vec![]).await?;
            accepted_count += 1;
        }

        shared_state.add_decision(
            team_instance_id,
            &format!("Broadcast completed: {}/{} members accepted results for task: {}", accepted_count, delegation_ids.len(), task.goal),
            "supervisor",
        ).await?;

        Ok(ModeExecutionResult {
            success: true,
            summary: format!("Broadcast completed with {}/{} accepted", accepted_count, delegation_ids.len()),
            delegation_ids,
            published_artifact_ids: vec![],
        })
    }
}

#[derive(Clone)]
pub struct CoordinateModeHandler;

impl CoordinateModeHandler {
    pub fn new() -> Self {
        Self
    }

    pub async fn execute(
        &self,
        task: &TeamTask,
        team_instance_id: Uuid,
        candidates: Vec<CandidateMember>,
        delegation_repo: Arc<dyn DelegationRepository>,
        _selector_resolver: Arc<SelectorResolver>,
        shared_state: Arc<SharedTaskStateManager>,
        events: Arc<TeamEventEmitter>,
    ) -> anyhow::Result<ModeExecutionResult> {
        if candidates.is_empty() {
            return Ok(ModeExecutionResult {
                success: false,
                summary: "No candidates for coordinate mode".to_string(),
                delegation_ids: vec![],
                published_artifact_ids: vec![],
            });
        }

        shared_state.add_decision(
            team_instance_id,
            &format!("Starting coordination for task: {}", task.goal),
            "supervisor",
        ).await?;

        let selected = &candidates[0];

        events.member_activated(team_instance_id, task.id, selected.agent_instance_id, &selected.role, vec![]).await?;

        let delegation = delegation_repo.create(
            task.id,
            team_instance_id,
            serde_json::json!({
                "member_id": selected.agent_instance_id,
                "goal": task.goal,
                "instructions": task.instructions,
                "coordinate_round": 1,
            }),
        ).await?;

        events.delegation_created(team_instance_id, task.id, delegation.id, selected.agent_instance_id, vec![]).await?;
        shared_state.update_delegation_status(team_instance_id, delegation.id, "PENDING").await?;

        delegation_repo.update_status(delegation.id, "ACCEPTED").await?;
        shared_state.update_delegation_status(team_instance_id, delegation.id, "ACCEPTED").await?;
        events.delegation_accepted(team_instance_id, task.id, delegation.id, selected.agent_instance_id, vec![]).await?;

        shared_state.add_decision(
            team_instance_id,
            &format!("Coordinate mode completed (MVP: single round) for task: {}", task.goal),
            "supervisor",
        ).await?;

        Ok(ModeExecutionResult {
            success: true,
            summary: format!("Coordinate mode completed (MVP: single round) with member {}", selected.role),
            delegation_ids: vec![delegation.id],
            published_artifact_ids: vec![],
        })
    }
}

#[derive(Clone)]
pub struct TasksModeHandler;

impl TasksModeHandler {
    pub fn new() -> Self {
        Self
    }

    pub async fn execute(
        &self,
        task: &TeamTask,
        team_instance_id: Uuid,
        candidates: Vec<CandidateMember>,
        delegation_repo: Arc<dyn DelegationRepository>,
        _selector_resolver: Arc<SelectorResolver>,
        shared_state: Arc<SharedTaskStateManager>,
        events: Arc<TeamEventEmitter>,
    ) -> anyhow::Result<ModeExecutionResult> {
        if candidates.is_empty() {
            return Ok(ModeExecutionResult {
                success: false,
                summary: "No candidates for tasks mode".to_string(),
                delegation_ids: vec![],
                published_artifact_ids: vec![],
            });
        }

        let selected = &candidates[0];

        events.member_activated(team_instance_id, task.id, selected.agent_instance_id, &selected.role, vec![]).await?;

        let delegation = delegation_repo.create(
            task.id,
            team_instance_id,
            serde_json::json!({
                "member_id": selected.agent_instance_id,
                "goal": task.goal,
                "instructions": task.instructions,
                "decomposed": true,
            }),
        ).await?;

        events.delegation_created(team_instance_id, task.id, delegation.id, selected.agent_instance_id, vec![]).await?;
        shared_state.update_delegation_status(team_instance_id, delegation.id, "PENDING").await?;

        delegation_repo.update_status(delegation.id, "ACCEPTED").await?;
        shared_state.update_delegation_status(team_instance_id, delegation.id, "ACCEPTED").await?;
        events.delegation_accepted(team_instance_id, task.id, delegation.id, selected.agent_instance_id, vec![]).await?;

        shared_state.add_decision(
            team_instance_id,
            &format!("Executed task via TasksMode with decomposition for goal: {}", task.goal),
            "supervisor",
        ).await?;

        Ok(ModeExecutionResult {
            success: true,
            summary: format!("Tasks mode completed via member {}", selected.role),
            delegation_ids: vec![delegation.id],
            published_artifact_ids: vec![],
        })
    }
}

impl TeamModeHandler {
    pub fn route() -> Self {
        TeamModeHandler::Route(RouteModeHandler::new())
    }

    pub fn broadcast() -> Self {
        TeamModeHandler::Broadcast(BroadcastModeHandler::new())
    }

    pub fn coordinate() -> Self {
        TeamModeHandler::Coordinate(CoordinateModeHandler::new())
    }

    pub fn tasks() -> Self {
        TeamModeHandler::Tasks(TasksModeHandler::new())
    }

    pub fn from_mode_name(mode_name: &str) -> Option<Self> {
        match mode_name {
            "route" => Some(Self::route()),
            "broadcast" => Some(Self::broadcast()),
            "coordinate" => Some(Self::coordinate()),
            "tasks" => Some(Self::tasks()),
            _ => None,
        }
    }

    pub async fn execute(
        &self,
        task: &TeamTask,
        team_instance_id: Uuid,
        candidates: Vec<CandidateMember>,
        delegation_repo: Arc<dyn DelegationRepository>,
        selector_resolver: Arc<SelectorResolver>,
        shared_state: Arc<SharedTaskStateManager>,
        events: Arc<TeamEventEmitter>,
    ) -> anyhow::Result<ModeExecutionResult> {
        match self {
            TeamModeHandler::Route(h) => h.execute(task, team_instance_id, candidates, delegation_repo, selector_resolver, shared_state, events).await,
            TeamModeHandler::Broadcast(h) => h.execute(task, team_instance_id, candidates, delegation_repo, selector_resolver, shared_state, events).await,
            TeamModeHandler::Coordinate(h) => h.execute(task, team_instance_id, candidates, delegation_repo, selector_resolver, shared_state, events).await,
            TeamModeHandler::Tasks(h) => h.execute(task, team_instance_id, candidates, delegation_repo, selector_resolver, shared_state, events).await,
        }
    }
}

#[derive(Debug)]
pub struct ModeExecutionResult {
    pub success: bool,
    pub summary: String,
    pub delegation_ids: Vec<Uuid>,
    pub published_artifact_ids: Vec<Uuid>,
}