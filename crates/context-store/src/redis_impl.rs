use async_trait::async_trait;
use redis::aio::ConnectionManager;
use crate::error::ContextStoreError;
use crate::store::{ArtifactPointer, ContextStore, StorageType};

pub struct RedisContextStore {
    conn: ConnectionManager,
    tenant_id: uuid::Uuid,
    ttl_secs: u64,
}

impl RedisContextStore {
    pub fn new(conn: ConnectionManager, tenant_id: uuid::Uuid, ttl_secs: u64) -> Self {
        Self { conn, tenant_id, ttl_secs }
    }
    
    fn make_key(&self, task_id: &str) -> String {
        format!("{}:node:{}:artifact", self.tenant_id, task_id)
    }
}

#[async_trait]
impl ContextStore for RedisContextStore {
    async fn write(&self, data: &[u8], content_type: &str) -> Result<ArtifactPointer, ContextStoreError> {
        let task_id = uuid::Uuid::new_v4().to_string();
        let key = self.make_key(&task_id);
        
        let mut conn = self.conn.clone();
        let _: () = redis::cmd("SETEX")
            .arg(&key)
            .arg(self.ttl_secs as i64)
            .arg(data)
            .query_async(&mut conn)
            .await?;
        
        Ok(ArtifactPointer {
            task_id,
            storage: StorageType::Redis,
            location: key,
            size_bytes: data.len() as i64,
            content_type: content_type.to_string(),
        })
    }
    
    async fn read(&self, pointer: &ArtifactPointer) -> Result<Vec<u8>, ContextStoreError> {
        let mut conn = self.conn.clone();
        let data: Vec<u8> = redis::cmd("GET")
            .arg(&pointer.location)
            .query_async(&mut conn)
            .await?;
        Ok(data)
    }
    
    async fn delete(&self, pointer: &ArtifactPointer) -> Result<(), ContextStoreError> {
        let mut conn = self.conn.clone();
        let _: () = redis::cmd("DEL")
            .arg(&pointer.location)
            .query_async(&mut conn)
            .await?;
        Ok(())
    }
}