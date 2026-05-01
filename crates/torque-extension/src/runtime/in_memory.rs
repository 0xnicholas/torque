use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use tokio::sync::{mpsc, RwLock};
use torque_kernel::AgentInstanceId;

use crate::{
    actor::ExtensionActor,
    bus::{BusEvent, TopicRegistry},
    config::ExtensionConfig,
    context::{BusEventInternal, ExtensionContext, PendingMessage},
    error::{ExtensionError, Result},
    hook::{
        executor::{HookExecutionOutcome, HookExecutor, HookExecutorConfig},
        input::HookInput,
        HookRegistry,
    },
    id::ExtensionId,
    lifecycle::ExtensionLifecycle,
    message::{ExtensionAction, ExtensionMessage},
    runtime::{
        mailbox::{ConfigPatchMessage, RuntimeMessage},
        snapshot::ExtensionSnapshot,
        ExtensionRuntime,
    },
};

// ── Entry ──────────────────────────────────────────────────────────────

/// Per-Extension state kept in the runtime registry.
struct ExtensionEntry {
    extension: Arc<dyn ExtensionActor>,
    context: ExtensionContext,
    name: &'static str,
    config: Mutex<ExtensionConfig>,
    lifecycle: RwLock<ExtensionLifecycle>,
    /// Handle for the background message-processing task.
    _task: tokio::task::JoinHandle<()>,
}

// ── Public runtime ─────────────────────────────────────────────────────

/// In-memory implementation of [`ExtensionRuntime`].
///
/// All Extensions run inside the same process.  Messages are delivered
/// synchronously (with back-pressure) through the background processing
/// loop.  Hook dispatch and EventBus fan-out happen inline.
pub struct InMemoryExtensionRuntime {
    inner: Arc<Inner>,
    mailbox_tx: mpsc::UnboundedSender<RuntimeMessage>,
    /// Keep the background task alive — dropped on shutdown.
    _task: tokio::task::JoinHandle<()>,
}

struct Inner {
    registry: RwLock<HashMap<ExtensionId, ExtensionEntry>>,
    hook_registry: Arc<HookRegistry>,
    hook_executor: HookExecutor,
    bus_registry: Arc<TopicRegistry>,
    name_index: RwLock<HashMap<String, ExtensionId>>,
}

impl InMemoryExtensionRuntime {
    /// Create a new empty runtime.
    pub fn new() -> Self {
        Self::with_config(HookExecutorConfig::default())
    }

    /// Create a new runtime with a custom hook executor config.
    pub fn with_config(hook_config: HookExecutorConfig) -> Self {
        let hook_registry = Arc::new(HookRegistry::new());
        let bus_registry = Arc::new(TopicRegistry::new());
        let (mailbox_tx, mailbox_rx) = mpsc::unbounded_channel();

        let inner = Arc::new(Inner {
            registry: RwLock::new(HashMap::new()),
            hook_registry: hook_registry.clone(),
            hook_executor: HookExecutor::with_config(hook_registry, hook_config),
            bus_registry,
            name_index: RwLock::new(HashMap::new()),
        });

        let task_inner = inner.clone();
        let task = tokio::spawn(async move {
            Self::mailbox_loop(task_inner, mailbox_rx).await;
        });

        Self {
            inner,
            mailbox_tx,
            _task: task,
        }
    }

    /// Look up an ExtensionId by its human-readable name.
    pub async fn find_by_name(&self, name: &str) -> Option<ExtensionId> {
        self.inner.name_index.read().await.get(name).copied()
    }

    /// Look up the name for a given ExtensionId.
    pub async fn name_for_id(&self, id: ExtensionId) -> Option<String> {
        let index = self.inner.name_index.read().await;
        for (name, &eid) in index.iter() {
            if eid == id {
                return Some(name.clone());
            }
        }
        None
    }

    /// Take a snapshot of an Extension's runtime state.
    ///
    /// Returns [`ExtensionError::NotFound`] if the Extension is not registered.
    pub async fn snapshot(&self, id: ExtensionId) -> crate::error::Result<ExtensionSnapshot> {
        let registry = self.inner.registry.read().await;
        let entry = registry.get(&id).ok_or(ExtensionError::NotFound(id))?;

        // Collect hook registrations for this extension.
        let hooks = self.inner.hook_registry.hooks_for_extension(id).await;

        // Collect bus subscriptions for this extension.
        let subs = self.inner.hook_registry.bus_registry().subscriptions_for_extension(id).await;

        let name = entry.name;
        let version = entry.extension.version();
        let lifecycle = *entry.lifecycle.read().await;
        let config = entry.config.lock().unwrap().clone();
        // Drop registry lock before constructing the result.
        drop(registry);

        Ok(ExtensionSnapshot::new(
            id,
            name,
            version,
            lifecycle,
            config,
            hooks.into_iter().map(|h| h.to_string()).collect(),
            subs,
        ))
    }

    /// Background loop: drain the global mailbox and dispatch messages.
    async fn mailbox_loop(inner: Arc<Inner>, mut rx: mpsc::UnboundedReceiver<RuntimeMessage>) {
        while let Some(msg) = rx.recv().await {
            match msg {
                RuntimeMessage::Actor(pending) => {
                    Self::dispatch_actor(&inner, pending).await;
                }
                RuntimeMessage::Bus(event) => {
                    Self::dispatch_bus(&inner, event).await;
                }
                RuntimeMessage::ConfigPatch(patch) => {
                    Self::dispatch_config_patch(&inner, patch).await;
                }
            }
        }
    }

    // ── Message dispatchers ───────────────────────────────────────

    async fn dispatch_actor(inner: &Inner, msg: PendingMessage) {
        let registry = inner.registry.read().await;
        let Some(entry) = registry.get(&msg.target) else {
            // Target not found — if this was a call, notify the sender.
            if let Some(tx) = msg.reply_tx {
                let _ = tx.send(Err(ExtensionError::TargetNotFound(msg.target)));
            }
            return;
        };

        // Skip message delivery if the extension is suspended or stopped.
        if !entry.lifecycle.read().await.is_active() {
            if let Some(tx) = msg.reply_tx {
                let _ = tx.send(Err(ExtensionError::InvalidState(msg.target)));
            }
            return;
        }

        let ext_msg = ExtensionMessage::Command {
            target: msg.target,
            action: msg.action,
        };
        let result = entry
            .extension
            .handle(&entry.context, ext_msg)
            .await;

        if let Some(tx) = msg.reply_tx {
            let _ = tx.send(result);
        }
    }

    async fn dispatch_bus(inner: &Inner, event: BusEventInternal) {
        let subscribers = inner.bus_registry.subscribers(&event.topic).await;
        for sub in &subscribers {
            let bus_event = BusEvent {
                id: uuid::Uuid::new_v4(),
                topic: event.topic.clone(),
                source: sub.extension_id,
                timestamp: chrono::Utc::now(),
                payload: event.payload.clone(),
            };
            sub.handler.handle(&bus_event).await;
        }
    }

    async fn dispatch_config_patch(inner: &Inner, msg: ConfigPatchMessage) {
        let registry = inner.registry.read().await;
        let Some(entry) = registry.get(&msg.extension_id) else {
            return;
        };

        let mut config = entry.config.lock().unwrap();
        let patch = &msg.patch;

        if let Some(settings) = &patch.settings {
            config.settings = settings.clone();
        }
        if let Some(tools) = &patch.tools {
            for (name, tool_cfg) in tools {
                config.tools.insert(name.clone(), tool_cfg.clone());
            }
        }
        if let Some(model) = &patch.model {
            config.model = Some(model.clone());
        }
    }

    /// Spawn a per-Extension message processing loop.
    ///
    /// The returned sender can be used to forward actor messages directly
    /// to this Extension's `handle()` from the dedicated loop, avoiding
    /// the global dispatch bottleneck for high-traffic Extensions.
    #[allow(unused)]
    fn spawn_extension_task(
        extension: Arc<dyn ExtensionActor>,
        ctx: ExtensionContext,
    ) -> mpsc::UnboundedSender<RuntimeMessage> {
        let (tx, mut rx) = mpsc::unbounded_channel::<RuntimeMessage>();

        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if let RuntimeMessage::Actor(pending) = msg {
                    let ext_msg = ExtensionMessage::Command {
                        target: pending.target,
                        action: pending.action,
                    };
                    let result = extension.handle(&ctx, ext_msg).await;
                    if let Some(reply_tx) = pending.reply_tx {
                        let _ = reply_tx.send(result);
                    }
                }
            }
        });

        tx
    }
}

impl Default for InMemoryExtensionRuntime {
    fn default() -> Self {
        Self::new()
    }
}

// ── ExtensionRuntime trait implementation ──────────────────────────────

#[async_trait]
impl ExtensionRuntime for InMemoryExtensionRuntime {
    async fn register(
        &self,
        extension: Arc<dyn ExtensionActor>,
        config: ExtensionConfig,
    ) -> Result<ExtensionId> {
        let id = extension.id();

        // Guard: prevent duplicate registration.
        {
            let registry = self.inner.registry.read().await;
            if registry.contains_key(&id) {
                return Err(ExtensionError::AlreadyRegistered(id));
            }
        }

        // Create the ExtensionContext.
        let ctx = ExtensionContext::new(
            id,
            extension.version(),
            self.inner.hook_registry.clone(),
            self.mailbox_tx.clone(),
        );

        // Call on_start — the Extension typically registers hooks here.
        extension.on_start(&ctx).await?;

        // Spawn the per-Extension inbox task (reserved for future
        // dedicated message routing — currently unused).
        let _dedicated_tx = Self::spawn_extension_task(extension.clone(), ctx.clone());

        // Record the version before moving `extension` into the entry.
        let ext_version = extension.version().to_string();
        let ext_name = extension.name();

        let entry = ExtensionEntry {
            extension,
            context: ctx.clone(),
            name: ext_name,
            config: Mutex::new(config),
            lifecycle: RwLock::new(ExtensionLifecycle::Running),
            _task: tokio::spawn(async move {
                // No-op holder — keeps the JoinHandle alive.
                std::future::pending::<()>().await;
            }),
        };

        {
            let mut registry = self.inner.registry.write().await;
            registry.insert(id, entry);
        }

        {
            let mut name_index = self.inner.name_index.write().await;
            name_index.insert(ext_name.to_string(), id);
        }

        // Publish registration event.
        Self::dispatch_bus(
            &self.inner,
            BusEventInternal {
                topic: crate::bus::BusTopic::from_str("ext:registered"),
                payload: serde_json::json!({
                    "id": id.to_string(),
                    "version": ext_version,
                }),
            },
        )
        .await;

        Ok(id)
    }

    async fn unregister(&self, id: ExtensionId) -> Result<()> {
        let entry = {
            let mut registry = self.inner.registry.write().await;
            registry.remove(&id)
        };

        // Clean up name index regardless of whether the entry existed.
        {
            let mut name_index = self.inner.name_index.write().await;
            name_index.retain(|_, v| *v != id);
        }

        match entry {
            Some(entry) => {
                {
                    let mut lifecycle = entry.lifecycle.write().await;
                    *lifecycle = ExtensionLifecycle::Stopped;
                }
                entry.extension.on_stop(&entry.context).await?;

                Self::dispatch_bus(
                    &self.inner,
                    BusEventInternal {
                        topic: crate::bus::BusTopic::from_str("ext:unregistered"),
                        payload: serde_json::json!({ "id": id.to_string() }),
                    },
                )
                .await;

                Ok(())
            }
            None => Err(ExtensionError::NotFound(id)),
        }
    }

    async fn context(&self, id: ExtensionId) -> Result<ExtensionContext> {
        let registry = self.inner.registry.read().await;
        registry
            .get(&id)
            .map(|entry| entry.context.clone())
            .ok_or(ExtensionError::NotFound(id))
    }

    async fn send(&self, target: ExtensionId, action: ExtensionAction) -> Result<()> {
        let msg = PendingMessage {
            target,
            action,
            reply_tx: None,
            timeout: std::time::Duration::from_secs(30),
        };
        self.mailbox_tx
            .send(RuntimeMessage::Actor(msg))
            .map_err(|_| ExtensionError::RuntimeError("runtime channel closed".into()))
    }

    async fn execute_hook(
        &self,
        hook_name: &'static str,
        input: HookInput,
        agent_id: Option<AgentInstanceId>,
    ) -> HookExecutionOutcome {
        self.inner
            .hook_executor
            .execute(hook_name, input, agent_id)
            .await
    }

    async fn list(&self) -> Vec<ExtensionId> {
        let registry = self.inner.registry.read().await;
        registry.keys().copied().collect()
    }

    async fn lifecycle_of(&self, id: ExtensionId) -> Result<ExtensionLifecycle> {
        let registry = self.inner.registry.read().await;
        match registry.get(&id) {
            Some(entry) => Ok(*entry.lifecycle.read().await),
            None => Err(ExtensionError::NotFound(id)),
        }
    }

    async fn suspend(&self, id: ExtensionId) -> Result<()> {
        let registry = self.inner.registry.read().await;
        let entry = registry.get(&id).ok_or(ExtensionError::NotFound(id))?;

        let mut lifecycle = entry.lifecycle.write().await;
        if *lifecycle == ExtensionLifecycle::Suspended {
            return Ok(()); // Already suspended — no-op.
        }
        if !lifecycle.can_transition_to(ExtensionLifecycle::Suspended) {
            return Err(ExtensionError::LifecycleError(format!(
                "cannot suspend extension from state: {lifecycle}"
            )));
        }
        *lifecycle = ExtensionLifecycle::Suspended;

        // Notify the extension via on_stop (suspended extensions stop processing).
        entry.extension.on_stop(&entry.context).await?;

        Self::dispatch_bus(
            &self.inner,
            BusEventInternal {
                topic: crate::bus::BusTopic::from_str("ext:suspended"),
                payload: serde_json::json!({ "id": id.to_string() }),
            },
        )
        .await;

        Ok(())
    }

    async fn resume(&self, id: ExtensionId) -> Result<()> {
        let registry = self.inner.registry.read().await;
        let entry = registry.get(&id).ok_or(ExtensionError::NotFound(id))?;

        let mut lifecycle = entry.lifecycle.write().await;
        if *lifecycle != ExtensionLifecycle::Suspended {
            return Err(ExtensionError::LifecycleError(format!(
                "cannot resume extension from state: {lifecycle}"
            )));
        }
        if !lifecycle.can_transition_to(ExtensionLifecycle::Running) {
            return Err(ExtensionError::LifecycleError(format!(
                "cannot resume extension from state: {lifecycle}"
            )));
        }
        *lifecycle = ExtensionLifecycle::Running;

        // Restart the extension via on_start.
        entry.extension.on_start(&entry.context).await?;

        Self::dispatch_bus(
            &self.inner,
            BusEventInternal {
                topic: crate::bus::BusTopic::from_str("ext:resumed"),
                payload: serde_json::json!({ "id": id.to_string() }),
            },
        )
        .await;

        Ok(())
    }
}

// ── Helpers ────────────────────────────────────────────────────────────

/// No-op handler used internally.
struct DiscardHandler;

#[async_trait]
impl crate::bus::BusEventHandler for DiscardHandler {
    async fn handle(&self, _event: &BusEvent) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actor::ExtensionActor;
    use crate::hook::handler::HookResult;
    use crate::hook::context::HookContext;
    use crate::hook::input::HookInput;
    use crate::hook::handler::HookHandler;
    use crate::message::{ExtensionAction, ExtensionMessage, ExtensionResponse, ResponseStatus};
    use crate::ExtensionVersion;

    struct TestActor {
        id: ExtensionId,
        version: ExtensionVersion,
        started: std::sync::atomic::AtomicBool,
        stopped: std::sync::atomic::AtomicBool,
        last_message: std::sync::Mutex<Option<ExtensionMessage>>,
    }

    impl TestActor {
        fn new() -> Self {
            Self {
                id: ExtensionId::new(),
                version: ExtensionVersion::new(1, 0, 0),
                started: std::sync::atomic::AtomicBool::new(false),
                stopped: std::sync::atomic::AtomicBool::new(false),
                last_message: std::sync::Mutex::new(None),
            }
        }
    }

    #[async_trait]
    impl ExtensionActor for TestActor {
        fn id(&self) -> ExtensionId { self.id }
        fn name(&self) -> &'static str { "TestActor" }
        fn version(&self) -> ExtensionVersion { self.version }

        async fn on_start(&self, _ctx: &ExtensionContext) -> Result<()> {
            self.started.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }

        async fn on_stop(&self, _ctx: &ExtensionContext) -> Result<()> {
            self.stopped.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }

        async fn handle(&self, _ctx: &ExtensionContext, msg: ExtensionMessage) -> Result<ExtensionResponse> {
            *self.last_message.lock().unwrap() = Some(msg);
            Ok(ExtensionResponse {
                request_id: uuid::Uuid::new_v4(),
                status: ResponseStatus::Success,
                result: Some(serde_json::json!("handled")),
            })
        }
    }

    #[tokio::test]
    async fn test_register_and_list() {
        let runtime = InMemoryExtensionRuntime::new();
        let actor = Arc::new(TestActor::new());

        let id = runtime.register(actor.clone(), ExtensionConfig::default()).await.unwrap();
        assert_eq!(id, actor.id);

        let list = runtime.list().await;
        assert_eq!(list.len(), 1);
        assert!(list.contains(&actor.id));
    }

    #[tokio::test]
    async fn test_register_triggers_on_start() {
        let runtime = InMemoryExtensionRuntime::new();
        let actor = Arc::new(TestActor::new());

        runtime.register(actor.clone(), ExtensionConfig::default()).await.unwrap();
        assert!(actor.started.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_unregister_removes_extension() {
        let runtime = InMemoryExtensionRuntime::new();
        let actor = Arc::new(TestActor::new());

        let id = runtime.register(actor.clone(), ExtensionConfig::default()).await.unwrap();
        runtime.unregister(id).await.unwrap();

        let list = runtime.list().await;
        assert!(list.is_empty());
    }

    #[tokio::test]
    async fn test_unregister_triggers_on_stop() {
        let runtime = InMemoryExtensionRuntime::new();
        let actor = Arc::new(TestActor::new());

        let id = runtime.register(actor.clone(), ExtensionConfig::default()).await.unwrap();
        runtime.unregister(id).await.unwrap();
        assert!(actor.stopped.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_duplicate_registration_returns_error() {
        let runtime = InMemoryExtensionRuntime::new();
        let actor = Arc::new(TestActor::new());

        runtime.register(actor.clone(), ExtensionConfig::default()).await.unwrap();
        let result = runtime.register(actor.clone(), ExtensionConfig::default()).await;
        assert!(result.is_err());
        match result {
            Err(ExtensionError::AlreadyRegistered(id)) => assert_eq!(id, actor.id),
            _ => panic!("expected AlreadyRegistered error"),
        }
    }

    #[tokio::test]
    async fn test_unregister_nonexistent_returns_error() {
        let runtime = InMemoryExtensionRuntime::new();
        let result = runtime.unregister(ExtensionId::new()).await;
        assert!(result.is_err());
        match result {
            Err(ExtensionError::NotFound(_)) => {}
            _ => panic!("expected NotFound error"),
        }
    }

    #[tokio::test]
    async fn test_send_message_to_extension() {
        let runtime = InMemoryExtensionRuntime::new();
        let actor = Arc::new(TestActor::new());
        let id = runtime.register(actor.clone(), ExtensionConfig::default()).await.unwrap();

        runtime.send(id, ExtensionAction::Query { key: "status".into() }).await.unwrap();

        // Give the mailbox loop time to process
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let last = actor.last_message.lock().unwrap().take();
        assert!(last.is_some());
    }

    #[tokio::test]
    async fn test_context_lookup() {
        let runtime = InMemoryExtensionRuntime::new();
        let actor = Arc::new(TestActor::new());
        let id = runtime.register(actor.clone(), ExtensionConfig::default()).await.unwrap();

        let ctx = runtime.context(id).await.unwrap();
        assert_eq!(ctx.id(), id);
    }

    #[tokio::test]
    async fn test_context_lookup_nonexistent() {
        let runtime = InMemoryExtensionRuntime::new();
        let result = runtime.context(ExtensionId::new()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_hook_with_no_handlers() {
        let runtime = InMemoryExtensionRuntime::new();

        let outcome = runtime.execute_hook(
            "tool_call",
            HookInput::ToolCall {
                tool: serde_json::json!("test"),
                args: serde_json::json!({}),
            },
            None,
        ).await;

        match outcome {
            HookExecutionOutcome::Passed(_) => {}
            _ => panic!("expected Passed, got {:?}", outcome),
        }
    }

    #[tokio::test]
    async fn test_execute_hook_with_registered_handler() {
        let runtime = InMemoryExtensionRuntime::new();

        // Register a test extension that registers a hook handler
        let actor = Arc::new(TestActor::new());
        let id = runtime.register(actor.clone(), ExtensionConfig::default()).await.unwrap();

        // Register a hook handler via the context
        struct PassthroughHandler;
        #[async_trait]
        impl HookHandler for PassthroughHandler {
            async fn handle(&self, _ctx: &HookContext, input: &HookInput) -> HookResult {
                HookResult::Modified(HookInput::ToolCall {
                    tool: serde_json::json!("modified"),
                    args: serde_json::json!({}),
                })
            }
        }

        let ctx = runtime.context(id).await.unwrap();
        ctx.register_hook("tool_call", Arc::new(PassthroughHandler)).await.unwrap();

        let outcome = runtime.execute_hook(
            "tool_call",
            HookInput::ToolCall {
                tool: serde_json::json!("original"),
                args: serde_json::json!({}),
            },
            None,
        ).await;

        match outcome {
            HookExecutionOutcome::Passed(HookInput::ToolCall { tool, .. }) => {
                assert_eq!(tool, "modified");
            }
            _ => panic!("expected Passed with modified input, got {:?}", outcome),
        }
    }

    #[tokio::test]
    async fn test_lifecycle_of_returns_running_after_register() {
        let runtime = InMemoryExtensionRuntime::new();
        let actor = Arc::new(TestActor::new());
        let id = runtime.register(actor.clone(), ExtensionConfig::default()).await.unwrap();

        let state = runtime.lifecycle_of(id).await.unwrap();
        assert_eq!(state, ExtensionLifecycle::Running);
    }

    #[tokio::test]
    async fn test_lifecycle_of_nonexistent_returns_not_found() {
        let runtime = InMemoryExtensionRuntime::new();
        let result = runtime.lifecycle_of(ExtensionId::new()).await;
        assert!(result.is_err());
        assert!(matches!(result, Err(ExtensionError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_suspend_changes_lifecycle_to_suspended() {
        let runtime = InMemoryExtensionRuntime::new();
        let actor = Arc::new(TestActor::new());
        let id = runtime.register(actor.clone(), ExtensionConfig::default()).await.unwrap();
        assert!(actor.started.load(std::sync::atomic::Ordering::SeqCst));

        runtime.suspend(id).await.unwrap();

        let state = runtime.lifecycle_of(id).await.unwrap();
        assert_eq!(state, ExtensionLifecycle::Suspended);
        // on_stop should have been called
        assert!(actor.stopped.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_suspend_already_suspended_is_noop() {
        let runtime = InMemoryExtensionRuntime::new();
        let actor = Arc::new(TestActor::new());
        let id = runtime.register(actor.clone(), ExtensionConfig::default()).await.unwrap();

        runtime.suspend(id).await.unwrap();
        // Second suspend should be a no-op (idempotent)
        runtime.suspend(id).await.unwrap();

        let state = runtime.lifecycle_of(id).await.unwrap();
        assert_eq!(state, ExtensionLifecycle::Suspended);
    }

    #[tokio::test]
    async fn test_resume_restores_running_state() {
        let runtime = InMemoryExtensionRuntime::new();
        let actor = Arc::new(TestActor::new());
        let id = runtime.register(actor.clone(), ExtensionConfig::default()).await.unwrap();

        runtime.suspend(id).await.unwrap();
        // Reset stopped flag to verify on_start is called again
        actor.stopped.store(false, std::sync::atomic::Ordering::SeqCst);
        runtime.resume(id).await.unwrap();

        let state = runtime.lifecycle_of(id).await.unwrap();
        assert_eq!(state, ExtensionLifecycle::Running);
        // on_start should have been called again
        assert!(actor.started.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_resume_from_running_returns_error() {
        let runtime = InMemoryExtensionRuntime::new();
        let actor = Arc::new(TestActor::new());
        let id = runtime.register(actor.clone(), ExtensionConfig::default()).await.unwrap();

        let result = runtime.resume(id).await;
        assert!(result.is_err());
        assert!(matches!(result, Err(ExtensionError::LifecycleError(_))));
    }

    #[tokio::test]
    async fn test_suspend_nonexistent_returns_not_found() {
        let runtime = InMemoryExtensionRuntime::new();
        let result = runtime.suspend(ExtensionId::new()).await;
        assert!(matches!(result, Err(ExtensionError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_find_by_name_returns_id() {
        let runtime = InMemoryExtensionRuntime::new();
        let actor = Arc::new(TestActor::new());
        let id = runtime.register(actor.clone(), ExtensionConfig::default()).await.unwrap();

        let found = runtime.find_by_name("TestActor").await;
        assert_eq!(found, Some(id));
    }

    #[tokio::test]
    async fn test_find_by_name_nonexistent() {
        let runtime = InMemoryExtensionRuntime::new();
        let found = runtime.find_by_name("nonexistent").await;
        assert_eq!(found, None);
    }

    #[tokio::test]
    async fn test_name_for_id_returns_name() {
        let runtime = InMemoryExtensionRuntime::new();
        let actor = Arc::new(TestActor::new());
        let id = runtime.register(actor.clone(), ExtensionConfig::default()).await.unwrap();

        let name = runtime.name_for_id(id).await;
        assert_eq!(name, Some("TestActor".to_string()));
    }

    #[tokio::test]
    async fn test_name_for_id_unregistered() {
        let runtime = InMemoryExtensionRuntime::new();
        let name = runtime.name_for_id(ExtensionId::new()).await;
        assert_eq!(name, None);
    }

    #[tokio::test]
    async fn test_snapshot_returns_state() {
        let runtime = InMemoryExtensionRuntime::new();
        let actor = Arc::new(TestActor::new());
        let id = runtime.register(actor.clone(), ExtensionConfig::default()).await.unwrap();

        let snap = runtime.snapshot(id).await.unwrap();
        assert_eq!(snap.id, id);
        assert_eq!(snap.name, "TestActor");
        assert_eq!(snap.lifecycle, ExtensionLifecycle::Running);
        assert!(snap.registered_hooks.is_empty());
        assert!(snap.bus_subscriptions.is_empty());
    }

    #[tokio::test]
    async fn test_snapshot_includes_hooks_and_subscriptions() {
        let runtime = InMemoryExtensionRuntime::new();
        let actor = Arc::new(TestActor::new());
        let id = runtime.register(actor.clone(), ExtensionConfig::default()).await.unwrap();

        // Register a hook
        let ctx = runtime.context(id).await.unwrap();
        struct TestHandler;
        #[async_trait]
        impl HookHandler for TestHandler {
            async fn handle(&self, _ctx: &HookContext, _input: &HookInput) -> HookResult {
                HookResult::Continue
            }
        }
        ctx.register_hook("tool_call", Arc::new(TestHandler)).await.unwrap();
        ctx.register_hook("turn_start", Arc::new(TestHandler)).await.unwrap();

        // Subscribe to a topic
        use crate::bus::BusTopic;
        let bus_handler = Arc::new(crate::bus::handler::TestBusHandler::default());
        ctx.subscribe(BusTopic::from_str("test:events"), bus_handler).await.unwrap();

        let snap = runtime.snapshot(id).await.unwrap();
        assert_eq!(snap.registered_hooks.len(), 2);
        assert!(snap.registered_hooks.contains(&"tool_call".to_string()));
        assert!(snap.registered_hooks.contains(&"turn_start".to_string()));
        assert_eq!(snap.bus_subscriptions.len(), 1);
        assert!(snap.bus_subscriptions[0].contains("test:events"));
    }

    #[tokio::test]
    async fn test_suspend_prevents_message_delivery() {
        let runtime = InMemoryExtensionRuntime::new();
        let actor = Arc::new(TestActor::new());
        let id = runtime.register(actor.clone(), ExtensionConfig::default()).await.unwrap();

        // Suspend the extension
        runtime.suspend(id).await.unwrap();

        // Send a message — should be silently dropped (extension is suspended)
        runtime.send(id, ExtensionAction::Query { key: "ping".into() }).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Message should NOT have been delivered
        let last = actor.last_message.lock().unwrap().take();
        assert!(last.is_none(), "suspended extension should not receive messages");
    }

    #[tokio::test]
    async fn test_resume_reenables_message_delivery() {
        let runtime = InMemoryExtensionRuntime::new();
        let actor = Arc::new(TestActor::new());
        let id = runtime.register(actor.clone(), ExtensionConfig::default()).await.unwrap();

        // Suspend, then resume
        runtime.suspend(id).await.unwrap();
        runtime.resume(id).await.unwrap();

        // Now send a message — should be delivered
        runtime.send(id, ExtensionAction::Query { key: "ping".into() }).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let last = actor.last_message.lock().unwrap().take();
        assert!(last.is_some(), "resumed extension should receive messages");
    }
}
