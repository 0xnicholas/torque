use std::collections::HashMap;

use crate::error::{ExtensionError, Result};
use crate::id::ExtensionId;

use super::handler::BoxedBusHandler;
use super::topic::BusTopic;

/// Identifier for a topic subscription.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubscriptionId(uuid::Uuid);

impl SubscriptionId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl Default for SubscriptionId {
    fn default() -> Self {
        Self::new()
    }
}

/// A subscription entry stored in the registry.
#[derive(Clone)]
pub(crate) struct SubscriptionEntry {
    pub id: SubscriptionId,
    pub extension_id: ExtensionId,
    pub handler: BoxedBusHandler,
}

impl std::fmt::Debug for SubscriptionEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubscriptionEntry")
            .field("id", &self.id)
            .field("extension_id", &self.extension_id)
            .field("handler", &format_args!("BusEventHandler(...)"))
            .finish()
    }
}

/// Thread-safe registry for EventBus topic subscriptions.
#[derive(Debug)]
pub struct TopicRegistry {
    topics: tokio::sync::RwLock<HashMap<BusTopic, Vec<SubscriptionEntry>>>,
}

impl TopicRegistry {
    /// Create an empty topic registry.
    pub fn new() -> Self {
        Self {
            topics: tokio::sync::RwLock::new(HashMap::new()),
        }
    }

    /// Subscribe to a topic.
    pub async fn subscribe(
        &self,
        topic: BusTopic,
        extension_id: ExtensionId,
        handler: BoxedBusHandler,
    ) -> Result<SubscriptionId> {
        let id = SubscriptionId::new();
        let mut topics = self.topics.write().await;
        topics
            .entry(topic)
            .or_default()
            .push(SubscriptionEntry {
                id,
                extension_id,
                handler,
            });
        Ok(id)
    }

    /// Unsubscribe by subscription ID.
    pub async fn unsubscribe(&self, subscription_id: SubscriptionId) -> Result<()> {
        let mut topics = self.topics.write().await;
        for (_topic, entries) in topics.iter_mut() {
            let before = entries.len();
            entries.retain(|e| e.id != subscription_id);
            if entries.len() < before {
                return Ok(());
            }
        }
        Err(ExtensionError::SubscriptionNotFound(format!(
            "{:?}",
            subscription_id
        )))
    }

    /// Get all subscribers for a topic.
    pub(crate) async fn subscribers(&self, topic: &BusTopic) -> Vec<SubscriptionEntry> {
        let topics = self.topics.read().await;
        topics.get(topic).cloned().unwrap_or_default()
    }

    /// Collect human-readable topic names for all subscriptions belonging to an Extension.
    pub(crate) async fn subscriptions_for_extension(&self, extension_id: ExtensionId) -> Vec<String> {
        let topics = self.topics.read().await;
        let mut result = Vec::new();
        for (topic, entries) in topics.iter() {
            if entries.iter().any(|e| e.extension_id == extension_id) {
                result.push(topic.to_string());
            }
        }
        result
    }
}

impl Default for TopicRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::ExtensionId;
    use crate::bus::handler::BusEventHandler;
    use std::sync::Arc;

    struct TestEventHandler {
        handled: std::sync::atomic::AtomicBool,
    }

    #[async_trait::async_trait]
    impl BusEventHandler for TestEventHandler {
        async fn handle(&self, _event: &super::super::event::BusEvent) {
            self.handled.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }

    impl TestEventHandler {
        fn new() -> Self {
            Self {
                handled: std::sync::atomic::AtomicBool::new(false),
            }
        }
        fn was_handled(&self) -> bool {
            self.handled.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    #[tokio::test]
    async fn test_subscribe_and_subscribers() {
        let registry = TopicRegistry::new();
        let topic = BusTopic::from_str("test:events");
        let ext_id = ExtensionId::new();
        let handler: BoxedBusHandler = Arc::new(TestEventHandler::new());

        let sub_id = registry.subscribe(topic.clone(), ext_id, handler.clone()).await.unwrap();
        let subscribers = registry.subscribers(&topic).await;
        assert_eq!(subscribers.len(), 1);
        assert_eq!(subscribers[0].extension_id, ext_id);
    }

    #[tokio::test]
    async fn test_subscribe_multiple() {
        let registry = TopicRegistry::new();
        let topic = BusTopic::from_str("test:multi");
        let handler: BoxedBusHandler = Arc::new(TestEventHandler::new());

        let id1 = registry.subscribe(topic.clone(), ExtensionId::new(), handler.clone()).await.unwrap();
        let id2 = registry.subscribe(topic.clone(), ExtensionId::new(), handler.clone()).await.unwrap();
        assert_ne!(id1, id2);

        let subscribers = registry.subscribers(&topic).await;
        assert_eq!(subscribers.len(), 2);
    }

    #[tokio::test]
    async fn test_unsubscribe_valid() {
        let registry = TopicRegistry::new();
        let topic = BusTopic::from_str("test:unsub");
        let handler: BoxedBusHandler = Arc::new(TestEventHandler::new());

        let sub_id = registry.subscribe(topic.clone(), ExtensionId::new(), handler).await.unwrap();
        assert_eq!(registry.subscribers(&topic).await.len(), 1);

        registry.unsubscribe(sub_id).await.unwrap();
        assert_eq!(registry.subscribers(&topic).await.len(), 0);
    }

    #[tokio::test]
    async fn test_unsubscribe_invalid_id() {
        let registry = TopicRegistry::new();
        let result = registry.unsubscribe(SubscriptionId::new()).await;
        assert!(result.is_err());
        match result {
            Err(ExtensionError::SubscriptionNotFound(_)) => {}
            _ => panic!("expected SubscriptionNotFound error"),
        }
    }

    #[tokio::test]
    async fn test_subscribe_different_topics() {
        let registry = TopicRegistry::new();
        let handler: BoxedBusHandler = Arc::new(TestEventHandler::new());

        registry.subscribe(BusTopic::from_str("a:x"), ExtensionId::new(), handler.clone()).await.unwrap();
        registry.subscribe(BusTopic::from_str("b:y"), ExtensionId::new(), handler.clone()).await.unwrap();

        assert_eq!(registry.subscribers(&BusTopic::from_str("a:x")).await.len(), 1);
        assert_eq!(registry.subscribers(&BusTopic::from_str("b:y")).await.len(), 1);
        assert_eq!(registry.subscribers(&BusTopic::from_str("a:y")).await.len(), 0);
    }

    #[test]
    fn test_subscription_id_new() {
        let id1 = SubscriptionId::new();
        let id2 = SubscriptionId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_subscription_id_debug() {
        let id = SubscriptionId::new();
        let debug = format!("{:?}", id);
        assert!(!debug.is_empty());
    }

    #[tokio::test]
    async fn test_subscriptions_for_extension() {
        let registry = TopicRegistry::new();
        let ext1 = ExtensionId::new();
        let ext2 = ExtensionId::new();
        let handler: BoxedBusHandler = Arc::new(TestEventHandler::new());

        registry.subscribe(BusTopic::from_str("a:x"), ext1, handler.clone()).await.unwrap();
        registry.subscribe(BusTopic::from_str("b:y"), ext1, handler.clone()).await.unwrap();
        registry.subscribe(BusTopic::from_str("a:x"), ext2, handler.clone()).await.unwrap();

        let ext1_subs = registry.subscriptions_for_extension(ext1).await;
        assert_eq!(ext1_subs.len(), 2);
        assert!(ext1_subs.contains(&"a:x".to_string()));
        assert!(ext1_subs.contains(&"b:y".to_string()));

        let ext2_subs = registry.subscriptions_for_extension(ext2).await;
        assert_eq!(ext2_subs.len(), 1);
        assert!(ext2_subs.contains(&"a:x".to_string()));

        let no_subs = registry.subscriptions_for_extension(ExtensionId::new()).await;
        assert!(no_subs.is_empty());
    }
}
