use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::config::ExtensionConfig;
use crate::id::{ExtensionId, ExtensionVersion};
use crate::lifecycle::ExtensionLifecycle;

// ── Snapshot Metadata ────────────────────────────────────────────────────

/// The reason a snapshot was taken.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SnapshotReason {
    /// Periodic scheduled snapshot.
    Periodic,
    /// Snapshot taken during graceful shutdown.
    Shutdown,
    /// Manual snapshot requested via API.
    Manual,
    /// Snapshot taken when an extension is suspended.
    Suspend,
    /// Snapshot taken before an extension upgrade.
    Upgrade,
    /// Snapshot taken during recovery replay.
    Recovery,
}

/// Metadata attached to every snapshot for ordering and provenance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    /// Monotonically increasing sequence number scoped to the extension.
    pub sequence: u64,
    /// Why this snapshot was taken.
    pub reason: SnapshotReason,
    /// When this snapshot was created.
    pub created_at: DateTime<Utc>,
}

// ── Extension Snapshot ───────────────────────────────────────────────────

/// A point-in-time snapshot of an Extension's full runtime state.
///
/// Used for observability, debugging, persistence (Phase 5), and recovery.
/// Snapshots are serializable and can be stored or transmitted without
/// holding runtime locks.
#[derive(Debug, Clone, Serialize)]
pub struct ExtensionSnapshot {
    pub id: ExtensionId,
    pub name: String,
    pub version: ExtensionVersion,
    /// Metadata about this snapshot (sequence, reason, timestamp).
    pub metadata: SnapshotMetadata,
    pub lifecycle: ExtensionLifecycle,
    pub config: ExtensionConfig,
    pub registered_hooks: Vec<String>,
    pub bus_subscriptions: Vec<String>,
}

impl ExtensionSnapshot {
    /// Create a new snapshot with manual-trigger metadata (default sequence 0).
    pub fn new(
        id: ExtensionId,
        name: &str,
        version: ExtensionVersion,
        lifecycle: ExtensionLifecycle,
        config: ExtensionConfig,
        registered_hooks: Vec<String>,
        bus_subscriptions: Vec<String>,
    ) -> Self {
        Self::with_metadata(
            id,
            name,
            version,
            lifecycle,
            config,
            registered_hooks,
            bus_subscriptions,
            SnapshotMetadata {
                sequence: 0,
                reason: SnapshotReason::Manual,
                created_at: Utc::now(),
            },
        )
    }

    /// Create a snapshot with explicit metadata.
    pub fn with_metadata(
        id: ExtensionId,
        name: &str,
        version: ExtensionVersion,
        lifecycle: ExtensionLifecycle,
        config: ExtensionConfig,
        registered_hooks: Vec<String>,
        bus_subscriptions: Vec<String>,
        metadata: SnapshotMetadata,
    ) -> Self {
        Self {
            id,
            name: name.to_string(),
            version,
            metadata,
            lifecycle,
            config,
            registered_hooks,
            bus_subscriptions,
        }
    }
}

// ── Registry-level Snapshot ──────────────────────────────────────────────

/// A snapshot of all extension registrations in the system.
///
/// This provides a high-level view of the extension subsystem health
/// and registration state at a point in time.
#[derive(Debug, Clone, Serialize)]
pub struct ExtensionRegistrySnapshot {
    /// Total number of extension snapshots stored.
    pub snapshot_count: usize,
    /// Number of registered extensions.
    pub extension_count: usize,
    /// Human-readable status summary.
    pub status: String,
    /// When this registry snapshot was taken.
    pub created_at: DateTime<Utc>,
}
