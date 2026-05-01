//! # Torque Extension System
//!
//! An event-driven, Actor-based extension framework for Torque.
//!
//! ## Architecture
//!
//! - **Actor Model**: Each Extension is an Actor with an isolated mailbox
//! - **Hook System**: Extensions observe or intercept Torque lifecycle events
//! - **Dual-Channel Communication**: Point-to-point (Actor) + Pub/Sub (EventBus)
//! - **Feature-Gated**: Optional dependency in `torque-harness`

pub mod actor;
pub mod builtin;
pub mod bus;
pub mod config;
pub mod context;
pub mod error;
pub mod hook;
pub mod id;
pub mod lifecycle;
pub mod message;
pub mod distributed;
pub mod runtime;
pub mod snapshot;

// Re-export the most common types at the crate root.
pub use actor::ExtensionActor;
pub use builtin::{LoggingExtension, MetricsExtension};
pub use bus::{BusEvent, BusEventHandler, BusTopic, SubscriptionId, TopicRegistry};
pub use config::{ExtensionConfig, ExtensionConfigPatch, ModelConfig, ToolConfig};
pub use context::ExtensionContext;
pub use error::{ExtensionError, Result};
pub use hook::{
    AbortSignal, HookContext, HookHandler, HookInput, HookMode, HookPhase,
    HookPointDef, HookRegistry, HookResult,
    definition::{
        AGENT_END, AGENT_START, CHECKPOINT, CONTEXT, DELEGATION_COMPLETE, DELEGATION_START,
        ERROR, EXECUTION_END, EXECUTION_START, TOOL_CALL, TOOL_RESULT, TURN_END, TURN_START,
        get_hook_def,
    },
    executor::{HookExecutionOutcome, HookExecutor, HookExecutorConfig},
};
pub use id::{ExtensionId, ExtensionVersion};
pub use lifecycle::ExtensionLifecycle;
pub use message::{ExtensionAction, ExtensionMessage, ExtensionResponse, ResponseStatus};
pub use runtime::ExtensionRuntime;
pub use snapshot::{
    ExtensionRegistrySnapshot, ExtensionSnapshot, InMemorySnapshotStorage, SnapshotManager,
    SnapshotMetadata, SnapshotReason, SnapshotStorage,
};

