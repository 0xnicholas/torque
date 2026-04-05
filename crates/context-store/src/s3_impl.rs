use async_trait::async_trait;
use crate::error::ContextStoreError;
use crate::store::{ArtifactPointer, ContextStore};

pub struct S3ContextStore {
    _bucket: String,
    _tenant_id: uuid::Uuid,
}

impl S3ContextStore {
    pub fn new(bucket: String, tenant_id: uuid::Uuid) -> Self {
        Self { _bucket: bucket, _tenant_id: tenant_id }
    }
}

#[async_trait]
impl ContextStore for S3ContextStore {
    async fn write(&self, _data: &[u8], _content_type: &str) -> Result<ArtifactPointer, ContextStoreError> {
        Err(ContextStoreError::S3("S3 backend not yet implemented".to_string()))
    }
    
    async fn read(&self, _pointer: &ArtifactPointer) -> Result<Vec<u8>, ContextStoreError> {
        Err(ContextStoreError::S3("S3 backend not yet implemented".to_string()))
    }
    
    async fn delete(&self, _pointer: &ArtifactPointer) -> Result<(), ContextStoreError> {
        Err(ContextStoreError::S3("S3 backend not yet implemented".to_string()))
    }
}