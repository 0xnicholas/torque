use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::error::{ExtensionError, Result};
use crate::id::ExtensionId;

use super::transport::RemoteEndpoint;

// ── Service Registry Trait ──────────────────────────────────────────────

/// Discovers where Extensions are located in the cluster.
///
/// Implementations:
/// - [`InMemoryServiceRegistry`] — in-process HashMap (testing / single-node)
/// - `RedisServiceRegistry` — Redis-backed (requires `redis` crate)
/// - `ConsulServiceRegistry` — HashiCorp Consul (requires `consul` crate)
#[async_trait]
pub trait ServiceRegistry: Send + Sync {
    /// Register an Extension at the given endpoint.
    async fn register(&self, extension_id: ExtensionId, endpoint: RemoteEndpoint) -> Result<()>;

    /// Unregister an Extension.
    async fn unregister(&self, extension_id: ExtensionId) -> Result<()>;

    /// Look up where an Extension is located.
    ///
    /// Returns `None` if the Extension is not registered.
    async fn lookup(&self, extension_id: ExtensionId) -> Result<Option<RemoteEndpoint>>;

    /// List all Extension IDs registered on a specific node.
    async fn list_node(&self, node_id: &str) -> Result<Vec<ExtensionId>>;

    /// List all known Extension IDs across the cluster.
    async fn list_all(&self) -> Result<Vec<ExtensionId>>;
}

// ── In-Memory Service Registry ───────────────────────────────────────────

/// An in-memory [`ServiceRegistry`] that stores endpoints in a `HashMap`.
///
/// Useful for testing, single-node deployments, and simulations.
#[derive(Debug)]
pub struct InMemoryServiceRegistry {
    /// extension_id → RemoteEndpoint
    entries: RwLock<HashMap<ExtensionId, RemoteEndpoint>>,
}

impl InMemoryServiceRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryServiceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ServiceRegistry for InMemoryServiceRegistry {
    async fn register(&self, extension_id: ExtensionId, endpoint: RemoteEndpoint) -> Result<()> {
        let mut entries = self.entries.write().await;
        entries.insert(extension_id, endpoint);
        Ok(())
    }

    async fn unregister(&self, extension_id: ExtensionId) -> Result<()> {
        let mut entries = self.entries.write().await;
        entries
            .remove(&extension_id)
            .ok_or(ExtensionError::NotFound(extension_id))?;
        Ok(())
    }

    async fn lookup(&self, extension_id: ExtensionId) -> Result<Option<RemoteEndpoint>> {
        let entries = self.entries.read().await;
        Ok(entries.get(&extension_id).cloned())
    }

    async fn list_node(&self, node_id: &str) -> Result<Vec<ExtensionId>> {
        let entries = self.entries.read().await;
        Ok(entries
            .iter()
            .filter(|(_, ep)| ep.node_id == node_id)
            .map(|(id, _)| *id)
            .collect())
    }

    async fn list_all(&self) -> Result<Vec<ExtensionId>> {
        let entries = self.entries.read().await;
        Ok(entries.keys().copied().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::ExtensionId;

    fn make_endpoint(ext_id: ExtensionId, node_id: &str) -> RemoteEndpoint {
        RemoteEndpoint {
            node_id: node_id.to_string(),
            extension_id: ext_id,
            address: "127.0.0.1".into(),
            port: 9090,
        }
    }

    #[tokio::test]
    async fn test_register_and_lookup() {
        let registry = InMemoryServiceRegistry::new();
        let ext_id = ExtensionId::new();
        let ep = make_endpoint(ext_id, "node-a");

        registry.register(ext_id, ep.clone()).await.unwrap();
        let found = registry.lookup(ext_id).await.unwrap().unwrap();
        assert_eq!(found, ep);
    }

    #[tokio::test]
    async fn test_lookup_nonexistent() {
        let registry = InMemoryServiceRegistry::new();
        let ext_id = ExtensionId::new();
        let found = registry.lookup(ext_id).await.unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn test_unregister() {
        let registry = InMemoryServiceRegistry::new();
        let ext_id = ExtensionId::new();
        let ep = make_endpoint(ext_id, "node-a");

        registry.register(ext_id, ep).await.unwrap();
        registry.unregister(ext_id).await.unwrap();
        assert!(registry.lookup(ext_id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_unregister_nonexistent_returns_error() {
        let registry = InMemoryServiceRegistry::new();
        let ext_id = ExtensionId::new();
        let result = registry.unregister(ext_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_node() {
        let registry = InMemoryServiceRegistry::new();
        let ext_a = ExtensionId::new();
        let ext_b = ExtensionId::new();
        let ext_c = ExtensionId::new();

        registry
            .register(ext_a, make_endpoint(ext_a, "node-a"))
            .await
            .unwrap();
        registry
            .register(ext_b, make_endpoint(ext_b, "node-b"))
            .await
            .unwrap();
        registry
            .register(ext_c, make_endpoint(ext_c, "node-a"))
            .await
            .unwrap();

        let node_a_ids = registry.list_node("node-a").await.unwrap();
        assert_eq!(node_a_ids.len(), 2);
        assert!(node_a_ids.contains(&ext_a));
        assert!(node_a_ids.contains(&ext_c));

        let node_b_ids = registry.list_node("node-b").await.unwrap();
        assert_eq!(node_b_ids.len(), 1);
        assert_eq!(node_b_ids[0], ext_b);
    }

    #[tokio::test]
    async fn test_list_all() {
        let registry = InMemoryServiceRegistry::new();
        let ext_a = ExtensionId::new();
        let ext_b = ExtensionId::new();

        registry
            .register(ext_a, make_endpoint(ext_a, "node-a"))
            .await
            .unwrap();
        registry
            .register(ext_b, make_endpoint(ext_b, "node-b"))
            .await
            .unwrap();

        let all = registry.list_all().await.unwrap();
        assert_eq!(all.len(), 2);
        assert!(all.contains(&ext_a));
        assert!(all.contains(&ext_b));
    }
}
