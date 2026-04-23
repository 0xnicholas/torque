pub mod agent_definition;
pub mod agent_instance;
pub mod approval;
pub mod artifact;
pub mod candidate_generator;
pub mod capability;
pub mod checkpoint;
pub mod delegation;
pub mod event;
pub mod event_replay;
pub mod gating;
pub mod memory;
pub mod memory_pipeline;
pub mod notification;
pub mod recovery;
pub mod reflexion;
pub mod run;
pub mod session;
pub mod task;
pub mod team;
pub mod tool;

pub use agent_definition::AgentDefinitionService;
pub use agent_instance::AgentInstanceService;
pub use approval::ApprovalService;
pub use artifact::ArtifactService;
pub use candidate_generator::{
    CandidateGenerator, NoOpCandidateGenerator, OpenAICandidateGenerator,
};
pub use capability::CapabilityService;
pub use checkpoint::CheckpointService;
pub use delegation::DelegationService;
pub use event::EventService;
pub use gating::MemoryGatingService;
pub use memory::MemoryService;
pub use memory_pipeline::MemoryPipelineService;
pub use notification::NotificationService;
pub use recovery::RecoveryService;
pub use reflexion::{
    ExperienceQuery, ReflectionResult, ReflexionService, RetrievedExperience, SubtaskResult,
};
pub use run::RunService;
pub use session::SessionService;
pub use task::TaskService;
pub use team::{TeamService, TeamSupervisor};
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
    pub team_supervisor: std::sync::Arc<TeamSupervisor>,
    pub delegation: std::sync::Arc<DelegationService>,
    pub approval: std::sync::Arc<ApprovalService>,
    pub checkpoint: std::sync::Arc<CheckpointService>,
    pub event: std::sync::Arc<EventService>,
    pub run: std::sync::Arc<RunService>,
    pub recovery: std::sync::Arc<RecoveryService>,
    pub idempotency: std::sync::Arc<crate::v1_guards::IdempotencyStore>,
    pub run_gate: std::sync::Arc<crate::v1_guards::RunGate>,
    pub candidate_generator: std::sync::Arc<dyn candidate_generator::CandidateGenerator>,
    pub gating: std::sync::Arc<gating::MemoryGatingService>,
    pub memory_pipeline: std::sync::Arc<memory_pipeline::MemoryPipelineService>,
    pub notification_service: std::sync::Arc<notification::NotificationService>,
}

impl ServiceContainer {
    pub fn new(
        repos: crate::repository::RepositoryContainer,
        memory_v1: std::sync::Arc<dyn crate::repository::MemoryRepositoryV1>,
        checkpointer: std::sync::Arc<dyn checkpointer::Checkpointer>,
        llm: std::sync::Arc<dyn llm::LlmClient>,
        embedding: Option<std::sync::Arc<dyn crate::embedding::EmbeddingGenerator>>,
        idempotency: std::sync::Arc<crate::v1_guards::IdempotencyStore>,
        run_gate: std::sync::Arc<crate::v1_guards::RunGate>,
    ) -> Self {
        let tool = std::sync::Arc::new(ToolService::new());
        let memory = std::sync::Arc::new(memory::MemoryService::new(
            repos.memory.clone(),
            memory_v1.clone(),
            embedding.clone(),
        ));
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
        let agent_definition =
            std::sync::Arc::new(AgentDefinitionService::new(repos.agent_definition.clone()));
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
            repos.team_task.clone(),
            repos.team_shared_state.clone(),
            repos.team_event.clone(),
        ));

        let team_selector_resolver = std::sync::Arc::new(team::SelectorResolver::new(
            repos.team_member.clone(),
            repos.agent_instance.clone(),
            repos.capability_profile.clone(),
            repos.capability_binding.clone(),
        ));
        let team_shared_state_manager = std::sync::Arc::new(team::SharedTaskStateManager::new(
            repos.team_shared_state.clone(),
        ));
        let team_event_emitter =
            std::sync::Arc::new(team::TeamEventEmitter::new(repos.team_event.clone()));
        let team_supervisor = std::sync::Arc::new(
            TeamSupervisor::new(
                repos.team_task.clone(),
                repos.delegation.clone(),
                team_selector_resolver,
                team_shared_state_manager,
                team_event_emitter,
            )
            .with_llm(llm.clone()),
        );

        let delegation = std::sync::Arc::new(DelegationService::new(repos.delegation.clone()));
        let approval = std::sync::Arc::new(ApprovalService::new(repos.approval.clone()));
        let checkpoint = std::sync::Arc::new(CheckpointService::new(repos.checkpoint_ext.clone()));
        let event = std::sync::Arc::new(EventService::new(repos.event_ext.clone()));
        let gating = std::sync::Arc::new(gating::MemoryGatingService::new(
            memory_v1.clone(),
            embedding.clone(),
        ));
        let notification_service =
            std::sync::Arc::new(notification::NotificationService::new().with_sse_hook());
        let memory_pipeline = std::sync::Arc::new(memory_pipeline::MemoryPipelineService::new(
            gating.clone(),
            Some(notification_service.clone()),
        ));
        let candidate_generator: std::sync::Arc<dyn candidate_generator::CandidateGenerator> =
            if let Ok(gen) = candidate_generator::OpenAICandidateGenerator::new() {
                std::sync::Arc::new(gen)
                    as std::sync::Arc<dyn candidate_generator::CandidateGenerator>
            } else {
                tracing::warn!(
                    "Failed to initialize OpenAICandidateGenerator, using NoOpCandidateGenerator"
                );
                std::sync::Arc::new(candidate_generator::NoOpCandidateGenerator {})
                    as std::sync::Arc<dyn candidate_generator::CandidateGenerator>
            };
        let run = std::sync::Arc::new(RunService::new(
            repos.agent_definition.clone(),
            repos.agent_instance.clone(),
            repos.task.clone(),
            repos.event.clone(),
            repos.checkpoint.clone(),
            checkpointer,
            llm,
            tool.clone(),
            candidate_generator.clone(),
            gating.clone(),
            memory_pipeline.clone(),
            None,
        ));
        let recovery = std::sync::Arc::new(RecoveryService::new(
            repos.agent_instance.clone(),
            repos.checkpoint_ext.clone(),
            repos.event_ext.clone(),
        ));

        Self {
            session,
            memory,
            tool,
            agent_instance,
            agent_definition,
            task,
            artifact,
            capability,
            team,
            team_supervisor,
            delegation,
            approval,
            checkpoint,
            event,
            run,
            recovery,
            idempotency,
            run_gate,
            candidate_generator,
            gating,
            memory_pipeline,
            notification_service,
        }
    }
}
