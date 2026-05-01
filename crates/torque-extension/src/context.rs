use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};

use crate::{
    bus::{BusEventHandler, BusTopic, SubscriptionId},
    config::ExtensionConfigPatch,
    error::{ExtensionError, Result},
    hook::{HookHandler, HookRegistry},
    id::{ExtensionId, ExtensionVersion},
    message::{ExtensionAction, ExtensionResponse},
    runtime::mailbox::{ConfigPatchMessage, ResponseSender, RuntimeMessage},
};

/// Runtime handle provided to every Extension.
///
/// All operations on this context are thread-safe. The concrete
/// backing is wired up by the [`ExtensionRuntime`][crate::runtime::ExtensionRuntime]
/// implementation at registration time.
#[derive(Clone)]
pub struct ExtensionContext {
    pub(crate) inner: Arc<ExtensionContextInner>,
}

pub(crate) struct ExtensionContextInner {
    pub(crate) id: ExtensionId,
    pub(crate) version: ExtensionVersion,
    pub(crate) hook_registry: Arc<HookRegistry>,
    /// Single unbounded channel to the runtime's message loop.
    pub(crate) runtime_tx: mpsc::UnboundedSender<RuntimeMessage>,
}

/// Internal message type used to communicate between the context
/// and the runtime's mailbox processing loop.
#[derive(Debug)]
pub(crate) struct PendingMessage {
    pub target: ExtensionId,
    pub action: ExtensionAction,
    pub reply_tx: Option<ResponseSender>,
    pub timeout: std::time::Duration,
}

/// Internal bus event forwarded to the runtime's event loop.
#[derive(Debug)]
pub(crate) struct BusEventInternal {
    pub topic: BusTopic,
    pub payload: serde_json::Value,
}

impl ExtensionContext {
    /// Create a new context (called by the runtime).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        id: ExtensionId,
        version: ExtensionVersion,
        hook_registry: Arc<HookRegistry>,
        runtime_tx: mpsc::UnboundedSender<RuntimeMessage>,
    ) -> Self {
        Self {
            inner: Arc::new(ExtensionContextInner {
                id,
                version,
                hook_registry,
                runtime_tx,
            }),
        }
    }

    // ── Accessors ──────────────────────────────────────────────

    /// Return this Extension's unique identifier.
    pub fn id(&self) -> ExtensionId {
        self.inner.id
    }

    /// Return the current version of this Extension.
    pub fn version(&self) -> ExtensionVersion {
        self.inner.version
    }

    // ── Hook System ────────────────────────────────────────────

    /// Register a hook handler for the given hook point.
    ///
    /// The handler will be invoked **in registration order** when the
    /// Torque runtime emits the corresponding event.
    pub async fn register_hook(
        &self,
        hook: &'static str,
        handler: Arc<dyn HookHandler>,
    ) -> Result<()> {
        self.inner
            .hook_registry
            .register(hook, self.inner.id, handler)
            .await
    }

    /// Unregister all handlers for the given hook point that belong
    /// to this Extension.
    pub async fn unregister_hook(&self, hook: &'static str) -> Result<()> {
        self.inner
            .hook_registry
            .unregister(hook, self.inner.id)
            .await
    }

    // ── Actor Channel (point-to-point) ─────────────────────────

    /// Fire-and-forget: send a message to another Extension.
    ///
    /// No response is expected.
    pub fn send(
        &self,
        target: ExtensionId,
        action: ExtensionAction,
    ) -> Result<()> {
        let msg = PendingMessage {
            target,
            action,
            reply_tx: None,
            timeout: std::time::Duration::from_secs(30),
        };
        self.inner
            .runtime_tx
            .send(RuntimeMessage::Actor(msg))
            .map_err(|_| ExtensionError::RuntimeError("runtime channel closed".into()))
    }

    /// Request-reply: send a message and await the response.
    pub async fn call(
        &self,
        target: ExtensionId,
        action: ExtensionAction,
    ) -> Result<ExtensionResponse> {
        let (tx, rx) = oneshot::channel();
        let msg = PendingMessage {
            target,
            action,
            reply_tx: Some(tx),
            timeout: std::time::Duration::from_secs(30),
        };
        self.inner
            .runtime_tx
            .send(RuntimeMessage::Actor(msg))
            .map_err(|_| ExtensionError::RuntimeError("runtime channel closed".into()))?;

        rx.await
            .map_err(|_| ExtensionError::Timeout(target))?
    }

    // ── EventBus (publish / subscribe) ─────────────────────────

    /// Publish an event on the given topic.
    ///
    /// All Extensions that have subscribed to this topic will receive
    /// the event asynchronously.
    pub fn publish(
        &self,
        topic: BusTopic,
        payload: serde_json::Value,
    ) -> Result<()> {
        self.inner
            .runtime_tx
            .send(RuntimeMessage::Bus(BusEventInternal { topic, payload }))
            .map_err(|_| ExtensionError::RuntimeError("runtime channel closed".into()))
    }

    /// Subscribe to a bus topic.
    ///
    /// Returns a [`SubscriptionId`] that can be used to unsubscribe later.
    pub async fn subscribe(
        &self,
        topic: BusTopic,
        handler: Arc<dyn BusEventHandler>,
    ) -> Result<SubscriptionId> {
        self.inner
            .hook_registry
            .bus_registry()
            .subscribe(topic, self.inner.id, handler)
            .await
    }

    /// Unsubscribe from a bus topic.
    pub async fn unsubscribe(&self, subscription_id: SubscriptionId) -> Result<()> {
        self.inner
            .hook_registry
            .bus_registry()
            .unsubscribe(subscription_id)
            .await
    }

    // ── Configuration ──────────────────────────────────────────

    /// Apply a partial configuration patch (immediate, Layer 2).
    pub fn update_config(&self, patch: ExtensionConfigPatch) -> Result<()> {
        self.inner
            .runtime_tx
            .send(RuntimeMessage::ConfigPatch(ConfigPatchMessage {
                extension_id: self.inner.id,
                patch,
            }))
            .map_err(|_| ExtensionError::RuntimeError("runtime channel closed".into()))
    }

    /// Hot-reload the Extension with a new version (Layer 1).
    pub fn reload(&self, _new_version: ExtensionVersion) -> Result<ExtensionVersion> {
        // Delegated to runtime; Phase 2 will wire this up.
        Ok(self.inner.version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::handler::TestBusHandler;
    use crate::hook::handler::TestHandler;
    use crate::hook::input::HookInput;
    use crate::hook::context::HookContext;
    use crate::hook::handler::HookResult;
    use crate::ResponseStatus;
    use uuid::Uuid;

    fn make_context() -> (ExtensionContext, mpsc::UnboundedReceiver<RuntimeMessage>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let registry = Arc::new(HookRegistry::new());
        let ctx = ExtensionContext::new(
            ExtensionId::new(),
            ExtensionVersion::new(1, 0, 0),
            registry,
            tx,
        );
        (ctx, rx)
    }

    #[test]
    fn test_context_id() {
        let id = ExtensionId::new();
        let (tx, _rx) = mpsc::unbounded_channel();
        let ctx = ExtensionContext::new(
            id,
            ExtensionVersion::new(1, 0, 0),
            Arc::new(HookRegistry::new()),
            tx,
        );
        assert_eq!(ctx.id(), id);
    }

    #[test]
    fn test_context_version() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let ctx = ExtensionContext::new(
            ExtensionId::new(),
            ExtensionVersion::new(2, 1, 0),
            Arc::new(HookRegistry::new()),
            tx,
        );
        assert_eq!(ctx.version(), ExtensionVersion::new(2, 1, 0));
    }

    #[tokio::test]
    async fn test_register_hook_delegates_to_registry() {
        let (ctx, _rx) = make_context();
        let handler: Arc<dyn HookHandler> = Arc::new(TestHandler::always_continue());
        ctx.register_hook("tool_call", handler).await.unwrap();
        // Verify the handler was registered
        let handlers = ctx.inner.hook_registry.get_handlers("tool_call").await;
        assert_eq!(handlers.len(), 1);
        assert_eq!(handlers[0].extension_id, ctx.id());
    }

    #[tokio::test]
    async fn test_unregister_hook_removes_from_registry() {
        let (ctx, _rx) = make_context();
        let handler: Arc<dyn HookHandler> = Arc::new(TestHandler::always_continue());
        ctx.register_hook("tool_call", handler).await.unwrap();
        ctx.unregister_hook("tool_call").await.unwrap();
        let handlers = ctx.inner.hook_registry.get_handlers("tool_call").await;
        assert!(handlers.is_empty());
    }

    #[test]
    fn test_send_sends_actor_message() {
        let (ctx, mut rx) = make_context();
        let target = ExtensionId::new();
        let action = ExtensionAction::Query { key: "status".into() };

        ctx.send(target, action.clone()).unwrap();

        let msg = rx.try_recv().expect("should have received a message");
        match msg {
            RuntimeMessage::Actor(pending) => {
                assert_eq!(pending.target, target);
                assert!(pending.reply_tx.is_none());
            }
            _ => panic!("expected Actor message, got {:?}", msg),
        }
    }

    #[tokio::test]
    async fn test_call_sends_and_receives_response() {
        let (ctx, mut rx) = make_context();
        let target = ExtensionId::new();
        let action = ExtensionAction::Query { key: "status".into() };

        // Spawn a task that sends the response back
        let ctx_clone = ctx.clone();
        tokio::spawn(async move {
            let msg = rx.recv().await.unwrap();
            if let RuntimeMessage::Actor(pending) = msg {
                if let Some(reply_tx) = pending.reply_tx {
                    let response = ExtensionResponse::ok(Uuid::new_v4(), Some(serde_json::json!("alive")));
                    let _ = reply_tx.send(Ok(response));
                }
            }
        });

        let response = ctx_clone.call(target, action).await.unwrap();
        assert!(matches!(response.status, ResponseStatus::Success));
    }

    #[test]
    fn test_publish_sends_bus_message() {
        let (ctx, mut rx) = make_context();
        let topic = BusTopic::from_str("test:event");

        ctx.publish(topic.clone(), serde_json::json!("payload")).unwrap();

        let msg = rx.try_recv().expect("should have received a message");
        match msg {
            RuntimeMessage::Bus(event) => {
                assert_eq!(event.topic, topic);
                assert_eq!(event.payload, "payload");
            }
            _ => panic!("expected Bus message, got {:?}", msg),
        }
    }

    #[tokio::test]
    async fn test_subscribe_delegates_to_bus_registry() {
        let (ctx, _rx) = make_context();
        let topic = BusTopic::from_str("test:sub");
        let handler: Arc<dyn BusEventHandler> = Arc::new(TestBusHandler);

        let sub_id = ctx.subscribe(topic.clone(), handler).await.unwrap();
        let subscribers = ctx.inner.hook_registry.bus_registry().subscribers(&topic).await;
        assert_eq!(subscribers.len(), 1);
        assert_eq!(subscribers[0].extension_id, ctx.id());
    }

    #[tokio::test]
    async fn test_unsubscribe_via_context() {
        let (ctx, _rx) = make_context();
        let topic = BusTopic::from_str("test:unsub");
        let handler: Arc<dyn BusEventHandler> = Arc::new(TestBusHandler);

        let sub_id = ctx.subscribe(topic.clone(), handler).await.unwrap();
        ctx.unsubscribe(sub_id).await.unwrap();
        let subscribers = ctx.inner.hook_registry.bus_registry().subscribers(&topic).await;
        assert!(subscribers.is_empty());
    }

    #[test]
    fn test_update_config_sends_config_patch() {
        let (ctx, mut rx) = make_context();
        let patch = ExtensionConfigPatch {
            settings: Some(serde_json::json!({ "key": "value" })),
            tools: None,
            model: None,
        };

        ctx.update_config(patch).unwrap();

        let msg = rx.try_recv().expect("should have received a message");
        match msg {
            RuntimeMessage::ConfigPatch(config_msg) => {
                assert_eq!(config_msg.extension_id, ctx.id());
                assert!(config_msg.patch.settings.is_some());
            }
            _ => panic!("expected ConfigPatch message, got {:?}", msg),
        }
    }

    #[test]
    fn test_reload_returns_current_version() {
        let (ctx, _rx) = make_context();
        let result = ctx.reload(ExtensionVersion::new(2, 0, 0)).unwrap();
        // Current version is what we set at construction
        assert_eq!(result, ExtensionVersion::new(1, 0, 0));
    }

    #[test]
    fn test_context_clone() {
        let (ctx, _rx) = make_context();
        let cloned = ctx.clone();
        assert_eq!(cloned.id(), ctx.id());
        assert_eq!(cloned.version(), ctx.version());
    }

    #[test]
    fn test_send_returns_error_when_channel_closed() {
        let (tx, rx) = mpsc::unbounded_channel();
        drop(rx); // Close the channel
        let ctx = ExtensionContext::new(
            ExtensionId::new(),
            ExtensionVersion::new(1, 0, 0),
            Arc::new(HookRegistry::new()),
            tx,
        );
        let result = ctx.send(ExtensionId::new(), ExtensionAction::Query { key: "x".into() });
        assert!(result.is_err());
        match result {
            Err(ExtensionError::RuntimeError(_)) => {}
            _ => panic!("expected RuntimeError"),
        }
    }

    #[tokio::test]
    async fn test_call_timeout_on_dropped_channel() {
        let (tx, rx) = mpsc::unbounded_channel();
        drop(rx); // Close the channel — sender is still alive but receiver is gone
        let ctx = ExtensionContext::new(
            ExtensionId::new(),
            ExtensionVersion::new(1, 0, 0),
            Arc::new(HookRegistry::new()),
            tx,
        );
        let result = ctx.call(ExtensionId::new(), ExtensionAction::Query { key: "x".into() }).await;
        assert!(result.is_err());
    }
}
