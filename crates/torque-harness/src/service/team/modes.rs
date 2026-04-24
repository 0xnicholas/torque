use crate::models::v1::delegation_event::DelegationEvent;
use crate::models::v1::partial_quality::PartialQuality;
use crate::models::v1::team::{CandidateMember, TeamTask};
use crate::repository::DelegationRepository;
use crate::service::team::event_listener::EventListener;
use crate::service::team::{SelectorResolver, SharedTaskStateManager, TeamEventEmitter};
use std::sync::Arc;
use tokio::time::{timeout, Duration, Instant};
use tokio_stream::StreamExt;
use uuid::Uuid;

pub async fn wait_for_delegation_completion(
    delegation_id: Uuid,
    event_listener: Arc<dyn EventListener>,
    timeout_duration: Duration,
) -> DelegationWaitOutcome {
    let deadline = Instant::now() + timeout_duration;
    let Ok(stream) = event_listener.subscribe_delegation(delegation_id).await else {
        return DelegationWaitOutcome::Timeout;
    };

    let mut stream = stream;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return DelegationWaitOutcome::Timeout;
        }

        match timeout(remaining, stream.next()).await {
            Ok(Some(event)) => match event {
                DelegationEvent::Completed { .. } => {
                    return DelegationWaitOutcome::Completed;
                }
                DelegationEvent::Failed { error, .. } => {
                    return DelegationWaitOutcome::Failed(error);
                }
                DelegationEvent::TimeoutPartial {
                    partial_quality, ..
                } => {
                    return DelegationWaitOutcome::TimeoutPartial(partial_quality);
                }
                DelegationEvent::Rejected { reason, .. } => {
                    return DelegationWaitOutcome::Rejected(reason.to_string());
                }
                _ => continue,
            },
            Ok(None) => continue,
            Err(_) => return DelegationWaitOutcome::Timeout,
        }
    }
}

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
        event_listener: Arc<dyn EventListener>,
        timeout_duration: Duration,
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

        events
            .member_activated(
                team_instance_id,
                task.id,
                selected.agent_instance_id,
                &selected.role,
                vec![],
            )
            .await?;

        let delegation = delegation_repo
            .create(
                task.id,
                team_instance_id,
                serde_json::json!({
                    "member_id": selected.agent_instance_id,
                    "goal": task.goal,
                    "instructions": task.instructions,
                }),
            )
            .await?;

        events
            .delegation_created(
                team_instance_id,
                task.id,
                delegation.id,
                selected.agent_instance_id,
                vec![],
            )
            .await?;
        shared_state
            .update_delegation_status(team_instance_id, delegation.id, "PENDING")
            .await?;

        let wait_result = wait_for_delegation_completion(delegation.id, event_listener, timeout_duration)
            .await;

        match wait_result {
            DelegationWaitOutcome::Completed => {
                delegation_repo
                    .update_status(delegation.id, "ACCEPTED")
                    .await?;
                shared_state
                    .update_delegation_status(team_instance_id, delegation.id, "ACCEPTED")
                    .await?;
                events
                    .delegation_accepted(
                        team_instance_id,
                        task.id,
                        delegation.id,
                        selected.agent_instance_id,
                        vec![],
                    )
                    .await?;
                shared_state
                    .add_decision(
                        team_instance_id,
                        &format!(
                            "Route completed via member {} for task: {}",
                            selected.role, task.goal
                        ),
                        "supervisor",
                    )
                    .await?;
                Ok(ModeExecutionResult {
                    success: true,
                    summary: format!(
                        "Route completed via member {} (delegation: {})",
                        selected.role, delegation.id
                    ),
                    delegation_ids: vec![delegation.id],
                    published_artifact_ids: vec![],
                })
            }
            DelegationWaitOutcome::Failed(error) => {
                delegation_repo.update_status(delegation.id, "FAILED").await?;
                shared_state
                    .update_delegation_status(team_instance_id, delegation.id, "FAILED")
                    .await?;
                events
                    .member_result_received(
                        team_instance_id,
                        task.id,
                        delegation.id,
                        selected.agent_instance_id,
                        vec![],
                    )
                    .await?;
                shared_state
                    .add_decision(
                        team_instance_id,
                        &format!(
                            "Route failed via member {} for task: {} - {}",
                            selected.role, task.goal, error
                        ),
                        "supervisor",
                    )
                    .await?;
                Ok(ModeExecutionResult {
                    success: false,
                    summary: format!("Delegation failed: {}", error),
                    delegation_ids: vec![delegation.id],
                    published_artifact_ids: vec![],
                })
            }
            DelegationWaitOutcome::Rejected(reason) => {
                delegation_repo.update_status(delegation.id, "REJECTED").await?;
                shared_state
                    .update_delegation_status(team_instance_id, delegation.id, "REJECTED")
                    .await?;
                events
                    .delegation_rejected(
                        team_instance_id,
                        task.id,
                        delegation.id,
                        selected.agent_instance_id,
                        &reason,
                        vec![],
                    )
                    .await?;
                shared_state
                    .add_decision(
                        team_instance_id,
                        &format!(
                            "Route rejected by member {} for task: {} - {}",
                            selected.role, task.goal, reason
                        ),
                        "supervisor",
                    )
                    .await?;
                Ok(ModeExecutionResult {
                    success: false,
                    summary: format!("Delegation rejected: {}", reason),
                    delegation_ids: vec![delegation.id],
                    published_artifact_ids: vec![],
                })
            }
            DelegationWaitOutcome::Timeout => {
                delegation_repo.update_status(delegation.id, "TIMEOUT").await?;
                shared_state
                    .update_delegation_status(team_instance_id, delegation.id, "TIMEOUT")
                    .await?;
                Ok(ModeExecutionResult {
                    success: false,
                    summary: "Delegation timed out".to_string(),
                    delegation_ids: vec![delegation.id],
                    published_artifact_ids: vec![],
                })
            }
            DelegationWaitOutcome::TimeoutPartial(partial_quality) => {
                delegation_repo
                    .update_status(delegation.id, "TIMEOUT_PARTIAL")
                    .await?;
                shared_state
                    .update_delegation_status(team_instance_id, delegation.id, "TIMEOUT_PARTIAL")
                    .await?;
                Ok(ModeExecutionResult {
                    success: false,
                    summary: format!(
                        "Delegation timed out with partial quality: {:?}",
                        partial_quality
                    ),
                    delegation_ids: vec![delegation.id],
                    published_artifact_ids: vec![],
                })
            }
        }
    }
}

#[derive(Debug)]
pub enum DelegationWaitOutcome {
    Completed,
    Failed(String),
    Rejected(String),
    TimeoutPartial(PartialQuality),
    Timeout,
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
        event_listener: Arc<dyn EventListener>,
        timeout_duration: Duration,
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
            events
                .member_activated(
                    team_instance_id,
                    task.id,
                    candidate.agent_instance_id,
                    &candidate.role,
                    vec![],
                )
                .await?;

            let delegation = delegation_repo
                .create(
                    task.id,
                    team_instance_id,
                    serde_json::json!({
                        "member_id": candidate.agent_instance_id,
                        "goal": task.goal,
                        "instructions": task.instructions,
                    }),
                )
                .await?;

            delegation_ids.push(delegation.id);
            shared_state
                .update_delegation_status(team_instance_id, delegation.id, "PENDING")
                .await?;
            events
                .delegation_created(
                    team_instance_id,
                    task.id,
                    delegation.id,
                    candidate.agent_instance_id,
                    vec![],
                )
                .await?;
        }

        let mut accepted_count = 0;
        let mut failed_count = 0;
        for (i, delegation_id) in delegation_ids.iter().enumerate() {
            let outcome = wait_for_delegation_completion(*delegation_id, event_listener.clone(), timeout_duration)
                .await;

            match outcome {
                DelegationWaitOutcome::Completed => {
                    delegation_repo.update_status(*delegation_id, "ACCEPTED").await?;
                    shared_state
                        .update_delegation_status(team_instance_id, *delegation_id, "ACCEPTED")
                        .await?;
                    events
                        .delegation_accepted(
                            team_instance_id,
                            task.id,
                            *delegation_id,
                            candidates[i].agent_instance_id,
                            vec![],
                        )
                        .await?;
                    accepted_count += 1;
                }
                DelegationWaitOutcome::Failed(_) => {
                    delegation_repo.update_status(*delegation_id, "FAILED").await?;
                    shared_state
                        .update_delegation_status(team_instance_id, *delegation_id, "FAILED")
                        .await?;
                    events
                        .member_result_received(
                            team_instance_id,
                            task.id,
                            *delegation_id,
                            candidates[i].agent_instance_id,
                            vec![],
                        )
                        .await?;
                    failed_count += 1;
                }
                DelegationWaitOutcome::Rejected(_) => {
                    delegation_repo.update_status(*delegation_id, "REJECTED").await?;
                    shared_state
                        .update_delegation_status(team_instance_id, *delegation_id, "REJECTED")
                        .await?;
                    events
                        .delegation_rejected(
                            team_instance_id,
                            task.id,
                            *delegation_id,
                            candidates[i].agent_instance_id,
                            "member rejected",
                            vec![],
                        )
                        .await?;
                    failed_count += 1;
                }
                DelegationWaitOutcome::Timeout => {
                    delegation_repo.update_status(*delegation_id, "TIMEOUT").await?;
                    shared_state
                        .update_delegation_status(team_instance_id, *delegation_id, "TIMEOUT")
                        .await?;
                    failed_count += 1;
                }
                DelegationWaitOutcome::TimeoutPartial(_) => {
                    delegation_repo
                        .update_status(*delegation_id, "TIMEOUT_PARTIAL")
                        .await?;
                    shared_state
                        .update_delegation_status(team_instance_id, *delegation_id, "TIMEOUT_PARTIAL")
                        .await?;
                    failed_count += 1;
                }
            }
        }

        shared_state
            .add_decision(
                team_instance_id,
                &format!(
                    "Broadcast completed: {}/{} accepted, {}/{} failed for task: {}",
                    accepted_count,
                    delegation_ids.len(),
                    failed_count,
                    delegation_ids.len(),
                    task.goal
                ),
                "supervisor",
            )
            .await?;

        Ok(ModeExecutionResult {
            success: failed_count == 0,
            summary: format!(
                "Broadcast completed with {}/{} accepted, {}/{} failed",
                accepted_count,
                delegation_ids.len(),
                failed_count,
                delegation_ids.len()
            ),
            delegation_ids,
            published_artifact_ids: vec![],
        })
    }
}

#[derive(Clone)]
pub struct CoordinateModeHandler {
    max_rounds: usize,
}

impl CoordinateModeHandler {
    pub fn new() -> Self {
        Self { max_rounds: 3 }
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
        event_listener: Arc<dyn EventListener>,
        timeout_duration: Duration,
    ) -> anyhow::Result<ModeExecutionResult> {
        if candidates.is_empty() {
            return Ok(ModeExecutionResult {
                success: false,
                summary: "No candidates for coordinate mode".to_string(),
                delegation_ids: vec![],
                published_artifact_ids: vec![],
            });
        }

        let max_rounds = self.max_rounds;
        let coordination_rounds = candidates.len().min(max_rounds);

        shared_state
            .add_decision(
                team_instance_id,
                &format!(
                    "Starting coordination for task: {} with {} rounds",
                    task.goal, coordination_rounds
                ),
                "supervisor",
            )
            .await?;

        let mut delegation_ids = Vec::new();
        let mut previous_result_available = false;
        let mut failed_rounds = 0;

        for round in 1..=coordination_rounds {
            let member_index = (round - 1) % candidates.len();
            let selected = &candidates[member_index];

            shared_state
                .add_decision(
                    team_instance_id,
                    &format!(
                        "Coordination round {}: delegating to {}",
                        round, selected.role
                    ),
                    "supervisor",
                )
                .await?;

            events
                .member_activated(
                    team_instance_id,
                    task.id,
                    selected.agent_instance_id,
                    &selected.role,
                    vec![],
                )
                .await?;

            let delegation_payload = serde_json::json!({
                "member_id": selected.agent_instance_id,
                "goal": task.goal,
                "instructions": task.instructions,
                "coordinate_round": round,
                "previous_round_completed": previous_result_available,
            });

            let delegation = delegation_repo
                .create(task.id, team_instance_id, delegation_payload)
                .await?;

            delegation_ids.push(delegation.id);
            events
                .delegation_created(
                    team_instance_id,
                    task.id,
                    delegation.id,
                    selected.agent_instance_id,
                    vec![],
                )
                .await?;
            shared_state
                .update_delegation_status(team_instance_id, delegation.id, "PENDING")
                .await?;

            let outcome = wait_for_delegation_completion(delegation.id, event_listener.clone(), timeout_duration)
                .await;

            match outcome {
                DelegationWaitOutcome::Completed => {
                    delegation_repo.update_status(delegation.id, "ACCEPTED").await?;
                    shared_state
                        .update_delegation_status(team_instance_id, delegation.id, "ACCEPTED")
                        .await?;
                    events
                        .delegation_accepted(
                            team_instance_id,
                            task.id,
                            delegation.id,
                            selected.agent_instance_id,
                            vec![],
                        )
                        .await?;
                    previous_result_available = true;
                }
                DelegationWaitOutcome::Failed(_) | DelegationWaitOutcome::Rejected(_) => {
                    delegation_repo.update_status(delegation.id, "FAILED").await?;
                    shared_state
                        .update_delegation_status(team_instance_id, delegation.id, "FAILED")
                        .await?;
                    events
                        .member_result_received(
                            team_instance_id,
                            task.id,
                            delegation.id,
                            selected.agent_instance_id,
                            vec![],
                        )
                        .await?;
                    failed_rounds += 1;
                    break;
                }
                DelegationWaitOutcome::Timeout | DelegationWaitOutcome::TimeoutPartial(_) => {
                    delegation_repo.update_status(delegation.id, "TIMEOUT").await?;
                    shared_state
                        .update_delegation_status(team_instance_id, delegation.id, "TIMEOUT")
                        .await?;
                    failed_rounds += 1;
                    break;
                }
            }
        }

        shared_state
            .add_decision(
                team_instance_id,
                &format!(
                    "Coordinate mode completed: {} rounds ({} failed) for task: {}",
                    coordination_rounds, failed_rounds, task.goal
                ),
                "supervisor",
            )
            .await?;

        Ok(ModeExecutionResult {
            success: failed_rounds == 0,
            summary: format!(
                "Coordinate mode completed with {} rounds ({} failed)",
                coordination_rounds, failed_rounds
            ),
            delegation_ids,
            published_artifact_ids: vec![],
        })
    }
}

#[derive(Clone)]
pub struct TasksModeHandler {
    max_parallel_tasks: usize,
}

impl TasksModeHandler {
    pub fn new() -> Self {
        Self {
            max_parallel_tasks: 5,
        }
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
        event_listener: Arc<dyn EventListener>,
        timeout_duration: Duration,
    ) -> anyhow::Result<ModeExecutionResult> {
        if candidates.is_empty() {
            return Ok(ModeExecutionResult {
                success: false,
                summary: "No candidates for tasks mode".to_string(),
                delegation_ids: vec![],
                published_artifact_ids: vec![],
            });
        }

        let subtasks = self.decompose_task(&task.goal);
        let max_tasks = subtasks.len().min(self.max_parallel_tasks);
        let subtasks_to_execute = &subtasks[..max_tasks];

        shared_state
            .add_decision(
                team_instance_id,
                &format!(
                    "Starting task decomposition: {} subtasks for goal: {}",
                    subtasks_to_execute.len(),
                    task.goal
                ),
                "supervisor",
            )
            .await?;

        let mut delegation_ids = Vec::new();
        let mut failed_count = 0;

        for (idx, subtask) in subtasks_to_execute.iter().enumerate() {
            let member_index = idx % candidates.len();
            let selected = &candidates[member_index];

            shared_state
                .add_decision(
                    team_instance_id,
                    &format!(
                        "Task {}: delegating '{}' to {}",
                        idx + 1,
                        subtask,
                        selected.role
                    ),
                    "supervisor",
                )
                .await?;

            events
                .member_activated(
                    team_instance_id,
                    task.id,
                    selected.agent_instance_id,
                    &selected.role,
                    vec![],
                )
                .await?;

            let delegation = delegation_repo
                .create(
                    task.id,
                    team_instance_id,
                    serde_json::json!({
                        "member_id": selected.agent_instance_id,
                        "goal": subtask,
                        "instructions": task.instructions,
                        "parent_task_id": task.id,
                        "subtask_index": idx,
                        "total_subtasks": subtasks_to_execute.len(),
                        "decomposed": true,
                    }),
                )
                .await?;

            delegation_ids.push(delegation.id);
            events
                .delegation_created(
                    team_instance_id,
                    task.id,
                    delegation.id,
                    selected.agent_instance_id,
                    vec![],
                )
                .await?;
            shared_state
                .update_delegation_status(team_instance_id, delegation.id, "PENDING")
                .await?;

            let outcome = wait_for_delegation_completion(delegation.id, event_listener.clone(), timeout_duration)
                .await;

            match outcome {
                DelegationWaitOutcome::Completed => {
                    delegation_repo.update_status(delegation.id, "ACCEPTED").await?;
                    shared_state
                        .update_delegation_status(team_instance_id, delegation.id, "ACCEPTED")
                        .await?;
                    events
                        .delegation_accepted(
                            team_instance_id,
                            task.id,
                            delegation.id,
                            selected.agent_instance_id,
                            vec![],
                        )
                        .await?;
                }
                DelegationWaitOutcome::Failed(_) | DelegationWaitOutcome::Rejected(_) => {
                    delegation_repo.update_status(delegation.id, "FAILED").await?;
                    shared_state
                        .update_delegation_status(team_instance_id, delegation.id, "FAILED")
                        .await?;
                    events
                        .member_result_received(
                            team_instance_id,
                            task.id,
                            delegation.id,
                            selected.agent_instance_id,
                            vec![],
                        )
                        .await?;
                    failed_count += 1;
                }
                DelegationWaitOutcome::Timeout | DelegationWaitOutcome::TimeoutPartial(_) => {
                    delegation_repo.update_status(delegation.id, "TIMEOUT").await?;
                    shared_state
                        .update_delegation_status(team_instance_id, delegation.id, "TIMEOUT")
                        .await?;
                    failed_count += 1;
                }
            }
        }

        shared_state
            .add_decision(
                team_instance_id,
                &format!(
                    "Task decomposition completed: {} delegations ({} failed) for goal: {}",
                    delegation_ids.len(),
                    failed_count,
                    task.goal
                ),
                "supervisor",
            )
            .await?;

        Ok(ModeExecutionResult {
            success: failed_count == 0,
            summary: format!(
                "Tasks mode completed: {} subtasks delegated ({} failed)",
                delegation_ids.len(),
                failed_count
            ),
            delegation_ids,
            published_artifact_ids: vec![],
        })
    }

    fn decompose_task(&self, goal: &str) -> Vec<String> {
        let mut subtasks = Vec::new();

        for line in goal.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let cleaned = line
                .trim_start_matches(|c: char| c.is_numeric() || c == '.' || c == ')' || c == ':')
                .trim();

            if !cleaned.is_empty() {
                subtasks.push(cleaned.to_string());
            }
        }

        if subtasks.len() < 2 {
            let words: Vec<&str> = goal.split_whitespace().collect();
            if words.len() > 10 {
                let mid = words.len() / 2;
                subtasks.push(words[..mid].join(" "));
                subtasks.push(words[mid..].join(" "));
            }
        }

        if subtasks.is_empty() {
            subtasks.push(goal.to_string());
        }

        subtasks
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
        event_listener: Arc<dyn EventListener>,
        timeout_duration: Duration,
    ) -> anyhow::Result<ModeExecutionResult> {
        match self {
            TeamModeHandler::Route(h) => {
                h.execute(
                    task,
                    team_instance_id,
                    candidates,
                    delegation_repo,
                    selector_resolver,
                    shared_state,
                    events,
                    event_listener,
                    timeout_duration,
                )
                .await
            }
            TeamModeHandler::Broadcast(h) => {
                h.execute(
                    task,
                    team_instance_id,
                    candidates,
                    delegation_repo,
                    selector_resolver,
                    shared_state,
                    events,
                    event_listener,
                    timeout_duration,
                )
                .await
            }
            TeamModeHandler::Coordinate(h) => {
                h.execute(
                    task,
                    team_instance_id,
                    candidates,
                    delegation_repo,
                    selector_resolver,
                    shared_state,
                    events,
                    event_listener,
                    timeout_duration,
                )
                .await
            }
            TeamModeHandler::Tasks(h) => {
                h.execute(
                    task,
                    team_instance_id,
                    candidates,
                    delegation_repo,
                    selector_resolver,
                    shared_state,
                    events,
                    event_listener,
                    timeout_duration,
                )
                .await
            }
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
