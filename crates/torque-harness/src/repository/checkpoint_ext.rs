use crate::db::Database;
use crate::models::v1::checkpoint::{Checkpoint, CheckpointRow};
use async_trait::async_trait;
use checkpointer::r#trait::Message;
use uuid::Uuid;

#[async_trait]
pub trait CheckpointRepositoryExt: Send + Sync {
    async fn list(&self, limit: i64) -> anyhow::Result<Vec<Checkpoint>>;
    async fn list_by_instance(
        &self,
        instance_id: Uuid,
        limit: i64,
    ) -> anyhow::Result<Vec<Checkpoint>>;
    async fn get(&self, id: Uuid) -> anyhow::Result<Option<Checkpoint>>;
    async fn get_messages(&self, checkpoint_id: Uuid) -> anyhow::Result<Vec<Message>>;
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
        let rows: Vec<CheckpointRow> = sqlx::query_as::<_, CheckpointRow>(
            "SELECT * FROM v1_checkpoints ORDER BY created_at DESC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn list_by_instance(
        &self,
        instance_id: Uuid,
        limit: i64,
    ) -> anyhow::Result<Vec<Checkpoint>> {
        let rows: Vec<CheckpointRow> = sqlx::query_as::<_, CheckpointRow>(
            "SELECT * FROM v1_checkpoints WHERE agent_instance_id = $1 ORDER BY created_at DESC LIMIT $2"
        )
        .bind(instance_id)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn get(&self, id: Uuid) -> anyhow::Result<Option<Checkpoint>> {
        let row: Option<CheckpointRow> =
            sqlx::query_as::<_, CheckpointRow>("SELECT * FROM v1_checkpoints WHERE id = $1")
                .bind(id)
                .fetch_optional(self.db.pool())
                .await?;
        Ok(row.map(Into::into))
    }

    async fn get_messages(&self, checkpoint_id: Uuid) -> anyhow::Result<Vec<Message>> {
        let checkpoint = self.get(checkpoint_id).await?;
        match checkpoint {
            Some(cp) => {
                let state: checkpointer::CheckpointState = serde_json::from_value(cp.snapshot)?;
                Ok(state.messages)
            }
            None => Ok(Vec::new()),
        }
    }
}
