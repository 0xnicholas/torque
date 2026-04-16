use async_trait::async_trait;
use crate::db::Database;
use uuid::Uuid;

#[async_trait]
pub trait CheckpointRepository: Send + Sync {
    async fn latest_checkpoint_id(
        &self,
        instance_id: Uuid,
    ) -> anyhow::Result<Option<Uuid>>;
}

pub struct PostgresCheckpointRepository {
    db: Database,
}

impl PostgresCheckpointRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

#[async_trait]
impl CheckpointRepository for PostgresCheckpointRepository {
    async fn latest_checkpoint_id(
        &self,
        instance_id: Uuid,
    ) -> anyhow::Result<Option<Uuid>> {
        let row: Option<(Uuid,)> = sqlx::query_as(
            "SELECT id FROM checkpoints WHERE instance_id = $1 ORDER BY created_at DESC LIMIT 1"
        )
        .bind(instance_id)
        .fetch_optional(self.db.pool())
        .await?;
        Ok(row.map(|(id,)| id))
    }
}
