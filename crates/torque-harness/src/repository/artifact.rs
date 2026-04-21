use crate::db::Database;
use crate::models::v1::artifact::{Artifact, ArtifactScope};
use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait ArtifactRepository: Send + Sync {
    async fn create(
        &self,
        kind: &str,
        scope: ArtifactScope,
        mime_type: &str,
        content: serde_json::Value,
    ) -> anyhow::Result<Artifact>;
    async fn list(&self, limit: i64) -> anyhow::Result<Vec<Artifact>>;
    async fn list_by_instance(
        &self,
        instance_id: Uuid,
        limit: i64,
    ) -> anyhow::Result<Vec<Artifact>>;
    async fn get(&self, id: Uuid) -> anyhow::Result<Option<Artifact>>;
    async fn delete(&self, id: Uuid) -> anyhow::Result<bool>;
    async fn update_scope(&self, id: Uuid, scope: ArtifactScope) -> anyhow::Result<bool>;
}

pub struct PostgresArtifactRepository {
    db: Database,
}

impl PostgresArtifactRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

#[async_trait]
impl ArtifactRepository for PostgresArtifactRepository {
    async fn create(
        &self,
        kind: &str,
        scope: ArtifactScope,
        mime_type: &str,
        content: serde_json::Value,
    ) -> anyhow::Result<Artifact> {
        let size_bytes = serde_json::to_string(&content)?.len() as i64;
        let row = sqlx::query_as::<_, Artifact>(
            "INSERT INTO v1_artifacts (kind, scope, mime_type, size_bytes, content) VALUES ($1, $2, $3, $4, $5) RETURNING *"
        )
        .bind(kind)
        .bind(scope)
        .bind(mime_type)
        .bind(size_bytes)
        .bind(content)
        .fetch_one(self.db.pool())
        .await?;
        Ok(row)
    }

    async fn list(&self, limit: i64) -> anyhow::Result<Vec<Artifact>> {
        let rows = sqlx::query_as::<_, Artifact>(
            "SELECT * FROM v1_artifacts ORDER BY created_at DESC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows)
    }

    async fn list_by_instance(
        &self,
        instance_id: Uuid,
        limit: i64,
    ) -> anyhow::Result<Vec<Artifact>> {
        let rows = sqlx::query_as::<_, Artifact>(
            "SELECT * FROM v1_artifacts WHERE source_instance_id = $1 ORDER BY created_at DESC LIMIT $2"
        )
        .bind(instance_id)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows)
    }

    async fn get(&self, id: Uuid) -> anyhow::Result<Option<Artifact>> {
        let row = sqlx::query_as::<_, Artifact>("SELECT * FROM v1_artifacts WHERE id = $1")
            .bind(id)
            .fetch_optional(self.db.pool())
            .await?;
        Ok(row)
    }

    async fn delete(&self, id: Uuid) -> anyhow::Result<bool> {
        let result = sqlx::query("DELETE FROM v1_artifacts WHERE id = $1")
            .bind(id)
            .execute(self.db.pool())
            .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn update_scope(&self, id: Uuid, scope: ArtifactScope) -> anyhow::Result<bool> {
        let result = sqlx::query("UPDATE v1_artifacts SET scope = $1 WHERE id = $2")
            .bind(scope)
            .bind(id)
            .execute(self.db.pool())
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
