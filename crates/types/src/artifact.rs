use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageType {
    Redis,
    S3,
}

impl std::fmt::Display for StorageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            StorageType::Redis => "redis",
            StorageType::S3 => "s3",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub id: Uuid,
    pub node_id: Uuid,
    pub tenant_id: Uuid,
    pub storage: StorageType,
    pub location: String,
    pub size_bytes: i64,
    pub content_type: String,
    pub created_at: DateTime<Utc>,
}

impl Artifact {
    pub fn new(
        node_id: Uuid,
        tenant_id: Uuid,
        storage: StorageType,
        location: String,
        size_bytes: i64,
        content_type: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            node_id,
            tenant_id,
            storage,
            location,
            size_bytes,
            content_type,
            created_at: Utc::now(),
        }
    }
}
