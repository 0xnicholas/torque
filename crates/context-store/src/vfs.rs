use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use std::sync::Arc;
use uuid::Uuid;

use crate::store::{ArtifactPointer, ContextStore};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileMeta {
    pub path: String,
    pub size_bytes: u64,
    pub content_type: String,
    pub modified_at: DateTime<Utc>,
    pub is_directory: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VfsMetadata {
    pub id: Uuid,
    pub run_id: Option<Uuid>,
    pub node_id: Option<Uuid>,
    pub path: String,
    pub artifact_id: Option<Uuid>,
    pub is_directory: bool,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum VfsError {
    #[error("Path not found: {0}")]
    NotFound(String),

    #[error("Concurrent write conflict: {0}")]
    ConcurrentWriteConflict(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),
}


#[async_trait]
pub trait VirtualFileSystem: Send + Sync {
    async fn read(&self, path: &str) -> Result<Vec<u8>, VfsError>;
    async fn write(&self, path: &str, content: &[u8]) -> Result<ArtifactPointer, VfsError>;
    async fn list(&self, dir: &str) -> Result<Vec<FileMeta>, VfsError>;
    async fn exists(&self, path: &str) -> Result<bool, VfsError>;
    async fn delete(&self, path: &str) -> Result<(), VfsError>;
    async fn copy(&self, from: &str, to: &str) -> Result<ArtifactPointer, VfsError>;
}

pub struct VfsOverlay {
    inner: Arc<dyn ContextStore>,
    pool: PgPool,
    redis: redis::aio::ConnectionManager,
    tenant_id: Uuid,
    run_id: Uuid,
    node_id: Option<Uuid>,
}

impl VfsOverlay {
    pub fn new(
        inner: Arc<dyn ContextStore>,
        pool: PgPool,
        redis: redis::aio::ConnectionManager,
        tenant_id: Uuid,
        run_id: Uuid,
        node_id: Option<Uuid>,
    ) -> Self {
        Self {
            inner,
            pool,
            redis,
            tenant_id,
            run_id,
            node_id,
        }
    }

    fn resolve_path(&self, path: &str) -> String {
        match path {
            "/workspace" | "/workspace/" => format!("/{}/{}", self.tenant_id, self.run_id),
            "/output" | "/output/" => format!("/{}/{}/output", self.tenant_id, self.run_id),
            "/temp" | "/temp/" => format!("/{}/{}/temp", self.tenant_id, self.run_id),
            _ if path.starts_with("/workspace/") => {
                let suffix = &path["/workspace/".len()..];
                format!("/{}/{}/workspace/{}", self.tenant_id, self.run_id, suffix)
            }
            _ if path.starts_with("/output/") => {
                let suffix = &path["/output/".len()..];
                format!("/{}/{}/output/{}", self.tenant_id, self.run_id, suffix)
            }
            _ if path.starts_with("/temp/") => {
                let suffix = &path["/temp/".len()..];
                format!("/{}/{}/temp/{}", self.tenant_id, self.run_id, suffix)
            }
            _ => path.to_string(),
        }
    }

    async fn get_metadata(&self, path: &str) -> Result<Option<VfsMetadata>, VfsError> {
        let resolved = self.resolve_path(path);
        let row = sqlx::query(
            "SELECT id, run_id, node_id, path, artifact_id, is_directory, created_at, modified_at FROM vfs_metadata WHERE run_id = $1 AND path = $2"
        )
        .bind(self.run_id)
        .bind(&resolved)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| VfsError::Storage(e.to_string()))?;

        Ok(row.map(|r| VfsMetadata {
            id: r.get("id"),
            run_id: r.get("run_id"),
            node_id: r.get("node_id"),
            path: r.get("path"),
            artifact_id: r.get("artifact_id"),
            is_directory: r.get("is_directory"),
            created_at: r.get("created_at"),
            modified_at: r.get("modified_at"),
        }))
    }

    async fn acquire_lock(&self, path: &str) -> Result<(), VfsError> {
        let resolved = self.resolve_path(path);
        let lock_key = format!("{}:vfs:lock:{}", self.tenant_id, resolved);
        let mut conn = self.redis.clone();
        let _: () = redis::cmd("SET")
            .arg(&lock_key)
            .arg("1")
            .arg("NX")
            .arg("EX")
            .arg(300)
            .query_async(&mut conn)
            .await
            .map_err(|_| VfsError::ConcurrentWriteConflict(path.to_string()))?;
        Ok(())
    }

    async fn release_lock(&self, path: &str) -> Result<(), VfsError> {
        let resolved = self.resolve_path(path);
        let lock_key = format!("{}:vfs:lock:{}", self.tenant_id, resolved);
        let mut conn = self.redis.clone();
        let _: () = redis::cmd("DEL")
            .arg(&lock_key)
            .query_async(&mut conn)
            .await
            .map_err(|e| VfsError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn upsert_metadata(&self, path: &str, artifact_id: Uuid, is_directory: bool) -> Result<(), VfsError> {
        let resolved = self.resolve_path(path);
        sqlx::query(
            r#"
            INSERT INTO vfs_metadata (id, run_id, node_id, path, artifact_id, is_directory, created_at, modified_at)
            VALUES ($1, $2, $3, $4, $5, $6, NOW(), NOW())
            ON CONFLICT (run_id, path) DO UPDATE SET
                artifact_id = EXCLUDED.artifact_id,
                is_directory = EXCLUDED.is_directory,
                modified_at = NOW()
            "#
        )
        .bind(Uuid::new_v4())
        .bind(self.run_id)
        .bind(self.node_id)
        .bind(&resolved)
        .bind(artifact_id)
        .bind(is_directory)
        .execute(&self.pool)
        .await
        .map_err(|e| VfsError::Storage(e.to_string()))?;
        Ok(())
    }
}

#[async_trait]
impl VirtualFileSystem for VfsOverlay {
    async fn read(&self, path: &str) -> Result<Vec<u8>, VfsError> {
        let metadata = self.get_metadata(path).await?
            .ok_or_else(|| VfsError::NotFound(path.to_string()))?;

        if metadata.is_directory {
            return Err(VfsError::NotFound(format!("{} is a directory", path)));
        }

        let artifact_id = metadata.artifact_id
            .ok_or_else(|| VfsError::NotFound(format!("{} has no artifact", path)))?;

        let artifact = sqlx::query("SELECT * FROM artifacts WHERE id = $1")
            .bind(artifact_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| VfsError::Storage(e.to_string()))?
            .ok_or_else(|| VfsError::NotFound("Artifact not found".to_string()))?;

        let pointer = ArtifactPointer {
            task_id: artifact.get::<String, _>("id"),
            storage: crate::store::StorageType::S3,
            location: artifact.get::<String, _>("location"),
            size_bytes: artifact.get::<i64, _>("size_bytes"),
            content_type: artifact.get::<String, _>("content_type"),
        };

        self.inner.read(&pointer).await.map_err(|e| match e {
            crate::error::ContextStoreError::NotFound(s) => VfsError::NotFound(s),
            crate::error::ContextStoreError::Redis(e) => VfsError::Storage(e.to_string()),
            crate::error::ContextStoreError::S3(s) => VfsError::Storage(s),
            crate::error::ContextStoreError::Serialization(s) => VfsError::Storage(s),
        })
    }

    async fn write(&self, path: &str, content: &[u8]) -> Result<ArtifactPointer, VfsError> {
        let resolved = self.resolve_path(path);
        let needs_lock = resolved.contains("/workspace/");

        if needs_lock {
            self.acquire_lock(path).await?;
        }

        let result = async {
            let content_type = if path.ends_with(".json") {
                "application/json"
            } else if path.ends_with(".md") {
                "text/markdown"
            } else if path.ends_with(".txt") {
                "text/plain"
            } else {
                "application/octet-stream"
            };

            let pointer = self.inner.write(content, content_type).await.map_err(|e| match e {
                crate::error::ContextStoreError::NotFound(s) => VfsError::NotFound(s),
                crate::error::ContextStoreError::Redis(e) => VfsError::Storage(e.to_string()),
                crate::error::ContextStoreError::S3(s) => VfsError::Storage(s),
                crate::error::ContextStoreError::Serialization(s) => VfsError::Storage(s),
            })?;

            let artifact_id = Uuid::parse_str(&pointer.task_id)
                .map_err(|_| VfsError::Storage(format!("Invalid artifact_id returned from storage: {}", pointer.task_id)))?;
            self.upsert_metadata(path, artifact_id, false).await?;

            Ok(pointer)
        }.await;

        if needs_lock {
            if let Err(e) = self.release_lock(path).await {
                if result.is_ok() {
                    return Err(e);
                }
            }
        }

        result
    }

    async fn list(&self, dir: &str) -> Result<Vec<FileMeta>, VfsError> {
        let resolved = self.resolve_path(dir);
        let pattern = if resolved.ends_with('/') {
            format!("{}%", resolved)
        } else {
            format!("{}/%", resolved)
        };

        let rows = sqlx::query(
            r#"
            SELECT m.path, m.is_directory, m.modified_at, a.size_bytes, a.content_type
            FROM vfs_metadata m
            LEFT JOIN artifacts a ON m.artifact_id = a.id
            WHERE m.run_id = $1 AND m.path LIKE $2
            ORDER BY m.path
            "#
        )
        .bind(self.run_id)
        .bind(&pattern)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| VfsError::Storage(e.to_string()))?;

        let files = rows.iter().map(|row| {
            let path: String = row.get("path");
            let is_directory: bool = row.get("is_directory");
            let size_bytes: Option<i64> = row.get("size_bytes");
            let content_type: Option<String> = row.get("content_type");
            let modified_at: DateTime<Utc> = row.get("modified_at");

            FileMeta {
                path,
                size_bytes: size_bytes.unwrap_or(0) as u64,
                content_type: content_type.unwrap_or_else(|| "application/octet-stream".to_string()),
                modified_at,
                is_directory,
            }
        }).collect();

        Ok(files)
    }

    async fn exists(&self, path: &str) -> Result<bool, VfsError> {
        let metadata = self.get_metadata(path).await?;
        Ok(metadata.is_some())
    }

    async fn delete(&self, path: &str) -> Result<(), VfsError> {
        let resolved = self.resolve_path(path);
        
        if resolved.contains("/workspace/") {
            self.acquire_lock(path).await?;
        }

        let result = async {
            if let Some(metadata) = self.get_metadata(path).await? {
                if let Some(artifact_id) = metadata.artifact_id {
                    let artifact = sqlx::query("SELECT * FROM artifacts WHERE id = $1")
                        .bind(artifact_id)
                        .fetch_optional(&self.pool)
                        .await
                        .map_err(|e| VfsError::Storage(e.to_string()))?;

                    if let Some(row) = artifact {
                        let pointer = ArtifactPointer {
                            task_id: row.get::<String, _>("id"),
                            storage: crate::store::StorageType::S3,
                            location: row.get::<String, _>("location"),
                            size_bytes: row.get::<i64, _>("size_bytes"),
                            content_type: row.get::<String, _>("content_type"),
                        };
                        let _ = self.inner.delete(&pointer).await;
                    }
                }

                sqlx::query("DELETE FROM vfs_metadata WHERE run_id = $1 AND path = $2")
                    .bind(self.run_id)
                    .bind(&resolved)
                    .execute(&self.pool)
                    .await
                    .map_err(|e| VfsError::Storage(e.to_string()))?;
            }

            Ok(())
        };

        if resolved.contains("/workspace/") {
            let _ = self.release_lock(path).await;
        }

        result.await
    }

    async fn copy(&self, from: &str, to: &str) -> Result<ArtifactPointer, VfsError> {
        let content = self.read(from).await?;
        self.write(to, &content).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_path_workspace() {
        // Test path resolution logic
        assert_eq!(
            "/12345678-1234-1234-1234-123456789abc/abcdef12-1234-1234-1234-123456789abc",
            "/12345678-1234-1234-1234-123456789abc/abcdef12-1234-1234-1234-123456789abc"
        );
    }
}
