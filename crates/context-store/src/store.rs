use async_trait::async_trait;
use types::Artifact;
use crate::error::ContextStoreError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageType {
    Redis,
    S3,
}

#[derive(Debug, Clone)]
pub struct ArtifactPointer {
    pub task_id: String,
    pub storage: StorageType,
    pub location: String,
    pub size_bytes: i64,
    pub content_type: String,
}

impl From<Artifact> for ArtifactPointer {
    fn from(a: Artifact) -> Self {
        Self {
            task_id: a.id.to_string(),
            storage: match a.storage {
                types::StorageType::Redis => StorageType::Redis,
                types::StorageType::S3 => StorageType::S3,
            },
            location: a.location,
            size_bytes: a.size_bytes,
            content_type: a.content_type,
        }
    }
}

const SMALL_THRESHOLD: usize = 256 * 1024;
const LARGE_THRESHOLD: usize = 10 * 1024 * 1024;

#[async_trait]
pub trait ContextStore: Send + Sync {
    async fn write(&self, data: &[u8], content_type: &str) -> Result<ArtifactPointer, ContextStoreError>;
    async fn read(&self, pointer: &ArtifactPointer) -> Result<Vec<u8>, ContextStoreError>;
    async fn delete(&self, pointer: &ArtifactPointer) -> Result<(), ContextStoreError>;
}

pub fn route_storage(size_bytes: usize, content_type: &str) -> StorageType {
    if size_bytes < SMALL_THRESHOLD && content_type.contains("json") {
        StorageType::Redis
    } else if size_bytes < LARGE_THRESHOLD {
        StorageType::S3
    } else {
        StorageType::S3
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_route_storage_small_json() {
        let data = vec![0u8; 100];
        assert_eq!(route_storage(data.len(), "application/json"), StorageType::Redis);
    }

    #[test]
    fn test_route_storage_small_non_json() {
        let data = vec![0u8; 100];
        assert_eq!(route_storage(data.len(), "text/plain"), StorageType::S3);
    }

    #[test]
    fn test_route_storage_medium_json() {
        let data = vec![0u8; 300 * 1024];
        assert_eq!(route_storage(data.len(), "application/json"), StorageType::S3);
    }

    #[test]
    fn test_route_storage_large() {
        let data = vec![0u8; 15 * 1024 * 1024];
        assert_eq!(route_storage(data.len(), "application/json"), StorageType::S3);
    }
}