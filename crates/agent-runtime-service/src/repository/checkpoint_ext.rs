use async_trait::async_trait;
use crate::db::Database;
use crate::models::v1::checkpoint::Checkpoint;
use uuid::Uuid;

#[async_trait]
pub trait CheckpointRepositoryExt: Send + Sync {
    async fn list(&self, limit: i64) -> anyhow::Result<Vec<Checkpoint>>;
    async fn list_by_instance(&self, instance_id: Uuid, limit: i64) -> anyhow::Result<Vec<Checkpoint>>;
    async fn get(&self, id: Uuid) -> anyhow::Result<Option<Checkpoint>>;
}

pub struct PostgresCheckpointRepositoryExt {
    db: Database,
}

impl PostgresCheckpointRepositoryExt {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

#[async_trait]
impl CheckpointRepositoryExt for PostgresCheckpointRepositoryExt {
    async fn list(&self, limit: i64) -> anyhow::Result<Vec<Checkpoint>> {
        let rows = sqlx::query_as::<_, Checkpoint>(
            "SELECT * FROM v1_checkpoints ORDER BY created_at DESC LIMIT $1"
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
    ) -> anyhow::Result<Vec<Checkpoint>> {
        let rows = sqlx::query_as::<_, Checkpoint>(
            "SELECT * FROM v1_checkpoints WHERE agent_instance_id = $1 ORDER BY created_at DESC LIMIT $2"
        )
        .bind(instance_id)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows)
    }

    async fn get(&self, id: Uuid) -> anyhow::Result<Option<Checkpoint>> {
        let row = sqlx::query_as::<_, Checkpoint>("SELECT * FROM v1_checkpoints WHERE id = $1")
            .bind(id)
            .fetch_optional(self.db.pool())
            .await?;
        Ok(row)
    }
}
