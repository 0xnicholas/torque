use std::sync::Arc;

use async_trait::async_trait;

use crate::{
    actor::ExtensionActor,
    config::ExtensionConfig,
    context::ExtensionContext,
    error::Result,
    hook::{executor::HookExecutionOutcome, HookInput},
    id::ExtensionId,
    lifecycle::ExtensionLifecycle,
};

/// The runtime that manages Extension lifecycle and message dispatch.
///
/// Implementations provide:
/// - Extension registration / unregistration
/// - Hook dispatch (`HookExecutor` integration)
/// - Actor mailboxes
/// - EventBus fan-out
#[async_trait]
pub trait ExtensionRuntime: Send + Sync {
    /// Register a new Extension.
    ///
    /// This creates the Extension's mailbox, calls `on_start`, and
    /// transitions it to the `Running` state.
    async fn register(
        &self,
        extension: Arc<dyn ExtensionActor>,
        config: ExtensionConfig,
    ) -> Result<ExtensionId>;

    /// Unregister and stop an Extension.
    async fn unregister(&self, id: ExtensionId) -> Result<()>;

    /// Get the context for a registered Extension.
    async fn context(&self, id: ExtensionId) -> Result<ExtensionContext>;

    /// Send a fire-and-forget message to an Extension.
    async fn send(&self, target: ExtensionId, action: crate::message::ExtensionAction) -> Result<()>;

    /// Execute all registered handlers for a hook point.
    ///
    /// This is the primary integration point for the Torque runtime:
    /// at each hook point the Torque runtime calls this method to
    /// let Extensions observe or intercept the flow.
    async fn execute_hook(
        &self,
        hook_name: &'static str,
        input: HookInput,
        agent_id: Option<torque_kernel::AgentInstanceId>,
    ) -> HookExecutionOutcome;

    /// List all registered Extension IDs.
    async fn list(&self) -> Vec<ExtensionId>;

    /// Query the lifecycle state of a registered Extension.
    async fn lifecycle_of(&self, id: ExtensionId) -> Result<ExtensionLifecycle>;

    /// Suspend a running Extension.
    ///
    /// A suspended Extension stops receiving messages and hook dispatches
    /// until it is resumed. This is a no-op if already suspended.
    async fn suspend(&self, id: ExtensionId) -> Result<()>;

    /// Resume a suspended Extension.
    ///
    /// Returns `InvalidState` if the Extension is not in the `Suspended` state.
    async fn resume(&self, id: ExtensionId) -> Result<()>;
}
