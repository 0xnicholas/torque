use std::collections::HashMap;
use std::sync::Arc;

use crate::{
    bus::TopicRegistry,
    error::Result,
    hook::definition::HookHandlerEntry,
    id::ExtensionId,
};

use super::handler::HookHandler;

/// Thread-safe registry for hook handlers and bus subscriptions.
///
/// Hooks are stored per-name and dispatched in **registration order**.
#[derive(Debug)]
pub struct HookRegistry {
    hooks: tokio::sync::RwLock<HashMap<&'static str, Vec<HookHandlerEntry>>>,
    bus: TopicRegistry,
}

impl HookRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            hooks: tokio::sync::RwLock::new(HashMap::new()),
            bus: TopicRegistry::new(),
        }
    }

    /// Register a hook handler (appended to the end of the handler list).
    pub async fn register(
        &self,
        hook_name: &'static str,
        extension_id: ExtensionId,
        handler: Arc<dyn HookHandler>,
    ) -> Result<()> {
        let mut hooks = self.hooks.write().await;
        hooks
            .entry(hook_name)
            .or_default()
            .push(HookHandlerEntry {
                extension_id,
                handler,
                metadata: HashMap::new(),
                timeout: None,
            });
        Ok(())
    }

    /// Unregister all handlers for the given hook point that belong
    /// to the specified Extension.
    pub async fn unregister(
        &self,
        hook_name: &'static str,
        extension_id: ExtensionId,
    ) -> Result<()> {
        let mut hooks = self.hooks.write().await;
        if let Some(handlers) = hooks.get_mut(hook_name) {
            handlers.retain(|entry| entry.extension_id != extension_id);
        }
        Ok(())
    }

    /// Get all handlers registered for a hook point, **in registration order**.
    pub async fn get_handlers(&self, hook_name: &'static str) -> Vec<HookHandlerEntry> {
        let hooks = self.hooks.read().await;
        hooks.get(hook_name).cloned().unwrap_or_default()
    }

    /// Return a reference to the internal bus topic registry.
    pub fn bus_registry(&self) -> &TopicRegistry {
        &self.bus
    }

    /// List all hook names for which the given Extension has registered handlers.
    pub async fn hooks_for_extension(&self, extension_id: ExtensionId) -> Vec<&'static str> {
        let hooks = self.hooks.read().await;
        let mut result = Vec::new();
        for (hook_name, handlers) in hooks.iter() {
            if handlers.iter().any(|h| h.extension_id == extension_id) {
                result.push(*hook_name);
            }
        }
        result
    }
}

impl Default for HookRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hook::handler::HookHandler;
    use crate::hook::context::HookContext;
    use crate::hook::input::HookInput;
    use crate::error::Result;
    use async_trait::async_trait;

    struct CountingHandler {
        count: std::sync::atomic::AtomicUsize,
    }

    impl CountingHandler {
        fn new() -> Self {
            Self {
                count: std::sync::atomic::AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl HookHandler for CountingHandler {
        async fn handle(&self, _ctx: &HookContext, _input: &HookInput) -> super::super::handler::HookResult {
            self.count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            super::super::handler::HookResult::Continue
        }
    }

    #[tokio::test]
    async fn test_register_and_get_handlers() {
        let registry = HookRegistry::new();
        let ext_id = ExtensionId::new();
        let handler: Arc<dyn HookHandler> = Arc::new(CountingHandler::new());

        registry.register("tool_call", ext_id, handler.clone()).await.unwrap();
        let handlers = registry.get_handlers("tool_call").await;
        assert_eq!(handlers.len(), 1);
        assert_eq!(handlers[0].extension_id, ext_id);
    }

    #[tokio::test]
    async fn test_register_multiple_handlers() {
        let registry = HookRegistry::new();
        let handler: Arc<dyn HookHandler> = Arc::new(CountingHandler::new());

        registry.register("turn_start", ExtensionId::new(), handler.clone()).await.unwrap();
        registry.register("turn_start", ExtensionId::new(), handler.clone()).await.unwrap();
        assert_eq!(registry.get_handlers("turn_start").await.len(), 2);
    }

    #[tokio::test]
    async fn test_register_preserves_order() {
        let registry = HookRegistry::new();
        let handler: Arc<dyn HookHandler> = Arc::new(CountingHandler::new());
        let id1 = ExtensionId::new();
        let id2 = ExtensionId::new();

        registry.register("tool_call", id1, handler.clone()).await.unwrap();
        registry.register("tool_call", id2, handler.clone()).await.unwrap();

        let handlers = registry.get_handlers("tool_call").await;
        assert_eq!(handlers[0].extension_id, id1);
        assert_eq!(handlers[1].extension_id, id2);
    }

    #[tokio::test]
    async fn test_unregister_removes_handlers() {
        let registry = HookRegistry::new();
        let ext_id = ExtensionId::new();
        let handler: Arc<dyn HookHandler> = Arc::new(CountingHandler::new());

        registry.register("tool_call", ext_id, handler).await.unwrap();
        assert_eq!(registry.get_handlers("tool_call").await.len(), 1);

        registry.unregister("tool_call", ext_id).await.unwrap();
        assert_eq!(registry.get_handlers("tool_call").await.len(), 0);
    }

    #[tokio::test]
    async fn test_unregister_partial() {
        let registry = HookRegistry::new();
        let handler: Arc<dyn HookHandler> = Arc::new(CountingHandler::new());
        let id1 = ExtensionId::new();
        let id2 = ExtensionId::new();

        registry.register("tool_call", id1, handler.clone()).await.unwrap();
        registry.register("tool_call", id2, handler.clone()).await.unwrap();

        registry.unregister("tool_call", id1).await.unwrap();
        let handlers = registry.get_handlers("tool_call").await;
        assert_eq!(handlers.len(), 1);
        assert_eq!(handlers[0].extension_id, id2);
    }

    #[tokio::test]
    async fn test_get_handlers_nonexistent() {
        let registry = HookRegistry::new();
        let handlers = registry.get_handlers("nonexistent").await;
        assert!(handlers.is_empty());
    }

    #[tokio::test]
    async fn test_bus_registry_access() {
        let registry = HookRegistry::new();
        let bus = registry.bus_registry();
        let id = bus
            .subscribe(
                crate::bus::BusTopic::from_str("test:topic"),
                ExtensionId::new(),
                Arc::new(crate::bus::handler::TestBusHandler::default()),
            )
            .await
            .unwrap();
        assert!(bus.subscribe(
            crate::bus::BusTopic::from_str("test:topic"),
            ExtensionId::new(),
            Arc::new(crate::bus::handler::TestBusHandler::default()),
        ).await.is_ok());
    }

    #[test]
    fn test_hook_registry_debug() {
        let registry = HookRegistry::new();
        let debug = format!("{:?}", registry);
        assert!(debug.contains("HookRegistry"));
    }

    #[tokio::test]
    async fn test_hooks_for_extension() {
        let registry = HookRegistry::new();
        let ext1 = ExtensionId::new();
        let ext2 = ExtensionId::new();
        let handler: Arc<dyn HookHandler> = Arc::new(CountingHandler::new());

        registry.register("before_tool_call", ext1, handler.clone()).await.unwrap();
        registry.register("before_tool_call", ext2, handler.clone()).await.unwrap();
        registry.register("after_tool_call", ext1, handler.clone()).await.unwrap();

        let ext1_hooks = registry.hooks_for_extension(ext1).await;
        assert_eq!(ext1_hooks.len(), 2);
        assert!(ext1_hooks.contains(&"before_tool_call"));
        assert!(ext1_hooks.contains(&"after_tool_call"));

        let ext2_hooks = registry.hooks_for_extension(ext2).await;
        assert_eq!(ext2_hooks.len(), 1);
        assert!(ext2_hooks.contains(&"before_tool_call"));

        let no_hooks = registry.hooks_for_extension(ExtensionId::new()).await;
        assert!(no_hooks.is_empty());
    }
}
