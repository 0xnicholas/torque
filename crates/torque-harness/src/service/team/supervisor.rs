use crate::infra::llm::LlmClient;
use crate::models::v1::partial_quality::PartialQuality;
use crate::models::v1::team::{
    MemberSelector, SelectorType, TeamMode, TeamTask,
    TeamTaskStatus, TriageResult,
};
use crate::repository::{DelegationRepository, TeamTaskRepository};
use crate::service::team::event_listener::EventListener;
use crate::service::team::modes::TeamModeHandler;
use crate::service::team::supervisor_agent::SupervisorAgent;
use crate::service::team::supervisor_tools::create_supervisor_tools;
use crate::service::team::{SelectorResolver, SharedTaskStateManager, TeamEventEmitter};
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;
use tokio::time::{timeout, Duration, Instant};
use tokio_stream::StreamExt;
use uuid::Uuid;

pub struct TeamSupervisor {
    task_repo: Arc<dyn TeamTaskRepository>,
    delegation_repo: Arc<dyn DelegationRepository>,
    selector_resolver: Arc<SelectorResolver>,
    shared_state: Arc<SharedTaskStateManager>,
    events: Arc<TeamEventEmitter>,
    supervisor_agent: TokioMutex<Option<SupervisorAgent>>,
    llm: Option<Arc<dyn LlmClient>>,
    event_listener: Option<Arc<dyn EventListener>>,
    delegation_timeout: Duration,
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
            supervisor_agent: TokioMutex::new(None),
            llm: None,
            event_listener: None,
            delegation_timeout: Duration::from_secs(300),
        }
    }

    pub fn with_llm(mut self, llm: Arc<dyn LlmClient>) -> Self {
        self.llm = Some(llm);
        self
    }

    pub fn with_supervisor_agent(mut self, agent: SupervisorAgent) -> Self {
        self.supervisor_agent = TokioMutex::new(Some(agent));
        self
    }

    pub fn with_event_listener(mut self, event_listener: Arc<dyn EventListener>) -> Self {
        self.event_listener = Some(event_listener);
        self
    }

    pub fn with_delegation_timeout(mut self, timeout: Duration) -> Self {
        self.delegation_timeout = timeout;
        self
    }

    async fn ensure_supervisor_agent(&self) -> anyhow::Result<()> {
        {
            let guard = self.supervisor_agent.lock().await;
            if guard.is_some() {
                return Ok(());
            }
        }
        if let Some(llm) = &self.llm {
            let tools = create_supervisor_tools();
            let agent = SupervisorAgent::new(llm.clone(), tools).await;
            let mut guard = self.supervisor_agent.lock().await;
            *guard = Some(agent);
        }
        Ok(())
    }

    pub async fn poll_and_execute(
        &self,
        team_instance_id: Uuid,
    ) -> anyhow::Result<Option<SupervisorResult>> {
        self.ensure_supervisor_agent().await?;

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

        let event_listener = self
            .event_listener
            .clone()
            .ok_or_else(|| anyhow::anyhow!("EventListener not configured"))?;

        let result = handler
            .execute(
                task,
                team_instance_id,
                candidates,
                self.delegation_repo.clone(),
                self.selector_resolver.clone(),
                self.shared_state.clone(),
                self.events.clone(),
                event_listener,
                self.delegation_timeout,
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
        let agent = {
            let guard = self.supervisor_agent.lock().await;
            match &*guard {
                Some(agent) => agent.triage(&task.goal).await?,
                None => {
                    return Err(anyhow::anyhow!(
                        "SupervisorAgent not available - LLM client may not be configured"
                    ));
                }
            }
        };

        Ok(agent)
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
