use async_trait::async_trait;
use torque_kernel::tool::ToolArc;

use crate::{
    context::ExtensionContext,
    error::Result,
    id::{ExtensionId, ExtensionVersion},
    message::ExtensionMessage,
};

/// The primary trait that every Extension must implement.
///
/// An Extension is an Actor with a well-defined lifecycle:
///
/// 1. The runtime calls [`on_start`] after registration.
/// 2. The runtime dispatches messages via [`handle`].
/// 3. The runtime calls [`on_stop`] before shutdown.
///
/// Extensions register interest in Torque events through
/// [`ExtensionContext::register_hook`] and communicate with peers
/// through [`ExtensionContext::send`] / [`ExtensionContext::call`].
#[async_trait]
pub trait ExtensionActor: Send + Sync {
    /// Unique identifier for this Extension instance.
    fn id(&self) -> ExtensionId;

    /// Human-readable name.
    fn name(&self) -> &'static str;

    /// Semantic version.
    fn version(&self) -> ExtensionVersion;

    /// Called after the Extension is registered and initialized.
    ///
    /// The Extension should use the provided [`ExtensionContext`] to:
    /// - Register hooks via [`ExtensionContext::register_hook`]
    /// - Subscribe to EventBus topics via [`ExtensionContext::subscribe`]
    /// - Perform one-time setup
    async fn on_start(&self, ctx: &ExtensionContext) -> Result<()>;

    /// Called during graceful shutdown.
    ///
    /// The Extension should release any held resources.
    async fn on_stop(&self, ctx: &ExtensionContext) -> Result<()>;

    /// Handle an incoming message from another Extension (Actor Channel).
    ///
    /// The return value is an [`ExtensionResponse`] that will be delivered
    /// back to the sender if the original message was a Request.
    async fn handle(
        &self,
        ctx: &ExtensionContext,
        msg: ExtensionMessage,
    ) -> Result<crate::message::ExtensionResponse>;

    /// Optional: return a list of tools this Extension provides.
    ///
    /// When the Extension is registered, any tools returned here will be
    /// automatically registered into the system's ToolRegistry so that
    /// LLM agents can discover and invoke them.
    ///
    /// The default implementation returns an empty vec, meaning no tools.
    fn tools(&self) -> Vec<ToolArc> {
        Vec::new()
    }
}
