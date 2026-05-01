use crate::{
    bus::{BusEvent, BusTopic},
    config::ExtensionConfigPatch,
    context::{BusEventInternal, PendingMessage},
    id::ExtensionVersion,
    message::ExtensionAction,
};

/// Internal message sent from Extension contexts to the runtime.
///
/// The runtime's background task receives these and routes them to
/// the correct target (another Extension's handle() for Actor messages,
/// or TopicRegistry subscribers for bus events).
#[derive(Debug)]
pub(crate) enum RuntimeMessage {
    /// Point-to-point actor message — route to the target Extension's `handle()`.
    Actor(PendingMessage),
    /// Bus event — dispatch to `TopicRegistry` subscribers.
    Bus(BusEventInternal),
    /// Configuration patch (Layer 2).
    ConfigPatch(ConfigPatchMessage),
}

/// Internal payload for a hot-configuration update (Layer 2).
#[derive(Debug)]
pub(crate) struct ConfigPatchMessage {
    pub extension_id: crate::id::ExtensionId,
    pub patch: ExtensionConfigPatch,
}

/// Payload returned from the target Extension's `handle()` and delivered
/// back to the caller through the oneshot channel embedded in
/// [`PendingMessage`].
pub(crate) type ResponseSender =
    tokio::sync::oneshot::Sender<crate::error::Result<crate::message::ExtensionResponse>>;
