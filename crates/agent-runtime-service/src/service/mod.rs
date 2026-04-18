pub mod agent_definition;
pub mod agent_instance;
pub mod approval;
pub mod artifact;
pub mod capability;
pub mod checkpoint;
pub mod delegation;
pub mod event;
pub mod memory;
pub mod run;
pub mod session;
pub mod task;
pub mod team;
pub mod tool;

pub use agent_definition::AgentDefinitionService;
pub use agent_instance::AgentInstanceService;
pub use approval::ApprovalService;
pub use artifact::ArtifactService;
pub use capability::CapabilityService;
pub use checkpoint::CheckpointService;
pub use delegation::DelegationService;
pub use event::EventService;
pub use memory::MemoryService;
pub use run::RunService;
pub use session::SessionService;
pub use task::TaskService;
pub use team::TeamService;
pub use tool::ToolService;

pub struct ServiceContainer {
    pub session: std::sync::Arc<SessionService>,
    pub memory: std::sync::Arc<memory::MemoryService>,
    pub tool: std::sync::Arc<ToolService>,
    pub agent_instance: std::sync::Arc<agent_instance::AgentInstanceService>,
    pub agent_definition: std::sync::Arc<AgentDefinitionService>,
    pub task: std::sync::Arc<TaskService>,
    pub artifact: std::sync::Arc<ArtifactService>,
    pub capability: std::sync::Arc<CapabilityService>,
    pub team: std::sync::Arc<TeamService>,
    pub delegation: std::sync::Arc<DelegationService>,
    pub approval: std::sync::Arc<ApprovalService>,
    pub checkpoint: std::sync::Arc<CheckpointService>,
    pub event: std::sync::Arc<EventService>,
    pub run: std::sync::Arc<RunService>,
    pub idempotency: std::sync::Arc<crate::v1_guards::IdempotencyStore>,
    pub run_gate: std::sync::Arc<crate::v1_guards::RunGate>,
}

impl ServiceContainer {
    pub fn new(
        repos: crate::repository::RepositoryContainer,
        checkpointer: std::sync::Arc<dyn checkpointer::Checkpointer>,
        llm: std::sync::Arc<dyn llm::LlmClient>,
        idempotency: std::sync::Arc<crate::v1_guards::IdempotencyStore>,
        run_gate: std::sync::Arc<crate::v1_guards::RunGate>,
    ) -> Self {
        let tool = std::sync::Arc::new(ToolService::new());
        let memory = std::sync::Arc::new(memory::MemoryService::new(repos.memory.clone()));
        let session = std::sync::Arc::new(SessionService::new(
            repos.session.clone(),
            repos.message.clone(),
            repos.event.clone(),
            repos.checkpoint.clone(),
            checkpointer.clone(),
            llm.clone(),
            tool.clone(),
            memory.clone(),
        ));
        let agent_instance = std::sync::Arc::new(agent_instance::AgentInstanceService::new(
            repos.agent_instance.clone(),
        ));
        let agent_definition = std::sync::Arc::new(AgentDefinitionService::new(
            repos.agent_definition.clone(),
        ));
        let task = std::sync::Arc::new(TaskService::new(repos.task.clone()));
        let artifact = std::sync::Arc::new(ArtifactService::new(repos.artifact.clone()));
        let capability = std::sync::Arc::new(CapabilityService::new(
            repos.capability_profile.clone(),
            repos.capability_binding.clone(),
        ));
        let team = std::sync::Arc::new(TeamService::new(
            repos.team_definition.clone(),
            repos.team_instance.clone(),
            repos.team_member.clone(),
            repos.task.clone(),
        ));
        let delegation = std::sync::Arc::new(DelegationService::new(repos.delegation.clone()));
        let approval = std::sync::Arc::new(ApprovalService::new(repos.approval.clone()));
        let checkpoint = std::sync::Arc::new(CheckpointService::new(repos.checkpoint_ext.clone()));
        let event = std::sync::Arc::new(EventService::new(repos.event_ext.clone()));
        let run = std::sync::Arc::new(RunService::new(
            repos.agent_definition.clone(),
            repos.agent_instance.clone(),
            repos.task.clone(),
            repos.event.clone(),
            repos.checkpoint.clone(),
            checkpointer,
            llm,
            tool.clone(),
        ));

        Self {
            session, memory, tool, agent_instance, agent_definition,
            task, artifact, capability, team, delegation, approval,
            checkpoint, event, run, idempotency, run_gate,
        }
    }
}
