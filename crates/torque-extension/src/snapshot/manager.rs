use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::Result;
use crate::id::ExtensionId;

use super::recovery::RecoveryManager;
use super::storage::SnapshotStorage;
use super::types::{
    ExtensionRegistrySnapshot, ExtensionSnapshot, SnapshotMetadata, SnapshotReason,
};

/// Manages the lifecycle of extension snapshots: creation, restoration,
/// pruning, and export.
///
/// Coordinates with the runtime to capture consistent snapshots and with
/// the storage backend for persistence.
pub struct SnapshotManager {
    storage: Arc<dyn SnapshotStorage>,
    recovery: RecoveryManager,
    /// Per-extension sequence counters for snapshot ordering.
    sequences: std::sync::Mutex<std::collections::HashMap<ExtensionId, u64>>,
    /// Global snapshot counter for registry metadata.
    global_counter: AtomicU64,
}

impl SnapshotManager {
    /// Create a new manager with the given storage backend.
    pub fn new(storage: Arc<dyn SnapshotStorage>) -> Self {
        Self {
            recovery: RecoveryManager::new(storage.clone()),
            storage,
            sequences: std::sync::Mutex::new(std::collections::HashMap::new()),
            global_counter: AtomicU64::new(0),
        }
    }

    /// Create a manager pre-configured with a recovery manager.
    pub fn with_recovery(storage: Arc<dyn SnapshotStorage>, recovery: RecoveryManager) -> Self {
        Self {
            storage,
            recovery,
            sequences: std::sync::Mutex::new(std::collections::HashMap::new()),
            global_counter: AtomicU64::new(0),
        }
    }

    // ── Snapshotting ──────────────────────────────────────────────────────

    /// Take a snapshot of a single extension with the given reason.
    pub async fn snapshot(
        &self,
        snapshot: ExtensionSnapshot,
        reason: SnapshotReason,
    ) -> Result<()> {
        let sequence = {
            let mut seq_map = self.sequences.lock().unwrap();
            let seq = seq_map.entry(snapshot.id).or_insert(0);
            *seq += 1;
            *seq
        };
        self.global_counter.fetch_add(1, Ordering::SeqCst);

        let snap = ExtensionSnapshot::with_metadata(
            snapshot.id,
            &snapshot.name,
            snapshot.version,
            snapshot.lifecycle,
            snapshot.config,
            snapshot.registered_hooks,
            snapshot.bus_subscriptions,
            SnapshotMetadata {
                sequence,
                reason,
                created_at: chrono::Utc::now(),
            },
        );
        self.storage.store(snap).await
    }

    /// Convenience: snapshot with `Manual` reason.
    pub async fn snapshot_manual(&self, snapshot: ExtensionSnapshot) -> Result<()> {
        self.snapshot(snapshot, SnapshotReason::Manual).await
    }

    // ── Restoration ───────────────────────────────────────────────────────

    /// Restore the latest snapshot for an extension.
    ///
    /// Returns `None` if no snapshot exists for the given extension.
    pub async fn restore(&self, id: ExtensionId) -> Result<Option<ExtensionSnapshot>> {
        self.storage.latest(id).await
    }

    /// Restore all extensions from their latest snapshots.
    pub async fn restore_all(&self) -> Result<Vec<(ExtensionId, ExtensionSnapshot)>> {
        self.recovery.restore_all().await
    }

    // ── Recovery ──────────────────────────────────────────────────────────

    /// Access the recovery manager.
    pub fn recovery(&self) -> &RecoveryManager {
        &self.recovery
    }

    // ── Pruning / Cleanup ─────────────────────────────────────────────────

    /// Prune old snapshots, keeping only the most recent N for each extension.
    pub async fn prune(&self, id: ExtensionId, keep: usize) -> Result<usize> {
        self.storage.prune(id, keep).await
    }

    /// Delete all snapshots for an extension.
    pub async fn delete_all(&self, id: ExtensionId) -> Result<()> {
        self.sequences.lock().unwrap().remove(&id);
        self.storage.delete_all(id).await
    }

    // ── Query ─────────────────────────────────────────────────────────────

    /// Get the latest snapshot for an extension.
    pub async fn latest(&self, id: ExtensionId) -> Result<Option<ExtensionSnapshot>> {
        self.storage.latest(id).await
    }

    /// List all snapshots for an extension, most recent first.
    pub async fn list(&self, id: ExtensionId) -> Result<Vec<ExtensionSnapshot>> {
        self.storage.list(id).await
    }

    /// Get a registry-level summary snapshot.
    pub async fn registry_snapshot(&self) -> Result<ExtensionRegistrySnapshot> {
        let count = self.storage.count().await?;
        Ok(ExtensionRegistrySnapshot {
            snapshot_count: count,
            extension_count: self.sequences.lock().unwrap().len(),
            status: "operational".to_string(),
            created_at: chrono::Utc::now(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ExtensionConfig;
    use crate::lifecycle::ExtensionLifecycle;

    fn make_manager() -> SnapshotManager {
        let storage: Arc<dyn SnapshotStorage> =
            Arc::new(super::super::storage::InMemorySnapshotStorage::new());
        SnapshotManager::new(storage)
    }

    fn make_snapshot(id: ExtensionId) -> ExtensionSnapshot {
        ExtensionSnapshot::new(
            id,
            "ext-a",
            Default::default(),
            ExtensionLifecycle::Running,
            ExtensionConfig::default(),
            vec![],
            vec![],
        )
    }

    #[tokio::test]
    async fn test_snapshot_and_restore() {
        let manager = make_manager();
        let id = ExtensionId::new();
        let snap = make_snapshot(id);

        manager.snapshot_manual(snap).await.unwrap();

        let restored = manager.restore(id).await.unwrap().unwrap();
        assert_eq!(restored.id, id);
        assert_eq!(restored.name, "ext-a");
    }

    #[tokio::test]
    async fn test_snapshot_sequence_increments() {
        let manager = make_manager();
        let id = ExtensionId::new();

        manager
            .snapshot(make_snapshot(id), SnapshotReason::Periodic)
            .await
            .unwrap();
        manager
            .snapshot(make_snapshot(id), SnapshotReason::Shutdown)
            .await
            .unwrap();

        let list = manager.list(id).await.unwrap();
        assert_eq!(list.len(), 2);
        // Most recent first
        assert_eq!(list[0].metadata.reason, SnapshotReason::Shutdown);
        assert_eq!(list[0].metadata.sequence, 2);
        assert_eq!(list[1].metadata.sequence, 1);
    }

    #[tokio::test]
    async fn test_restore_nonexistent() {
        let manager = make_manager();
        let id = ExtensionId::new();
        assert!(manager.restore(id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_prune() {
        let manager = make_manager();
        let id = ExtensionId::new();

        for _ in 0..5 {
            manager.snapshot_manual(make_snapshot(id)).await.unwrap();
        }

        let removed = manager.prune(id, 2).await.unwrap();
        assert_eq!(removed, 3);
        assert_eq!(manager.list(id).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_delete_all() {
        let manager = make_manager();
        let id = ExtensionId::new();

        manager.snapshot_manual(make_snapshot(id)).await.unwrap();
        manager.delete_all(id).await.unwrap();

        assert!(manager.restore(id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_registry_snapshot() {
        let manager = make_manager();
        let reg = manager.registry_snapshot().await.unwrap();
        assert_eq!(reg.status, "operational");
    }
}
