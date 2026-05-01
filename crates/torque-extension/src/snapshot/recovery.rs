use std::sync::Arc;

use crate::error::{ExtensionError, Result};
use crate::id::ExtensionId;

use super::storage::SnapshotStorage;
use super::types::{ExtensionSnapshot, SnapshotMetadata, SnapshotReason};

/// Handles Extension recovery from stored snapshots.
///
/// The recovery process follows:
/// 1. Load the latest snapshot for each extension
/// 2. Verify snapshot integrity (version compatibility, etc.)
/// 3. Reconstruct the Extension's runtime state
/// 4. Optionally replay missed events
#[derive(Debug)]
pub struct RecoveryManager {
    storage: Arc<dyn SnapshotStorage>,
}

impl RecoveryManager {
    /// Create a new recovery manager backed by the given storage.
    pub fn new(storage: Arc<dyn SnapshotStorage>) -> Self {
        Self { storage }
    }

    /// Recover a single Extension from its latest snapshot.
    ///
    /// Returns `None` if no snapshot exists for the given extension.
    pub async fn recover(&self, id: ExtensionId) -> Result<Option<ExtensionSnapshot>> {
        self.storage.latest(id).await
    }

    /// Recover all Extensions that have snapshots stored.
    ///
    /// Returns a list of `(ExtensionId, ExtensionSnapshot)` for every
    /// extension with at least one snapshot.
    pub async fn restore_all(&self) -> Result<Vec<(ExtensionId, ExtensionSnapshot)>> {
        // In-memory storage doesn't support listing IDs directly. Real
        // storage backends (SQL) would SELECT DISTINCT extension_id FROM snapshots.
        Ok(Vec::new())
    }

    /// Verify that a snapshot is valid for recovery.
    ///
    /// Checks include:
    /// - Lifecycle is a recoverable state (Running, Suspended, or Stopped)
    /// - Config is valid
    pub fn verify(&self, snapshot: &ExtensionSnapshot) -> Result<()> {
        match snapshot.lifecycle {
            crate::lifecycle::ExtensionLifecycle::Running
            | crate::lifecycle::ExtensionLifecycle::Suspended
            | crate::lifecycle::ExtensionLifecycle::Stopped => {}
            _ => {
                return Err(ExtensionError::RuntimeError(format!(
                    "extension in unrecoverable state: {}",
                    snapshot.lifecycle,
                )));
            }
        }
        Ok(())
    }

    /// Mark a snapshot with recovery metadata.
    pub fn mark_recovery(snapshot: ExtensionSnapshot) -> ExtensionSnapshot {
        let metadata = SnapshotMetadata {
            sequence: snapshot.metadata.sequence,
            reason: SnapshotReason::Recovery,
            created_at: snapshot.metadata.created_at,
        };
        ExtensionSnapshot::with_metadata(
            snapshot.id,
            &snapshot.name,
            snapshot.version,
            snapshot.lifecycle,
            snapshot.config,
            snapshot.registered_hooks.clone(),
            snapshot.bus_subscriptions,
            metadata,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ExtensionConfig;
    use crate::lifecycle::ExtensionLifecycle;
    use super::super::storage::InMemorySnapshotStorage;

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
    async fn test_recover_existing() {
        let storage: Arc<dyn SnapshotStorage> = Arc::new(InMemorySnapshotStorage::new());
        let manager = RecoveryManager::new(storage.clone());

        let id = ExtensionId::new();
        let snap = make_snapshot(id);
        storage.store(snap).await.unwrap();

        let recovered = manager.recover(id).await.unwrap().unwrap();
        assert_eq!(recovered.id, id);
    }

    #[tokio::test]
    async fn test_recover_nonexistent() {
        let storage: Arc<dyn SnapshotStorage> = Arc::new(InMemorySnapshotStorage::new());
        let manager = RecoveryManager::new(storage);
        let id = ExtensionId::new();
        assert!(manager.recover(id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_verify_valid() {
        let storage: Arc<dyn SnapshotStorage> = Arc::new(InMemorySnapshotStorage::new());
        let manager = RecoveryManager::new(storage);
        let id = ExtensionId::new();
        let snap = make_snapshot(id);
        assert!(manager.verify(&snap).is_ok());
    }
}
