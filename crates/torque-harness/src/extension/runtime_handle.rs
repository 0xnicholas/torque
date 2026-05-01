use std::sync::Arc;
use torque_extension::{
    config::ExtensionConfig,
    context::ExtensionContext,
    error::Result,
    hook::{
        executor::HookExecutionOutcome,
        input::HookInput,
    },
    id::ExtensionId,
    lifecycle::ExtensionLifecycle,
    message::ExtensionAction,
    runtime::{
        in_memory::InMemoryExtensionRuntime,
        snapshot::ExtensionSnapshot,
        ExtensionRuntime,
    },
};
use torque_kernel::AgentInstanceId;

/// Wraps `InMemoryExtensionRuntime` with convenience methods needed by
/// the Harness layer.
///
/// Adds:
/// - Name-based lookup (`find_by_name`)
/// - Lifecycle querying (`lifecycle`)
/// - Suspend / Resume
pub struct HarnessExtensionRuntimeHandle {
    inner: InMemoryExtensionRuntime,
}

impl HarnessExtensionRuntimeHandle {
    /// Create a new runtime handle backed by an in-memory runtime.
    pub fn new() -> Self {
        Self {
            inner: InMemoryExtensionRuntime::new(),
        }
    }

    /// Register an extension.
    pub async fn register(
        &self,
        extension: Arc<dyn torque_extension::actor::ExtensionActor>,
        config: ExtensionConfig,
    ) -> Result<ExtensionId> {
        let id = self.inner.register(extension, config).await?;
        Ok(id)
    }

    /// Unregister by ID.
    pub async fn unregister(&self, id: ExtensionId) -> Result<()> {
        self.inner.unregister(id).await
    }

    /// Look up an ExtensionId by its human-readable name.
    pub async fn find_by_name(&self, name: &str) -> Option<ExtensionId> {
        self.inner.find_by_name(name).await
    }

    /// List all registered extension IDs.
    pub async fn list(&self) -> Vec<ExtensionId> {
        self.inner.list().await
    }

    /// List all registered extensions with their names and IDs.
    pub async fn list_with_names(&self) -> Vec<(ExtensionId, String)> {
        let ids = self.inner.list().await;
        let mut result = Vec::with_capacity(ids.len());
        for id in ids {
            let name = self.inner.name_for_id(id).await
                .unwrap_or_else(|| format!("ext-{}", id));
            result.push((id, name));
        }
        result
    }

    /// Get the ExtensionContext for a given ID.
    pub async fn context(&self, id: ExtensionId) -> Result<ExtensionContext> {
        self.inner.context(id).await
    }

    /// Send a fire-and-forget message to an extension.
    pub async fn send(&self, target: ExtensionId, action: ExtensionAction) -> Result<()> {
        self.inner.send(target, action).await
    }

    /// Execute hooks through the runtime.
    pub async fn execute_hook(
        &self,
        hook_name: &'static str,
        input: HookInput,
        agent_id: Option<AgentInstanceId>,
    ) -> HookExecutionOutcome {
        self.inner.execute_hook(hook_name, input, agent_id).await
    }

    /// Query the lifecycle state of an extension.
    pub async fn lifecycle(&self, id: ExtensionId) -> Option<ExtensionLifecycle> {
        self.inner.lifecycle_of(id).await.ok()
    }

    /// Suspend an extension.
    pub async fn suspend(&self, id: ExtensionId) -> Result<()> {
        self.inner.suspend(id).await
    }

    /// Resume a suspended extension.
    pub async fn resume(&self, id: ExtensionId) -> Result<()> {
        self.inner.resume(id).await
    }

    /// Take a snapshot of an extension's runtime state.
    pub async fn snapshot(&self, id: ExtensionId) -> Result<ExtensionSnapshot> {
        self.inner.snapshot(id).await
    }
}

impl Default for HarnessExtensionRuntimeHandle {
    fn default() -> Self {
        Self::new()
    }
}
