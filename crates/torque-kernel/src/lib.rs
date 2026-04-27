//! Core execution contracts for the Torque kernel.
//!
//! The kernel owns stable execution semantics. Concrete production runtime
//! environments are expected to sit above this layer.

pub mod agent_definition;
pub mod agent_instance;
pub mod context_ref;
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
pub use context_ref::{AccessMode, ExternalContextKind, ExternalContextRef, SyncPolicy};
pub use engine::{ExecutionEngine, StepDecision};
pub use error::{KernelError, StateTransitionError, ValidationError};
pub use execution::{
    ExecutionEvent, ExecutionMode, ExecutionOutcome, ExecutionRequest, ExecutionResult,
};
pub use ids::{
    AgentDefinitionId, AgentInstanceId, ApprovalRequestId, ArtifactId, CheckpointId,
    DelegationRequestId, ExecutionRequestId, ExternalContextRefId, TaskId,
};
pub use recovery::{
    Checkpoint, CheckpointStateView, RecoveryAction, RecoveryAssessment,
    RecoveryDisposition, RecoveryView,
};
pub use runtime::{
    InMemoryKernelRuntime, InMemoryRuntimeStore, KernelRuntime, ResumeSignal, RuntimeCommand,
    RuntimeStore,
};
pub use task::{ExpectedOutput, Task, TaskConstraint, TaskInputRef, TaskState};
pub use task_packet::TaskPacket;
