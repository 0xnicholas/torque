//! # Extension Snapshot & Recovery
//!
//! Persistence and recovery subsystem for the Torque Extension system.
//!
//! ## Module Structure
//!
//! - [`types`] — Core snapshot types (`ExtensionSnapshot`, `SnapshotMetadata`, etc.)
//! - [`storage`] — `SnapshotStorage` trait + `InMemorySnapshotStorage`
//! - [`manager`] — `SnapshotManager` orchestrating snapshot lifecycle
//! - [`recovery`] — `RecoveryManager` replaying state from snapshots

pub mod manager;
pub mod recovery;
pub mod storage;
pub mod types;

pub use manager::SnapshotManager;
pub use recovery::RecoveryManager;
pub use storage::{InMemorySnapshotStorage, SnapshotStorage};
pub use types::{
    ExtensionRegistrySnapshot, ExtensionSnapshot, SnapshotMetadata, SnapshotReason,
};
