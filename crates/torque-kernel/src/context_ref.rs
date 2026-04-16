use serde::{Deserialize, Serialize};

use crate::ids::ExternalContextRefId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExternalContextKind {
    Repository,
    Document,
    Ticket,
    FileSpace,
    Conversation,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccessMode {
    ReadOnly,
    ReadWrite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncPolicy {
    Snapshot,
    LazyFetch,
    Refreshable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalContextRef {
    pub id: ExternalContextRefId,
    pub kind: ExternalContextKind,
    pub locator: String,
    pub access_mode: AccessMode,
    pub sync_policy: SyncPolicy,
    pub metadata: Vec<(String, String)>,
}
