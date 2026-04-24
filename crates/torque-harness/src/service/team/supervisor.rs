use crate::infra::llm::LlmClient;
use crate::models::v1::partial_quality::PartialQuality;
use crate::models::v1::team::{
    MemberSelector, SelectorType, TeamMode, TeamMember, TeamTask,
    TeamTaskStatus, TriageResult, TaskComplexity, ProcessingPath,
};
use crate::repository::{DelegationRepository, TeamTaskRepository};
use crate::service::team::event_listener::EventListener;
use crate::service::team::modes::TeamModeHandler;
use crate::service::team::supervisor_agent::SupervisorAgent;
use crate::service::team::supervisor_tools::SupervisorToolsConfig;
use crate::service::team::{SelectorResolver, SharedTaskStateManager, TeamEventEmitter};
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;
use tokio::time::{timeout, Duration, Instant};
use tokio_stream::StreamExt;
use tracing::{info, warn};
use uuid::Uuid;

pub struct RuleBasedTriage;

impl RuleBasedTriage {
    pub fn select_member(task: &TeamTask, members: &[TeamMember]) -> Option<MemberSelector> {
        let goal_lower = task.goal.to_lowercase();

        if goal_lower.contains("code") || goal_lower.contains("implement") || goal_lower.contains("build") {
            if let Some(member) = members.iter().find(|m| m.role == "engineer") {
                info!("Rule-based triage: matched 'code/implement/build' -> selected engineer member {}", member.id);
                return Some(MemberSelector {
                    selector_type: SelectorType::Role,
                    capability_profiles: vec![],
                    role: Some("engineer".to_string()),
                    agent_definition_id: None,
                });
            }
        }

        if goal_lower.contains("review") || goal_lower.contains("analyze") || goal_lower.contains("audit") {
            if let Some(member) = members.iter().find(|m| m.role == "reviewer") {
                info!("Rule-based triage: matched 'review/analyze/audit' -> selected reviewer member {}", member.id);
                return Some(MemberSelector {
                    selector_type: SelectorType::Role,
                    capability_profiles: vec![],
                    role: Some("reviewer".to_string()),
                    agent_definition_id: None,
                });
            }
        }

        if let Some(member) = members.first() {
            info!("Rule-based triage: no rule match, selecting first available member {}", member.id);
            return Some(MemberSelector {
                selector_type: SelectorType::Role,
                capability_profiles: vec![],
                role: Some(member.role.clone()),
                agent_definition_id: None,
            });
        }

        warn!("Rule-based triage: no members available");
        None
    }
}

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
    current_team_instance_id: TokioMutex<Option<Uuid>>,
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
            current_team_instance_id: TokioMutex::new(None),
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

    async fn ensure_supervisor_agent(&self, team_instance_id: Uuid) -> anyhow::Result<()> {
        let needs_recreate = {
            let current_id = self.current_team_instance_id.lock().await;
            current_id.map_or(true, |id| id != team_instance_id)
        };

        if !needs_recreate {
            let guard = self.supervisor_agent.lock().await;
            if guard.is_some() {
                return Ok(());
            }
        }

        if let Some(llm) = &self.llm {
            let team_member_repo = self.selector_resolver.team_member_repo();

            let config = SupervisorToolsConfig {
                delegation_repo: self.delegation_repo.clone(),
                selector_resolver: self.selector_resolver.clone(),
                shared_state: self.shared_state.clone(),
                team_member_repo,
                team_task_repo: self.task_repo.clone(),
                team_instance_id,
            };
            let agent = SupervisorAgent::new(llm.clone(), vec![], Some(config)).await;
            let mut guard = self.supervisor_agent.lock().await;
            let mut current_id = self.current_team_instance_id.lock().await;
            *guard = Some(agent);
            *current_id = Some(team_instance_id);
        }
        Ok(())
    }

    pub async fn poll_and_execute(
        &self,
        team_instance_id: Uuid,
    ) -> anyhow::Result<Option<SupervisorResult>> {
        self.ensure_supervisor_agent(team_instance_id).await?;

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

        let triage_result = self.triage(task, team_instance_id).await?;
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

    async fn triage(&self, task: &TeamTask, team_instance_id: Uuid) -> anyhow::Result<TriageResult> {
        let agent_result = {
            let guard = self.supervisor_agent.lock().await;
            match &*guard {
                Some(agent) => Some(agent.triage(&task.goal).await),
                None => {
                    warn!("SupervisorAgent not available - LLM client may not be configured, falling back to rule-based triage");
                    None
                }
            }
        };

        match agent_result {
            Some(Ok(triage_result)) => {
                info!("LLM-based triage completed successfully");
                Ok(triage_result)
            }
            Some(Err(e)) => {
                warn!("LLM-based triage failed: {:?}, falling back to rule-based triage", e);
                self.rule_based_triage_fallback(task, team_instance_id).await
            }
            None => {
                info!("LLM agent unavailable, using rule-based triage");
                self.rule_based_triage_fallback(task, team_instance_id).await
            }
        }
    }

    async fn rule_based_triage_fallback(&self, task: &TeamTask, team_instance_id: Uuid) -> anyhow::Result<TriageResult> {
        let team_member_repo = self.selector_resolver.team_member_repo();
        let members = team_member_repo.list_by_team(team_instance_id, 100).await?;

        let selector = RuleBasedTriage::select_member(task, &members);

        match selector {
            Some(_) => {
                Ok(TriageResult {
                    complexity: TaskComplexity::Simple,
                    processing_path: ProcessingPath::SingleRoute,
                    selected_mode: TeamMode::Route,
                    lead_member_ref: selector.as_ref().and_then(|s| s.role.clone()),
                    rationale: "Rule-based triage: LLM unavailable, used deterministic role-based selection".to_string(),
                })
            }
            None => {
                Err(anyhow::anyhow!("Rule-based triage failed: no members available"))
            }
        }
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
