pub mod agent_definition;
pub mod agent_instance;
pub mod approval;
pub mod artifact;
pub mod async_runner;
pub mod candidate_generator;
pub mod capability;
pub mod checkpoint;
pub mod context_compaction;
pub mod delegation;
pub mod delegation_packet;
pub mod escalation;
pub mod event;
pub mod event_replay;
pub mod gating;
pub mod governed_tool;
pub mod memory;
pub mod memory_pipeline;
pub mod merge_strategy;
pub mod notification;
pub mod recovery;
pub mod reflexion;
pub mod run;
pub mod task;
pub mod team;
pub mod tool;
pub mod tool_offload;
pub mod vfs;
pub mod webhook_manager;

pub use agent_definition::AgentDefinitionService;
pub use agent_instance::AgentInstanceService;
pub use approval::ApprovalService;
pub use artifact::ArtifactService;
pub use async_runner::AsyncRunner;
pub use candidate_generator::{
    CandidateGenerator, NoOpCandidateGenerator, OpenAICandidateGenerator,
};
pub use capability::CapabilityService;
pub use checkpoint::CheckpointService;
pub use context_compaction::{CompactSummary, ContextCompactionPolicy, ContextCompactionService};
pub use delegation::DelegationService;
pub use delegation_packet::build_delegation_packet;
pub use escalation::EscalationService;
pub use event::EventService;
pub use governed_tool::GovernedToolRegistry;
pub use gating::MemoryGatingService;
pub use memory::MemoryService;
pub use memory_pipeline::MemoryPipelineService;
pub use notification::NotificationService;
pub use recovery::RecoveryService;
pub use reflexion::{
    ExperienceQuery, ReflectionResult, ReflexionService, RetrievedExperience, SubtaskResult,
};
pub use run::RunService;
pub use task::TaskService;
pub use team::{TeamService, TeamSupervisor};
pub use tool::ToolService;
pub use tool_offload::{HarnessOffloadArtifactStore, ToolOffloadConfig, TOOL_OUTPUT_ARTIFACT_KIND};
pub use vfs::RoutedVfs;
pub use webhook_manager::WebhookManager;

use crate::runtime::host::KernelRuntimeHandle;
use crate::runtime::{
    HarnessEventSink, HarnessModelDriver, HarnessToolExecutor, StreamEventSinkAdapter,
};
use crate::repository::EventRepository;
use std::sync::Arc;
use torque_runtime::environment::RuntimeCheckpointSink;

/// Assembles runtime dependencies and creates `KernelRuntimeHandle` instances.
pub struct RuntimeFactory {
    event_repo: Arc<dyn EventRepository>,
    checkpointer: Arc<dyn RuntimeCheckpointSink>,
}

impl RuntimeFactory {
    pub fn new(
        event_repo: Arc<dyn EventRepository>,
        checkpointer: Arc<dyn RuntimeCheckpointSink>,
    ) -> Self {
        Self {
            event_repo,
            checkpointer,
        }
    }

    pub fn create_handle(
        &self,
        agent_defs: Vec<torque_kernel::AgentDefinition>,
    ) -> KernelRuntimeHandle {
        KernelRuntimeHandle::new(
            agent_defs,
            Arc::new(HarnessEventSink::new(self.event_repo.clone())),
            self.checkpointer.clone(),
        )
    }

    pub fn create_model_driver(&self, llm: Arc<dyn llm::LlmClient>) -> HarnessModelDriver {
        HarnessModelDriver::new(llm)
    }

    pub fn create_tool_executor(
        &self,
        governed: Arc<GovernedToolRegistry>,
    ) -> HarnessToolExecutor {
        HarnessToolExecutor::new(governed)
    }

    pub fn create_output_sink(
        &self,
        tx: tokio::sync::mpsc::Sender<crate::agent::stream::StreamEvent>,
    ) -> StreamEventSinkAdapter {
        StreamEventSinkAdapter::new(tx)
    }

    pub fn checkpointer(&self) -> &Arc<dyn RuntimeCheckpointSink> {
        &self.checkpointer
    }
}

pub struct ServiceContainer {
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
    pub run_repo: std::sync::Arc<dyn crate::repository::RunRepository>,
    pub async_runner: std::sync::Arc<AsyncRunner>,
    pub recovery: std::sync::Arc<RecoveryService>,
    pub escalation_service: std::sync::Arc<EscalationService>,
    pub idempotency: std::sync::Arc<crate::v1_guards::IdempotencyStore>,
    pub run_gate: std::sync::Arc<crate::v1_guards::RunGate>,
    pub candidate_generator: std::sync::Arc<dyn candidate_generator::CandidateGenerator>,
    pub gating: std::sync::Arc<gating::MemoryGatingService>,
    pub memory_pipeline: std::sync::Arc<memory_pipeline::MemoryPipelineService>,
    pub notification_service: std::sync::Arc<notification::NotificationService>,
    pub tool_governance: std::sync::Arc<crate::policy::ToolGovernanceService>,
    pub tool_policy: std::sync::Arc<dyn crate::repository::ToolPolicyRepository>,
    pub runtime_factory: Arc<RuntimeFactory>,
}

impl ServiceContainer {
    pub fn new(
        repos: crate::repository::RepositoryContainer,
        memory_v1: std::sync::Arc<dyn crate::repository::MemoryRepositoryV1>,
        checkpointer: std::sync::Arc<dyn RuntimeCheckpointSink>,
        llm: std::sync::Arc<dyn llm::LlmClient>,
        embedding: Option<std::sync::Arc<dyn crate::embedding::EmbeddingGenerator>>,
        idempotency: std::sync::Arc<crate::v1_guards::IdempotencyStore>,
        run_gate: std::sync::Arc<crate::v1_guards::RunGate>,
    ) -> Self {
        let artifact = std::sync::Arc::new(ArtifactService::new(repos.artifact.clone()));
        let tool = std::sync::Arc::new(ToolService::new_with_builtins(artifact.clone()));
        let memory = std::sync::Arc::new(memory::MemoryService::new(
            repos.memory.clone(),
            memory_v1.clone(),
            embedding.clone(),
        ));

        let runtime_factory = Arc::new(RuntimeFactory::new(
            repos.event.clone(),
            checkpointer.clone(),
        ));

        let agent_instance = std::sync::Arc::new(agent_instance::AgentInstanceService::new(
            repos.agent_instance.clone(),
        ));
        let agent_definition =
            std::sync::Arc::new(AgentDefinitionService::new(repos.agent_definition.clone()));
        let task = std::sync::Arc::new(TaskService::new(repos.task.clone()));
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

        let tool_governance = std::sync::Arc::new(crate::policy::ToolGovernanceService::new(
            crate::models::v1::tool_policy::ToolGovernanceConfig {
                default_risk_level: crate::models::v1::tool_policy::ToolRiskLevel::Medium,
                approval_required_above: crate::models::v1::tool_policy::ToolRiskLevel::High,
                blocked_tools: vec![],
                privileged_tools: vec![],
                side_effect_tracking: false,
            },
        ));

        let team_supervisor = std::sync::Arc::new(
            TeamSupervisor::new(
                repos.team_task.clone(),
                repos.delegation.clone(),
                team_selector_resolver,
                team_shared_state_manager,
                team_event_emitter,
                tool_governance.clone(),
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
            None,
        ));
        let notification_service =
            std::sync::Arc::new(notification::NotificationService::new().with_sse_hook());
        let memory_pipeline = std::sync::Arc::new(memory_pipeline::MemoryPipelineService::new(
            gating.clone(),
            Some(notification_service.clone()),
        ));
        let candidate_generator: std::sync::Arc<dyn candidate_generator::CandidateGenerator> =
            {
                let gen = candidate_generator::OpenAICandidateGenerator::new(
                    Arc::new(HarnessModelDriver::new(llm.clone())),
                );
                std::sync::Arc::new(gen)
                    as std::sync::Arc<dyn candidate_generator::CandidateGenerator>
            };
        let run = std::sync::Arc::new(RunService::new(
            repos.agent_definition.clone(),
            repos.agent_instance.clone(),
            repos.task.clone(),
            runtime_factory.clone(),
            llm.clone(),
            tool.clone(),
            tool_governance.clone(),
            candidate_generator.clone(),
            gating.clone(),
            memory_pipeline.clone(),
            None,
        ));
        let run_repo = repos.run.clone();
        let async_runner = std::sync::Arc::new(AsyncRunner::new(repos.run.clone(), run.clone()));
        let escalation_service =
            std::sync::Arc::new(EscalationService::new(repos.escalation.clone()));
        let recovery = std::sync::Arc::new(
            RecoveryService::new(
                repos.agent_instance.clone(),
                repos.checkpoint_ext.clone(),
                repos.event_ext.clone(),
            )
            .with_escalation_service(escalation_service.clone()),
        );
        let tool_policy = repos.tool_policy.clone();

        Self {
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
            run_repo,
            async_runner,
            recovery,
            escalation_service,
            idempotency,
            run_gate,
            candidate_generator,
            gating,
            memory_pipeline,
            notification_service,
            tool_governance,
            tool_policy,
            runtime_factory,
        }
    }
}
