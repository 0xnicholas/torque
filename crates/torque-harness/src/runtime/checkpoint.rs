use crate::db::Database;
use async_trait::async_trait;
use checkpointer::{CheckpointId, CheckpointMeta, CheckpointState, Checkpointer};
use chrono::{DateTime, Utc};
use uuid::Uuid;

pub struct PostgresCheckpointer {
    db: Database,
}

impl PostgresCheckpointer {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Checkpointer for PostgresCheckpointer {
    async fn save(
        &self,
        run_id: Uuid,
        node_id: Uuid,
        state: CheckpointState,
    ) -> checkpointer::Result<CheckpointId> {
        let id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO v1_checkpoints (id, agent_instance_id, task_id, snapshot, created_at)
            VALUES ($1, $2, $3, $4, NOW())
            "#,
        )
        .bind(id)
        .bind(run_id)
        .bind(node_id)
        .bind(
            serde_json::to_value(&state)
                .map_err(|e| checkpointer::CheckpointerError::Serialization(e.to_string()))?,
        )
        .execute(self.db.pool())
        .await
        .map_err(checkpointer::CheckpointerError::Database)?;
        Ok(CheckpointId(id))
    }

    async fn load(&self, checkpoint_id: CheckpointId) -> checkpointer::Result<CheckpointState> {
        let row: Option<(serde_json::Value,)> =
            sqlx::query_as("SELECT snapshot FROM v1_checkpoints WHERE id = $1")
                .bind(checkpoint_id.0)
                .fetch_optional(self.db.pool())
                .await
                .map_err(checkpointer::CheckpointerError::Database)?;

        let (json,) = row.ok_or_else(|| {
            checkpointer::CheckpointerError::NotFound(format!(
                "checkpoint not found: {}",
                checkpoint_id.0
            ))
        })?;
        serde_json::from_value(json)
            .map_err(|e| checkpointer::CheckpointerError::Serialization(e.to_string()))
    }

    async fn list_run_checkpoints(
        &self,
        run_id: Uuid,
    ) -> checkpointer::Result<Vec<CheckpointMeta>> {
        let rows: Vec<(Uuid, Option<Uuid>, DateTime<Utc>)> = sqlx::query_as(
            "SELECT id, task_id, created_at FROM v1_checkpoints WHERE agent_instance_id = $1 ORDER BY created_at DESC",
        )
        .bind(run_id)
        .fetch_all(self.db.pool())
        .await
        .map_err(checkpointer::CheckpointerError::Database)?;

        Ok(rows
            .into_iter()
            .map(|(id, task_id, created_at)| CheckpointMeta {
                id: CheckpointId(id),
                run_id,
                node_id: task_id.unwrap_or(run_id),
                created_at,
                state_hash: String::new(),
            })
            .collect())
    }

    async fn list_node_checkpoints(
        &self,
        node_id: Uuid,
    ) -> checkpointer::Result<Vec<CheckpointMeta>> {
        let rows: Vec<(Uuid, Uuid, DateTime<Utc>)> = sqlx::query_as(
            "SELECT id, agent_instance_id, created_at FROM v1_checkpoints WHERE task_id = $1 ORDER BY created_at DESC",
        )
        .bind(node_id)
        .fetch_all(self.db.pool())
        .await
        .map_err(checkpointer::CheckpointerError::Database)?;

        Ok(rows
            .into_iter()
            .map(|(id, run_id, created_at)| CheckpointMeta {
                id: CheckpointId(id),
                run_id,
                node_id,
                created_at,
                state_hash: String::new(),
            })
            .collect())
    }

    async fn delete(&self, checkpoint_id: CheckpointId) -> checkpointer::Result<()> {
        sqlx::query("DELETE FROM v1_checkpoints WHERE id = $1")
            .bind(checkpoint_id.0)
            .execute(self.db.pool())
            .await
            .map_err(checkpointer::CheckpointerError::Database)?;
        Ok(())
    }
}
