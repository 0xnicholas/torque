use std::sync::Arc;

use torque_extension::{
    actor::ExtensionActor,
    config::ExtensionConfig,
    error::Result,
    id::ExtensionId,
    lifecycle::ExtensionLifecycle,
    message::ExtensionAction,
    runtime::snapshot::ExtensionSnapshot,
    snapshot::{InMemorySnapshotStorage, SnapshotManager, SnapshotReason, SnapshotStorage},
};

use crate::extension::runtime_handle::HarnessExtensionRuntimeHandle;

/// High-level service that manages Extension lifecycle within the Harness.
///
/// Responsibilities:
/// - Register / unregister extensions
/// - Load built-in extensions at startup
/// - Provide a central point for the API layer to query and control extensions
/// - Manage snapshot lifecycle (persistence + recovery)
pub struct ExtensionService {
    runtime: Arc<HarnessExtensionRuntimeHandle>,
    snapshot_manager: SnapshotManager,
}

impl ExtensionService {
    /// Create a new ExtensionService with the given runtime handle.
    pub fn new(runtime: Arc<HarnessExtensionRuntimeHandle>) -> Self {
        let storage: Arc<dyn SnapshotStorage> = Arc::new(InMemorySnapshotStorage::new());
        Self {
            snapshot_manager: SnapshotManager::new(storage),
            runtime,
        }
    }

    /// Create a new ExtensionService with a custom snapshot storage backend.
    pub fn with_storage(
        runtime: Arc<HarnessExtensionRuntimeHandle>,
        storage: Arc<dyn SnapshotStorage>,
    ) -> Self {
        Self {
            snapshot_manager: SnapshotManager::new(storage),
            runtime,
        }
    }

    /// Return a reference to the underlying runtime handle.
    pub fn runtime(&self) -> &Arc<HarnessExtensionRuntimeHandle> {
        &self.runtime
    }

    /// Return a reference to the SnapshotManager.
    pub fn snapshot_manager(&self) -> &SnapshotManager {
        &self.snapshot_manager
    }
    // ── Registration ───────────────────────────────────────────

    /// Register a new extension with the given config.
    pub async fn register(
        &self,
        extension: Arc<dyn ExtensionActor>,
        config: ExtensionConfig,
    ) -> Result<ExtensionId> {
        let id = self.runtime.register(extension, config).await?;

        // Take an initial snapshot when an extension is registered.
        if let Ok(snap) = self.runtime.snapshot(id).await {
            let _ = self.snapshot_manager.snapshot_manual(snap).await;
        }

        Ok(id)
    }

    /// Unregister an extension by ID.
    pub async fn unregister(&self, id: ExtensionId) -> Result<()> {
        self.runtime.unregister(id).await?;

        // Clean up snapshots.
        let _ = self.snapshot_manager.delete_all(id).await;

        Ok(())
    }

    // ── Discovery ──────────────────────────────────────────────

    /// List all registered extension IDs.
    pub async fn list(&self) -> Vec<ExtensionId> {
        self.runtime.list().await
    }

    /// List all registered extensions with their names and IDs.
    pub async fn list_with_names(&self) -> Vec<(ExtensionId, String)> {
        self.runtime.list_with_names().await
    }

    /// Look up an extension by name.
    pub async fn find_by_name(&self, name: &str) -> Option<ExtensionId> {
        self.runtime.find_by_name(name).await
    }

    /// Query the lifecycle state of an extension.
    pub async fn lifecycle(&self, id: ExtensionId) -> Option<ExtensionLifecycle> {
        self.runtime.lifecycle(id).await
    }

    /// Take a snapshot of an extension's runtime state.
    pub async fn snapshot(&self, id: ExtensionId) -> Result<ExtensionSnapshot> {
        self.runtime.snapshot(id).await
    }

    /// Persist a snapshot with explicit reason metadata.
    pub async fn persist_snapshot(
        &self,
        id: ExtensionId,
        reason: SnapshotReason,
    ) -> Result<()> {
        let snap = self.runtime.snapshot(id).await?;
        self.snapshot_manager.snapshot(snap, reason).await
    }

    // ── Communication ──────────────────────────────────────────

    /// Send a fire-and-forget message to an extension.
    pub async fn send(&self, target: ExtensionId, action: ExtensionAction) -> Result<()> {
        self.runtime.send(target, action).await
    }

    // ── Lifecycle Control ──────────────────────────────────────

    /// Suspend an extension (future: delegates to runtime).
    pub async fn suspend(&self, id: ExtensionId) -> Result<()> {
        self.runtime.suspend(id).await
    }

    /// Resume a suspended extension (future).
    pub async fn resume(&self, id: ExtensionId) -> Result<()> {
        self.runtime.resume(id).await
    }

    // ── Built-in Extension Loaders ─────────────────────────────

    /// Load built-in extensions based on configuration.
    ///
    /// Supported built-ins: `"logging"`, `"metrics"`.
    pub async fn load_builtins(
        &self,
        builtins: &[String],
    ) -> Vec<BuiltinLoadResult> {
        let mut results = Vec::with_capacity(builtins.len());
        for name in builtins {
            let result = match name.as_str() {
                "logging" => self.try_load_builtin(name, Self::create_logging_extension()).await,
                "metrics" => self.try_load_builtin(name, Self::create_metrics_extension()).await,
                other => BuiltinLoadResult {
                    name: other.to_string(),
                    id: None,
                    error: Some(format!("unknown built-in extension: {other}")),
                },
            };
            results.push(result);
        }
        results
    }

    async fn try_load_builtin(
        &self,
        name: &str,
        ext: Option<Arc<dyn ExtensionActor>>,
    ) -> BuiltinLoadResult {
        let Some(extension) = ext else {
            return BuiltinLoadResult {
                name: name.to_string(),
                id: None,
                error: Some(format!("built-in '{name}' not yet implemented")),
            };
        };

        let cfg = ExtensionConfig {
            settings: serde_json::json!({"source": "builtin"}),
            ..Default::default()
        };
        match self.register(extension, cfg).await {
            Ok(id) => BuiltinLoadResult {
                name: name.to_string(),
                id: Some(id),
                error: None,
            },
            Err(e) => BuiltinLoadResult {
                name: name.to_string(),
                id: None,
                error: Some(e.to_string()),
            },
        }
    }

    /// Create a logging extension (stub — Phase 4 will provide the real implementation).
    fn create_logging_extension() -> Option<Arc<dyn ExtensionActor>> {
        None
    }

    /// Create a metrics extension (stub — Phase 4 will provide the real implementation).
    fn create_metrics_extension() -> Option<Arc<dyn ExtensionActor>> {
        None
    }
}

/// Result of loading a single built-in extension.
#[derive(Debug, Clone)]
pub struct BuiltinLoadResult {
    /// Name of the built-in extension.
    pub name: String,
    /// Registered ExtensionId, if successful.
    pub id: Option<ExtensionId>,
    /// Error message, if loading failed.
    pub error: Option<String>,
}
