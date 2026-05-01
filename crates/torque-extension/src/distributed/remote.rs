use std::sync::Arc;

use async_trait::async_trait;
use torque_kernel::AgentInstanceId;
use tokio::sync::RwLock;

use crate::{
    actor::ExtensionActor,
    config::ExtensionConfig,
    context::ExtensionContext,
    error::{ExtensionError, Result},
    hook::{
        executor::HookExecutionOutcome,
        input::HookInput,
    },
    id::ExtensionId,
    lifecycle::ExtensionLifecycle,
    message::ExtensionAction,
    runtime::ExtensionRuntime,
};

use super::registry::ServiceRegistry;
use super::transport::{RemoteEndpoint, Transport};

/// A [`ExtensionRuntime`] implementation that routes Extension operations
/// to either a local runtime or a remote node via Transport + ServiceRegistry.
///
/// ```text
///  RemoteExtensionRuntime
///     ├── local: Arc<dyn ExtensionRuntime>   (local Extensions)
///     ├── transport: Arc<dyn Transport>       (cross-process I/O)
///     └── registry: Arc<dyn ServiceRegistry>  (location discovery)
/// ```
pub struct RemoteExtensionRuntime {
    local: Arc<dyn ExtensionRuntime>,
    transport: Arc<dyn Transport>,
    registry: Arc<dyn ServiceRegistry>,
    node_id: String,
    /// Cache of remote endpoints to avoid registry lookups on every send.
    endpoint_cache: RwLock<std::collections::HashMap<ExtensionId, RemoteEndpoint>>,
}

impl RemoteExtensionRuntime {
    /// Create a new remote runtime.
    pub fn new(
        local: Arc<dyn ExtensionRuntime>,
        transport: Arc<dyn Transport>,
        registry: Arc<dyn ServiceRegistry>,
        node_id: impl Into<String>,
    ) -> Self {
        Self {
            local,
            transport,
            registry,
            node_id: node_id.into(),
            endpoint_cache: RwLock::new(std::collections::HashMap::new()),
        }
    }

    /// Check whether an Extension is registered on the local node.
    pub async fn is_local(&self, id: ExtensionId) -> bool {
        // Fast path: check if local runtime has it.
        if self.local.lifecycle_of(id).await.is_ok() {
            return true;
        }
        // Check cache for known remote endpoints.
        let cache = self.endpoint_cache.read().await;
        match cache.get(&id) {
            Some(ep) => ep.node_id == self.node_id,
            None => false, // not found locally or in cache — treat as unknown
        }
    }

    /// Resolve the endpoint for an Extension, using cache first, then registry.
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

#[async_trait]
impl ExtensionRuntime for RemoteExtensionRuntime {
    async fn register(
        &self,
        extension: Arc<dyn ExtensionActor>,
        config: ExtensionConfig,
    ) -> Result<ExtensionId> {
        let id = self.local.register(extension, config).await?;
        Ok(id)
    }

    async fn unregister(&self, id: ExtensionId) -> Result<()> {
        self.local.unregister(id).await
    }

    async fn context(&self, id: ExtensionId) -> Result<ExtensionContext> {
        self.local.context(id).await
    }

    async fn send(&self, target: ExtensionId, action: ExtensionAction) -> Result<()> {
        if self.is_local(target).await {
            self.local.send(target, action).await
        } else {
            let endpoint = self.resolve(target).await?;
            self.transport
                .send(&endpoint, action)
                .await
                .map_err(ExtensionError::from)
        }
    }

    async fn execute_hook(
        &self,
        hook_name: &'static str,
        input: HookInput,
        agent_id: Option<AgentInstanceId>,
    ) -> HookExecutionOutcome {
        // Hook execution is always local.
        self.local.execute_hook(hook_name, input, agent_id).await
    }

    async fn list(&self) -> Vec<ExtensionId> {
        let mut ids = self.local.list().await;
        // Also include remote Extensions from registry.
        if let Ok(remote_ids) = self.registry.list_all().await {
            for id in remote_ids {
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
        }
        ids
    }

    async fn lifecycle_of(&self, id: ExtensionId) -> Result<ExtensionLifecycle> {
        if self.is_local(id).await {
            self.local.lifecycle_of(id).await
        } else {
            // Remote Extensions are assumed Running.
            Ok(ExtensionLifecycle::Running)
        }
    }

    async fn suspend(&self, id: ExtensionId) -> Result<()> {
        // Suspend is a local-only operation.
        if self.is_local(id).await {
            self.local.suspend(id).await
        } else {
            Err(ExtensionError::RuntimeError(
                "cannot suspend remote extension".into(),
            ))
        }
    }

    async fn resume(&self, id: ExtensionId) -> Result<()> {
        if self.is_local(id).await {
            self.local.resume(id).await
        } else {
            Err(ExtensionError::RuntimeError(
                "cannot resume remote extension".into(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        actor::ExtensionActor,
        builtin::LoggingExtension,
        config::ExtensionConfig,
        distributed::registry::InMemoryServiceRegistry,
        distributed::transport::InProcTransport,
        id::{ExtensionId, ExtensionVersion},
        lifecycle::ExtensionLifecycle,
        message::ExtensionAction,
        runtime::in_memory::InMemoryExtensionRuntime,
    };

    fn make_local_runtime() -> Arc<dyn ExtensionRuntime> {
        Arc::new(InMemoryExtensionRuntime::new())
    }

    #[tokio::test]
    async fn test_register_and_list_local() {
        let local = make_local_runtime();
        let transport = Arc::new(InProcTransport::new("node-a"));
        let registry = Arc::new(InMemoryServiceRegistry::new());
        let remote_runtime = RemoteExtensionRuntime::new(local, transport, registry, "node-a");

        let ext = Arc::new(LoggingExtension::new());
        let id = remote_runtime
            .register(ext, ExtensionConfig::default())
            .await
            .unwrap();
        let lifecycle = remote_runtime.lifecycle_of(id).await.unwrap();
        assert_eq!(lifecycle, ExtensionLifecycle::Running);

        let list = remote_runtime.list().await;
        assert!(list.contains(&id));
    }

    #[tokio::test]
    async fn test_send_to_local_extension() {
        let local = make_local_runtime();
        let transport = Arc::new(InProcTransport::new("node-a"));
        let registry = Arc::new(InMemoryServiceRegistry::new());
        let remote_runtime = RemoteExtensionRuntime::new(local.clone(), transport, registry, "node-a");

        let ext = Arc::new(LoggingExtension::new());
        let id = remote_runtime
            .register(ext, ExtensionConfig::default())
            .await
            .unwrap();

        // Sending to a local extension should succeed.
        let result = remote_runtime
            .send(id, ExtensionAction::Query { key: "stats".into() })
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_send_to_unknown_extension_fails() {
        let local = make_local_runtime();
        let transport = Arc::new(InProcTransport::new("node-a"));
        let registry = Arc::new(InMemoryServiceRegistry::new());
        let remote_runtime = RemoteExtensionRuntime::new(local, transport, registry, "node-a");

        let unknown_id = ExtensionId::new();
        let result = remote_runtime
            .send(unknown_id, ExtensionAction::Custom { namespace: "torque".into(), name: "ping".into(), payload: serde_json::Value::Null })
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_suspend_resume_local() {
        let local = make_local_runtime();
        let transport = Arc::new(InProcTransport::new("node-a"));
        let registry = Arc::new(InMemoryServiceRegistry::new());
        let remote_runtime = RemoteExtensionRuntime::new(local.clone(), transport, registry, "node-a");

        let ext = Arc::new(LoggingExtension::new());
        let id = remote_runtime
            .register(ext, ExtensionConfig::default())
            .await
            .unwrap();

        remote_runtime.suspend(id).await.unwrap();
        assert_eq!(
            remote_runtime.lifecycle_of(id).await.unwrap(),
            ExtensionLifecycle::Suspended
        );

        remote_runtime.resume(id).await.unwrap();
        assert_eq!(
            remote_runtime.lifecycle_of(id).await.unwrap(),
            ExtensionLifecycle::Running
        );
    }

    #[tokio::test]
    async fn test_suspend_remote_extension_returns_error() {
        let local = make_local_runtime();
        let transport = Arc::new(InProcTransport::new("node-a"));
        let registry = Arc::new(InMemoryServiceRegistry::new());
        let remote_runtime = RemoteExtensionRuntime::new(local, transport, registry, "node-a");

        let remote_id = ExtensionId::new();
        let result = remote_runtime.suspend(remote_id).await;
        // ID not known — should fail.
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_includes_remote_extensions() {
        let local = make_local_runtime();
        let transport = Arc::new(InProcTransport::new("node-a"));
        let registry = Arc::new(InMemoryServiceRegistry::new());
        let remote_runtime = RemoteExtensionRuntime::new(local.clone(), transport, registry.clone(), "node-a");

        // Register a local extension.
        let local_ext = Arc::new(LoggingExtension::new());
        let local_id = remote_runtime
            .register(local_ext, ExtensionConfig::default())
            .await
            .unwrap();

        // Manually register a remote extension in the service registry.
        let remote_id = ExtensionId::new();
        let remote_ep = RemoteEndpoint {
            node_id: "node-b".into(),
            extension_id: remote_id,
            address: "192.168.1.2".into(),
            port: 9091,
        };
        registry.register(remote_id, remote_ep).await.unwrap();

        let list = remote_runtime.list().await;
        assert!(list.contains(&local_id));
        assert!(list.contains(&remote_id));
    }
}
