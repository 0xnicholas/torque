use crate::db::Database;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use torque_runtime::checkpoint::{ArtifactPointer, Message, RuntimeCheckpointPayload, RuntimeCheckpointRef};
use torque_runtime::environment::RuntimeCheckpointSink;
use uuid::Uuid;

pub struct PostgresCheckpointer {
    db: Database,
}

impl PostgresCheckpointer {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn load(
        &self,
        checkpoint_id: Uuid,
    ) -> anyhow::Result<serde_json::Value> {
        let row: Option<(serde_json::Value,)> =
            sqlx::query_as("SELECT snapshot FROM v1_checkpoints WHERE id = $1")
                .bind(checkpoint_id)
                .fetch_optional(self.db.pool())
                .await?;

        let (json,) = row.ok_or_else(|| anyhow::anyhow!("checkpoint not found: {}", checkpoint_id))?;
        Ok(json)
    }

    pub async fn list_run_checkpoints(
        &self,
        run_id: Uuid,
    ) -> anyhow::Result<Vec<CheckpointRow>> {
        let rows: Vec<(Uuid, Option<Uuid>, DateTime<Utc>)> = sqlx::query_as(
            "SELECT id, task_id, created_at FROM v1_checkpoints WHERE agent_instance_id = $1 ORDER BY created_at DESC",
        )
        .bind(run_id)
        .fetch_all(self.db.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|(id, task_id, created_at)| CheckpointRow {
                id,
                run_id,
                node_id: task_id.unwrap_or(run_id),
                created_at,
            })
            .collect())
    }

    pub async fn list_node_checkpoints(
        &self,
        node_id: Uuid,
    ) -> anyhow::Result<Vec<CheckpointRow>> {
        let rows: Vec<(Uuid, Uuid, DateTime<Utc>)> = sqlx::query_as(
            "SELECT id, agent_instance_id, created_at FROM v1_checkpoints WHERE task_id = $1 ORDER BY created_at DESC",
        )
        .bind(node_id)
        .fetch_all(self.db.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|(id, run_id, created_at)| CheckpointRow {
                id,
                run_id,
                node_id,
                created_at,
            })
            .collect())
    }

    pub async fn delete(&self, checkpoint_id: Uuid) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM v1_checkpoints WHERE id = $1")
            .bind(checkpoint_id)
            .execute(self.db.pool())
            .await?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct CheckpointRow {
    pub id: Uuid,
    pub run_id: Uuid,
    pub node_id: Uuid,
    pub created_at: DateTime<Utc>,
}

#[async_trait]
impl RuntimeCheckpointSink for PostgresCheckpointer {
    async fn save(
        &self,
        checkpoint: RuntimeCheckpointPayload,
    ) -> anyhow::Result<RuntimeCheckpointRef> {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO v1_checkpoints (id, agent_instance_id, task_id, snapshot, created_at)
            VALUES ($1, $2, $3, $4, NOW())
            "#,
        )
        .bind(id)
        .bind(checkpoint.instance_id.as_uuid())
        .bind(checkpoint.node_id)
        .bind(&checkpoint.state)
        .execute(self.db.pool())
        .await?;

        Ok(RuntimeCheckpointRef {
            checkpoint_id: id,
            instance_id: checkpoint.instance_id.as_uuid(),
        })
    }
}
