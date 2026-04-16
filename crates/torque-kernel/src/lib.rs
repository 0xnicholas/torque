//! Core runtime contracts for Torque kernel.

pub mod agent_definition;
pub mod agent_instance;
pub mod approval;
pub mod artifact;
pub mod context_ref;
pub mod delegation;
pub mod engine;
pub mod error;
pub mod execution;
pub mod ids;
pub mod recovery;
pub mod runtime;
pub mod task;
pub mod task_packet;

pub use agent_definition::{AgentDefinition, AgentLimits};
pub use agent_instance::{AgentInstance, AgentInstanceState};
pub use approval::{ApprovalKind, ApprovalRequest, ApprovalState};
pub use artifact::{Artifact, ArtifactBodyRef, ArtifactKind};
pub use context_ref::{AccessMode, ExternalContextKind, ExternalContextRef, SyncPolicy};
pub use delegation::{DelegationRequest, DelegationResult, DelegationState};
pub use engine::{ExecutionEngine, StepDecision};
pub use error::{KernelError, StateTransitionError, ValidationError};
pub use execution::{
    ExecutionEvent, ExecutionMode, ExecutionOutcome, ExecutionRequest, ExecutionResult,
};
pub use ids::{
    AgentDefinitionId, AgentInstanceId, ApprovalRequestId, ArtifactId, DelegationRequestId,
    ExecutionRequestId, ExternalContextRefId, TaskId,
};
pub use recovery::{
    Checkpoint, CheckpointId, CheckpointStateView, RecoveryAction, RecoveryAssessment,
    RecoveryDisposition, RecoveryView,
};
pub use runtime::{
    InMemoryKernelRuntime, InMemoryRuntimeStore, KernelRuntime, ResumeSignal, RuntimeCommand,
    RuntimeStore,
};
pub use task::{ExpectedOutput, Task, TaskConstraint, TaskInputRef, TaskState};
pub use task_packet::TaskPacket;
