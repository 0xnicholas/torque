use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::{
    error::{ExtensionError, Result},
    id::ExtensionId,
    message::ExtensionAction,
    runtime::ExtensionRuntime,
};

use super::load_balancer::LoadBalancer;
use super::registry::ServiceRegistry;
use super::transport::{RemoteEndpoint, Transport};

/// Routes messages between local and remote Extensions.
///
/// Acts as the central dispatch layer:
/// - Local Extensions → deliver directly via runtime
/// - Remote Extensions → deliver via Transport + ServiceRegistry
/// - Load balancing across eligible targets
pub struct MessageRouter {
    runtime: Arc<dyn ExtensionRuntime>,
    transport: Arc<dyn Transport>,
    registry: Arc<dyn ServiceRegistry>,
    node_id: String,
    load_balancer: Option<Arc<LoadBalancer>>,
    /// Cache of remote endpoints.
    endpoint_cache: RwLock<std::collections::HashMap<ExtensionId, RemoteEndpoint>>,
}

impl MessageRouter {
    /// Create a new message router.
    pub fn new(
        runtime: Arc<dyn ExtensionRuntime>,
        transport: Arc<dyn Transport>,
        registry: Arc<dyn ServiceRegistry>,
        node_id: impl Into<String>,
    ) -> Self {
        Self {
            runtime,
            transport,
            registry,
            node_id: node_id.into(),
            load_balancer: None,
            endpoint_cache: RwLock::new(std::collections::HashMap::new()),
        }
    }

    /// Create a new message router with a load balancer.
    pub fn with_load_balancer(
        runtime: Arc<dyn ExtensionRuntime>,
        transport: Arc<dyn Transport>,
        registry: Arc<dyn ServiceRegistry>,
        node_id: impl Into<String>,
        load_balancer: Arc<LoadBalancer>,
    ) -> Self {
        Self {
            runtime,
            transport,
            registry,
            node_id: node_id.into(),
            load_balancer: Some(load_balancer),
            endpoint_cache: RwLock::new(std::collections::HashMap::new()),
        }
    }

    /// Check whether an Extension lives on the local node.
    pub async fn is_local(&self, id: ExtensionId) -> bool {
        if self.runtime.lifecycle_of(id).await.is_ok() {
            return true;
        }
        let cache = self.endpoint_cache.read().await;
        match cache.get(&id) {
            Some(ep) => ep.node_id == self.node_id,
            None => false,
        }
    }

    /// Route and send a message to an Extension.
    pub async fn route(&self, target: ExtensionId, action: ExtensionAction) -> Result<()> {
        if self.is_local(target).await {
            self.runtime.send(target, action).await
        } else {
            let endpoint = self.resolve(target).await?;
            self.transport
                .send(&endpoint, action)
                .await
                .map_err(ExtensionError::from)
        }
    }

    /// Route a message to one of several eligible Extension instances
    /// using the configured load-balancing strategy.
    pub async fn route_with_balance(
        &self,
        targets: &[ExtensionId],
        action: ExtensionAction,
    ) -> Result<()> {
        if targets.is_empty() {
            return Err(ExtensionError::RuntimeError("no targets available".into()));
        }

        if let Some(lb) = &self.load_balancer {
            if let Some(selected) = lb.select(targets).await {
                lb.record_connection(selected).await;
                let result = self.route(selected, action).await;
                lb.release_connection(selected).await;
                return result;
            }
        }

        // No load balancer — round-robin fallback.
        self.route(targets[0], action).await
    }

    /// Resolve an Extension ID to its endpoint.
    async fn resolve(&self, id: ExtensionId) -> Result<RemoteEndpoint> {
        // Check cache.
        {
            let cache = self.endpoint_cache.read().await;
            if let Some(ep) = cache.get(&id) {
                return Ok(ep.clone());
            }
        }

        // Look up in registry.
        match self.registry.lookup(id).await? {
            Some(ep) => {
                let mut cache = self.endpoint_cache.write().await;
                cache.insert(id, ep.clone());
                Ok(ep)
            }
            None => Err(ExtensionError::NotFound(id)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        builtin::LoggingExtension,
        config::ExtensionConfig,
        distributed::{
            load_balancer::{LoadBalancer, LoadBalancingStrategy},
            registry::InMemoryServiceRegistry,
            transport::InProcTransport,
        },
        id::ExtensionId,
        runtime::in_memory::InMemoryExtensionRuntime,
    };

    fn make_runtime() -> Arc<InMemoryExtensionRuntime> {
        Arc::new(InMemoryExtensionRuntime::new())
    }

    #[tokio::test]
    async fn test_route_to_local_extension() {
        let runtime = make_runtime();
        let transport = Arc::new(InProcTransport::new("node-a"));
        let registry = Arc::new(InMemoryServiceRegistry::new());
        let router = MessageRouter::new(runtime.clone(), transport, registry, "node-a");

        let ext = Arc::new(LoggingExtension::new());
        let id = runtime
            .register(ext, ExtensionConfig::default())
            .await
            .unwrap();

        let result = router
            .route(id, ExtensionAction::Query { key: "stats".into() })
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_route_to_unknown_extension_fails() {
        let runtime = make_runtime();
        let transport = Arc::new(InProcTransport::new("node-a"));
        let registry = Arc::new(InMemoryServiceRegistry::new());
        let router = MessageRouter::new(runtime, transport, registry, "node-a");

        let unknown = ExtensionId::new();
        let result = router
            .route(unknown, ExtensionAction::Custom { namespace: "torque".into(), name: "ping".into(), payload: serde_json::Value::Null })
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_route_with_balance_selects_target() {
        let runtime = make_runtime();
        let transport = Arc::new(InProcTransport::new("node-a"));
        let registry = Arc::new(InMemoryServiceRegistry::new());
        let lb = Arc::new(LoadBalancer::new(LoadBalancingStrategy::RoundRobin));
        let router =
            MessageRouter::with_load_balancer(runtime.clone(), transport, registry, "node-a", lb);

        let ext = Arc::new(LoggingExtension::new());
        let id = runtime
            .register(ext, ExtensionConfig::default())
            .await
            .unwrap();

        let result = router
            .route_with_balance(&[id], ExtensionAction::Custom { namespace: "torque".into(), name: "test".into(), payload: serde_json::Value::Null })
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_route_with_balance_empty_targets_fails() {
        let runtime = make_runtime();
        let transport = Arc::new(InProcTransport::new("node-a"));
        let registry = Arc::new(InMemoryServiceRegistry::new());
        let lb = Arc::new(LoadBalancer::new(LoadBalancingStrategy::Random));
        let router =
            MessageRouter::with_load_balancer(runtime, transport, registry, "node-a", lb);

        let result = router
            .route_with_balance(&[], ExtensionAction::Custom { namespace: "torque".into(), name: "test".into(), payload: serde_json::Value::Null })
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_is_local_known_extension() {
        let runtime = make_runtime();
        let transport = Arc::new(InProcTransport::new("node-a"));
        let registry = Arc::new(InMemoryServiceRegistry::new());
        let router = MessageRouter::new(runtime.clone(), transport, registry, "node-a");

        let ext = Arc::new(LoggingExtension::new());
        let id = runtime
            .register(ext, ExtensionConfig::default())
            .await
            .unwrap();

        assert!(router.is_local(id).await);
    }

    #[tokio::test]
    async fn test_is_local_unknown_extension_not_local() {
        let runtime = make_runtime();
        let transport = Arc::new(InProcTransport::new("node-a"));
        let registry = Arc::new(InMemoryServiceRegistry::new());
        let router = MessageRouter::new(runtime, transport, registry, "node-a");

        let unknown = ExtensionId::new();
        // Unknown extensions are not local.
        assert!(!router.is_local(unknown).await);
    }
}
