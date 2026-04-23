use crate::models::v1::partial_quality::PartialQuality;
use crate::models::v1::team::{
    MemberSelector, ProcessingPath, SelectorType, TaskComplexity, TeamMode, TeamTask,
    TeamTaskStatus, TriageResult,
};
use crate::repository::{DelegationRepository, TeamTaskRepository};
use crate::service::team::event_listener::EventListener;
use crate::service::team::modes::TeamModeHandler;
use crate::service::team::supervisor_agent::SupervisorAgent;
use crate::service::team::{SelectorResolver, SharedTaskStateManager, TeamEventEmitter};
use std::sync::Arc;
use tokio::time::{timeout, Duration, Instant};
use tokio_stream::StreamExt;
use uuid::Uuid;

pub struct TeamSupervisor {
    task_repo: Arc<dyn TeamTaskRepository>,
    delegation_repo: Arc<dyn DelegationRepository>,
    selector_resolver: Arc<SelectorResolver>,
    shared_state: Arc<SharedTaskStateManager>,
    events: Arc<TeamEventEmitter>,
    supervisor_agent: Option<SupervisorAgent>,
}

impl TeamSupervisor {
    pub fn new(
        task_repo: Arc<dyn TeamTaskRepository>,
        delegation_repo: Arc<dyn DelegationRepository>,
        selector_resolver: Arc<SelectorResolver>,
        shared_state: Arc<SharedTaskStateManager>,
        events: Arc<TeamEventEmitter>,
    ) -> Self {
        Self {
            task_repo,
            delegation_repo,
            selector_resolver,
            shared_state,
            events,
            supervisor_agent: None,
        }
    }

    pub fn with_supervisor_agent(mut self, agent: SupervisorAgent) -> Self {
        self.supervisor_agent = Some(agent);
        self
    }

    pub async fn poll_and_execute(
        &self,
        team_instance_id: Uuid,
    ) -> anyhow::Result<Option<SupervisorResult>> {
        let open_tasks = self.task_repo.list_open(team_instance_id, 10).await?;

        if open_tasks.is_empty() {
            return Ok(None);
        }

        let task = &open_tasks[0];
        self.execute_task(task, team_instance_id).await
    }

    pub async fn wait_for_delegation_completion(
        &self,
        delegation_id: Uuid,
        event_listener: Arc<dyn EventListener>,
        timeout_duration: Duration,
    ) -> anyhow::Result<DelegationWaitResult> {
        let deadline = Instant::now() + timeout_duration;
        let mut stream = event_listener.subscribe_delegation(delegation_id).await?;

        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Ok(DelegationWaitResult::Timeout);
            }

            match timeout(remaining, stream.next()).await {
                Ok(Some(event)) => match event {
                    crate::models::v1::delegation_event::DelegationEvent::Completed { .. } => {
                        return Ok(DelegationWaitResult::Completed);
                    }
                    crate::models::v1::delegation_event::DelegationEvent::Failed {
                        error, ..
                    } => {
                        return Ok(DelegationWaitResult::Failed(error));
                    }
                    crate::models::v1::delegation_event::DelegationEvent::TimeoutPartial {
                        partial_quality,
                        ..
                    } => {
                        return Ok(DelegationWaitResult::TimeoutPartial(partial_quality));
                    }
                    crate::models::v1::delegation_event::DelegationEvent::Rejected {
                        reason,
                        ..
                    } => {
                        return Ok(DelegationWaitResult::Rejected(reason.to_string()));
                    }
                    _ => continue,
                },
                Ok(None) => continue,
                Err(_) => return Ok(DelegationWaitResult::Timeout),
            }
        }
    }

    pub async fn execute_task(
        &self,
        task: &TeamTask,
        team_instance_id: Uuid,
    ) -> anyhow::Result<Option<SupervisorResult>> {
        self.events.task_received(team_instance_id, task.id).await?;

        let triage_result = self.triage(task).await?;
        self.events
            .triage_completed(team_instance_id, task.id, &triage_result)
            .await?;
        self.task_repo
            .update_triage_result(task.id, &triage_result)
            .await?;

        self.events
            .mode_selected(team_instance_id, task.id, &triage_result.selected_mode)
            .await?;
        self.task_repo
            .update_mode(task.id, &triage_result.selected_mode.to_string())
            .await?;
        self.task_repo
            .update_status(task.id, TeamTaskStatus::InProgress)
            .await?;

        let candidates = self
            .selector_resolver
            .resolve(
                &MemberSelector {
                    selector_type: SelectorType::Any,
                    capability_profiles: vec![],
                    role: None,
                    agent_definition_id: None,
                },
                team_instance_id,
            )
            .await?;

        let mode_name = match triage_result.selected_mode {
            TeamMode::Route => "route",
            TeamMode::Broadcast => "broadcast",
            TeamMode::Coordinate => "coordinate",
            TeamMode::Tasks => "tasks",
        };

        let handler = TeamModeHandler::from_mode_name(mode_name)
            .ok_or_else(|| anyhow::anyhow!("No handler for mode: {}", mode_name))?;

        let result = handler
            .execute(
                task,
                team_instance_id,
                candidates,
                self.delegation_repo.clone(),
                self.selector_resolver.clone(),
                self.shared_state.clone(),
                self.events.clone(),
            )
            .await?;

        if result.success {
            self.task_repo
                .update_status(task.id, TeamTaskStatus::Completed)
                .await?;
            self.task_repo.mark_completed(task.id).await?;
            self.events
                .team_completed(team_instance_id, task.id)
                .await?;
        } else {
            self.task_repo
                .update_status(task.id, TeamTaskStatus::Failed)
                .await?;
            self.events
                .team_failed(team_instance_id, task.id, &result.summary)
                .await?;
        }

        Ok(Some(SupervisorResult {
            task_id: task.id,
            success: result.success,
            summary: result.summary,
        }))
    }

    async fn triage(&self, task: &TeamTask) -> anyhow::Result<TriageResult> {
        if let Some(ref _agent) = self.supervisor_agent {
            // For MVP, we still use heuristic but could call LLM here
            // Full LLM integration is deferred to Task 15
        }

        let complexity = if task.goal.len() > 200 {
            TaskComplexity::Complex
        } else if task.goal.len() > 100 {
            TaskComplexity::Medium
        } else {
            TaskComplexity::Simple
        };

        let (processing_path, selected_mode) = match complexity {
            TaskComplexity::Simple => (ProcessingPath::SingleRoute, TeamMode::Route),
            TaskComplexity::Medium => (ProcessingPath::GuidedDelegate, TeamMode::Route),
            TaskComplexity::Complex => (ProcessingPath::StructuredOrchestration, TeamMode::Tasks),
        };

        Ok(TriageResult {
            complexity: complexity.clone(),
            processing_path,
            selected_mode: selected_mode.clone(),
            lead_member_ref: None,
            rationale: format!(
                "Triage determined {:?} complexity (goal len: {}), using {:?} mode. LLM integration deferred to Task 15.",
                complexity,
                task.goal.len(),
                selected_mode
            ),
        })
    }
}

#[derive(Debug)]
pub struct SupervisorResult {
    pub task_id: Uuid,
    pub success: bool,
    pub summary: String,
}

#[derive(Debug)]
pub enum DelegationWaitResult {
    Completed,
    Failed(String),
    TimeoutPartial(PartialQuality),
    Rejected(String),
    Timeout,
}
