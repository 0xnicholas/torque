use std::sync::Arc;

pub mod agent_definition;
pub mod agent_instance;
pub mod approval;
pub mod artifact;
pub mod capability;
pub mod checkpoint;
pub mod checkpoint_ext;
pub mod delegation;
pub mod ephemeral_log;
pub mod escalation;
pub mod event;
pub mod event_ext;
pub mod memory;
pub mod memory_v1;
pub mod message;
pub mod rule;
pub mod run;
pub mod session;
pub mod task;
pub mod team;
pub mod tool_policy;

pub use agent_definition::{AgentDefinitionRepository, PostgresAgentDefinitionRepository};
pub use tool_policy::{ToolPolicyRepository, PostgresToolPolicyRepository};
pub use agent_instance::{AgentInstanceRepository, PostgresAgentInstanceRepository};
pub use approval::{ApprovalRepository, PostgresApprovalRepository};
pub use artifact::{ArtifactRepository, PostgresArtifactRepository};
pub use capability::{
    CapabilityProfileRepository, CapabilityRegistryBindingRepository,
    PostgresCapabilityProfileRepository, PostgresCapabilityRegistryBindingRepository,
};
pub use checkpoint::{CheckpointRepository, PostgresCheckpointRepository};
pub use checkpoint_ext::{CheckpointRepositoryExt, PostgresCheckpointRepositoryExt};
pub use delegation::{DelegationRepository, PostgresDelegationRepository};
pub use ephemeral_log::{EphemeralLogRepository, PostgresEphemeralLogRepository};
pub use escalation::{EscalationRepository, PostgresEscalationRepository};
pub use event::{EventRepository, PostgresEventRepository};
pub use event_ext::{EventRepositoryExt, PostgresEventRepositoryExt};
pub use memory::{MemoryRepository, PostgresMemoryRepository};
pub use memory_v1::{MemoryRepositoryV1, PostgresMemoryRepositoryV1};
pub use message::{MessageRepository, PostgresMessageRepository};
pub use rule::{PostgresRuleRepository, RuleRepository};
pub use run::{PostgresRunRepository, RunRepository};
pub use session::{PostgresSessionRepository, SessionKernelState, SessionRepository};
pub use task::{PostgresTaskRepository, TaskRepository};
pub use team::{
    PostgresSharedTaskStateRepository, PostgresTeamDefinitionRepository,
    PostgresTeamEventRepository, PostgresTeamInstanceRepository, PostgresTeamMemberRepository,
    PostgresTeamTaskRepository, SharedTaskStateRepository, TeamDefinitionRepository,
    TeamEventRepository, TeamInstanceRepository, TeamMemberRepository, TeamTaskRepository,
};

pub struct RepositoryContainer {
    pub session: Arc<dyn SessionRepository>,
    pub message: Arc<dyn MessageRepository>,
    pub memory: Arc<dyn MemoryRepository>,
    pub event: Arc<dyn EventRepository>,
    pub checkpoint: Arc<dyn CheckpointRepository>,
    pub agent_definition: Arc<dyn AgentDefinitionRepository>,
    pub agent_instance: Arc<dyn AgentInstanceRepository>,
    pub task: Arc<dyn TaskRepository>,
    pub artifact: Arc<dyn ArtifactRepository>,
    pub capability_profile: Arc<dyn CapabilityProfileRepository>,
    pub capability_binding: Arc<dyn CapabilityRegistryBindingRepository>,
    pub team_definition: Arc<dyn TeamDefinitionRepository>,
    pub team_instance: Arc<dyn TeamInstanceRepository>,
    pub team_member: Arc<dyn TeamMemberRepository>,
    pub team_task: Arc<dyn TeamTaskRepository>,
    pub team_shared_state: Arc<dyn SharedTaskStateRepository>,
    pub team_event: Arc<dyn TeamEventRepository>,
    pub delegation: Arc<dyn DelegationRepository>,
    pub approval: Arc<dyn ApprovalRepository>,
    pub checkpoint_ext: Arc<dyn CheckpointRepositoryExt>,
    pub event_ext: Arc<dyn EventRepositoryExt>,
    pub ephemeral_log: Arc<dyn EphemeralLogRepository>,
    pub rule: Arc<dyn RuleRepository>,
    pub escalation: Arc<dyn EscalationRepository>,
    pub run: Arc<dyn RunRepository>,
    pub tool_policy: Arc<dyn ToolPolicyRepository>,
}
