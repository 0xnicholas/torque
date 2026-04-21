use crate::models::v1::team::{MemberSelector, SelectorType, TeamMode, TeamTask, TeamTaskStatus, TriageResult, TaskComplexity, ProcessingPath};
use crate::service::team::modes::TeamModeHandler;
use crate::service::team::{SelectorResolver, SharedTaskStateManager, TeamEventEmitter};
use crate::repository::{DelegationRepository, TeamTaskRepository};
use std::sync::Arc;
use uuid::Uuid;

pub struct TeamSupervisor {
    task_repo: Arc<dyn TeamTaskRepository>,
    delegation_repo: Arc<dyn DelegationRepository>,
    selector_resolver: Arc<SelectorResolver>,
    shared_state: Arc<SharedTaskStateManager>,
    events: Arc<TeamEventEmitter>,
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
        }
    }

    pub async fn poll_and_execute(&self, team_instance_id: Uuid) -> anyhow::Result<Option<SupervisorResult>> {
        let open_tasks = self.task_repo.list_open(team_instance_id, 10).await?;

        if open_tasks.is_empty() {
            return Ok(None);
        }

        let task = &open_tasks[0];
        self.execute_task(task, team_instance_id).await
    }

    pub async fn execute_task(&self, task: &TeamTask, team_instance_id: Uuid) -> anyhow::Result<Option<SupervisorResult>> {
        self.events.task_received(team_instance_id, task.id).await?;

        let triage_result = self.triage(task).await?;
        self.events.triage_completed(team_instance_id, task.id, &triage_result).await?;
        self.task_repo.update_triage_result(task.id, &triage_result).await?;

        self.events.mode_selected(team_instance_id, task.id, &triage_result.selected_mode).await?;
        self.task_repo.update_mode(task.id, &triage_result.selected_mode.to_string()).await?;
        self.task_repo.update_status(task.id, TeamTaskStatus::InProgress).await?;

        let candidates = self.selector_resolver.resolve(
            &MemberSelector {
                selector_type: SelectorType::Any,
                capability_profiles: vec![],
                role: None,
                agent_definition_id: None,
            },
            team_instance_id,
        ).await?;

        let mode_name = match triage_result.selected_mode {
            TeamMode::Route => "route",
            TeamMode::Broadcast => "broadcast",
            TeamMode::Coordinate => "coordinate",
            TeamMode::Tasks => "tasks",
        };

        let handler = TeamModeHandler::from_mode_name(mode_name)
            .ok_or_else(|| anyhow::anyhow!("No handler for mode: {}", mode_name))?;

        let result = handler.execute(
            task,
            team_instance_id,
            candidates,
            self.delegation_repo.clone(),
            self.selector_resolver.clone(),
            self.shared_state.clone(),
            self.events.clone(),
        ).await?;

        if result.success {
            self.task_repo.update_status(task.id, TeamTaskStatus::Completed).await?;
            self.task_repo.mark_completed(task.id).await?;
            self.events.team_completed(team_instance_id, task.id).await?;
        } else {
            self.task_repo.update_status(task.id, TeamTaskStatus::Failed).await?;
            self.events.team_failed(team_instance_id, task.id, &result.summary).await?;
        }

        Ok(Some(SupervisorResult {
            task_id: task.id,
            success: result.success,
            summary: result.summary,
        }))
    }

    async fn triage(&self, task: &TeamTask) -> anyhow::Result<TriageResult> {
        let complexity = if task.goal.len() > 200 {
            TaskComplexity::Complex
        } else if task.goal.len() > 100 {
            TaskComplexity::Medium
        } else {
            TaskComplexity::Simple
        };

        let (processing_path, selected_mode) = match complexity {
            TaskComplexity::Simple => (
                ProcessingPath::SingleRoute,
                TeamMode::Route,
            ),
            TaskComplexity::Medium => (
                ProcessingPath::GuidedDelegate,
                TeamMode::Route,
            ),
            TaskComplexity::Complex => (
                ProcessingPath::StructuredOrchestration,
                TeamMode::Tasks,
            ),
        };

        Ok(TriageResult {
            complexity: complexity.clone(),
            processing_path,
            selected_mode: selected_mode.clone(),
            lead_member_ref: None,
            rationale: format!("Triage determined {:?} complexity (goal len: {}), using {:?} mode", complexity, task.goal.len(), selected_mode),
        })
    }
}

#[derive(Debug)]
pub struct SupervisorResult {
    pub task_id: Uuid,
    pub success: bool,
    pub summary: String,
}