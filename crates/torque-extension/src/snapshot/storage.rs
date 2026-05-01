use std::sync::Arc;

use async_trait::async_trait;

use crate::error::Result;
use crate::id::ExtensionId;

use super::types::ExtensionSnapshot;

// ── SnapshotStorage Trait ────────────────────────────────────────────────

/// Abstract storage backend for extension snapshots.
///
/// Implementations can be in-memory (for testing), SQLite, or PostgreSQL
/// backed. The trait is designed to be agnostic to the storage layer.
#[async_trait]
pub trait SnapshotStorage: Send + Sync + std::fmt::Debug {
    /// Persist a single extension snapshot.
    async fn store(&self, snapshot: ExtensionSnapshot) -> Result<()>;
    /// Retrieve the most recent snapshot for an extension.
    async fn latest(&self, id: ExtensionId) -> Result<Option<ExtensionSnapshot>>;
    /// Retrieve all snapshots for an extension, most recent first.
    async fn list(&self, id: ExtensionId) -> Result<Vec<ExtensionSnapshot>>;
    /// Delete all snapshots for an extension.
    async fn delete_all(&self, id: ExtensionId) -> Result<()>;
    /// Delete snapshots older than `keep` count, keeping the most recent N.
    async fn prune(&self, id: ExtensionId, keep: usize) -> Result<usize>;
    /// Return the total number of stored snapshots.
    async fn count(&self) -> Result<usize>;
}

// ── InMemorySnapshotStorage ──────────────────────────────────────────────

/// In-memory implementation of [`SnapshotStorage`].
///
/// Stores snapshots in a `Vec` per extension. Useful for testing or
/// single-node deployments without persistence requirements.
#[derive(Debug, Default)]
pub struct InMemorySnapshotStorage {
    snapshots: std::sync::Mutex<std::collections::HashMap<ExtensionId, Vec<ExtensionSnapshot>>>,
}

impl InMemorySnapshotStorage {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl SnapshotStorage for InMemorySnapshotStorage {
    async fn store(&self, snapshot: ExtensionSnapshot) -> Result<()> {
        let mut map = self.snapshots.lock().unwrap();
        map.entry(snapshot.id).or_default().push(snapshot);
        Ok(())
    }

    async fn latest(&self, id: ExtensionId) -> Result<Option<ExtensionSnapshot>> {
        let map = self.snapshots.lock().unwrap();
        Ok(map.get(&id).and_then(|v| v.last().cloned()))
    }

    async fn list(&self, id: ExtensionId) -> Result<Vec<ExtensionSnapshot>> {
        let map = self.snapshots.lock().unwrap();
        let mut all = map.get(&id).cloned().unwrap_or_default();
        all.reverse(); // most recent first
        Ok(all)
    }

    async fn delete_all(&self, id: ExtensionId) -> Result<()> {
        let mut map = self.snapshots.lock().unwrap();
        map.remove(&id);
        Ok(())
    }

    async fn prune(&self, id: ExtensionId, keep: usize) -> Result<usize> {
        let mut map = self.snapshots.lock().unwrap();
        if let Some(snapshots) = map.get_mut(&id) {
            if snapshots.len() > keep {
                let removed = snapshots.len() - keep;
                let _ = snapshots.drain(..removed);
                return Ok(removed);
            }
        }
        Ok(0)
    }

    async fn count(&self) -> Result<usize> {
        let map = self.snapshots.lock().unwrap();
        Ok(map.values().map(|v| v.len()).sum())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ExtensionConfig;
    use crate::id::ExtensionId;
    use crate::lifecycle::ExtensionLifecycle;

    fn make_snapshot(id: ExtensionId) -> ExtensionSnapshot {
        ExtensionSnapshot::new(
            id,
            "test-ext",
            Default::default(),
            ExtensionLifecycle::Running,
            ExtensionConfig::default(),
            vec![],
            vec![],
        )
    }

    #[tokio::test]
    async fn test_store_and_latest() {
        let storage = InMemorySnapshotStorage::new();
        let id = ExtensionId::new();
        let snap = make_snapshot(id);
        storage.store(snap.clone()).await.unwrap();
        let latest = storage.latest(id).await.unwrap().unwrap();
        assert_eq!(latest.id, id);
        assert_eq!(latest.name, "test-ext");
    }

    #[tokio::test]
    async fn test_latest_nonexistent() {
        let storage = InMemorySnapshotStorage::new();
        let id = ExtensionId::new();
        assert!(storage.latest(id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_list_ordering() {
        let storage = InMemorySnapshotStorage::new();
        let id = ExtensionId::new();
        let snap1 = make_snapshot(id);
        let snap2 = make_snapshot(id);
        storage.store(snap1).await.unwrap();
        storage.store(snap2).await.unwrap();
        let list = storage.list(id).await.unwrap();
        assert_eq!(list.len(), 2);
    }

    #[tokio::test]
    async fn test_delete_all() {
        let storage = InMemorySnapshotStorage::new();
        let id = ExtensionId::new();
        storage.store(make_snapshot(id)).await.unwrap();
        storage.store(make_snapshot(id)).await.unwrap();
        storage.delete_all(id).await.unwrap();
        assert!(storage.latest(id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_prune() {
        let storage = InMemorySnapshotStorage::new();
        let id = ExtensionId::new();
        for _ in 0..5 {
            storage.store(make_snapshot(id)).await.unwrap();
        }
        let removed = storage.prune(id, 2).await.unwrap();
        assert_eq!(removed, 3);
        assert_eq!(storage.list(id).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_count() {
        let storage = InMemorySnapshotStorage::new();
        let id1 = ExtensionId::new();
        let id2 = ExtensionId::new();
        storage.store(make_snapshot(id1)).await.unwrap();
        storage.store(make_snapshot(id1)).await.unwrap();
        storage.store(make_snapshot(id2)).await.unwrap();
        assert_eq!(storage.count().await.unwrap(), 3);
    }
}
