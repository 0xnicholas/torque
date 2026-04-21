use crate::db::Database;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EphemeralLog {
    pub id: Uuid,
    pub plan_id: Uuid,
    pub task_id: Uuid,
    pub input: Option<String>,
    pub output: Option<String>,
    pub duration_ms: Option<i32>,
    pub status: String,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EphemeralLogCreate {
    pub plan_id: Uuid,
    pub task_id: Uuid,
    pub input: Option<String>,
    pub output: Option<String>,
    pub duration_ms: Option<i32>,
    pub status: String,
    pub error_message: Option<String>,
}

#[async_trait]
pub trait EphemeralLogRepository: Send + Sync {
    async fn create(&self, log: &EphemeralLogCreate) -> anyhow::Result<EphemeralLog>;
    async fn get_by_id(&self, id: Uuid) -> anyhow::Result<Option<EphemeralLog>>;
    async fn list_by_plan(&self, plan_id: Uuid, limit: i64) -> anyhow::Result<Vec<EphemeralLog>>;
    async fn list_by_task(&self, task_id: Uuid, limit: i64) -> anyhow::Result<Vec<EphemeralLog>>;
    async fn update_output(
        &self,
        id: Uuid,
        output: String,
        duration_ms: i32,
        status: &str,
    ) -> anyhow::Result<Option<EphemeralLog>>;
    async fn update_error(
        &self,
        id: Uuid,
        error_message: String,
    ) -> anyhow::Result<Option<EphemeralLog>>;
}

pub struct PostgresEphemeralLogRepository {
    db: Database,
}

impl PostgresEphemeralLogRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

#[async_trait]
impl EphemeralLogRepository for PostgresEphemeralLogRepository {
    async fn create(&self, log: &EphemeralLogCreate) -> anyhow::Result<EphemeralLog> {
        let row = sqlx::query_as::<_, EphemeralLog>(
            r#"
            INSERT INTO ephemeral_logs (plan_id, task_id, input, output, duration_ms, status, error_message)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING *
            "#,
        )
        .bind(log.plan_id)
        .bind(log.task_id)
        .bind(&log.input)
        .bind(&log.output)
        .bind(log.duration_ms)
        .bind(&log.status)
        .bind(&log.error_message)
        .fetch_one(self.db.pool())
        .await?;
        Ok(row)
    }

    async fn get_by_id(&self, id: Uuid) -> anyhow::Result<Option<EphemeralLog>> {
        let row = sqlx::query_as::<_, EphemeralLog>("SELECT * FROM ephemeral_logs WHERE id = $1")
            .bind(id)
            .fetch_optional(self.db.pool())
            .await?;
        Ok(row)
    }

    async fn list_by_plan(&self, plan_id: Uuid, limit: i64) -> anyhow::Result<Vec<EphemeralLog>> {
        let rows = sqlx::query_as::<_, EphemeralLog>(
            "SELECT * FROM ephemeral_logs WHERE plan_id = $1 ORDER BY created_at DESC LIMIT $2",
        )
        .bind(plan_id)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows)
    }

    async fn list_by_task(&self, task_id: Uuid, limit: i64) -> anyhow::Result<Vec<EphemeralLog>> {
        let rows = sqlx::query_as::<_, EphemeralLog>(
            "SELECT * FROM ephemeral_logs WHERE task_id = $1 ORDER BY created_at DESC LIMIT $2",
        )
        .bind(task_id)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows)
    }

    async fn update_output(
        &self,
        id: Uuid,
        output: String,
        duration_ms: i32,
        status: &str,
    ) -> anyhow::Result<Option<EphemeralLog>> {
        let row = sqlx::query_as::<_, EphemeralLog>(
            r#"
            UPDATE ephemeral_logs
            SET output = $2, duration_ms = $3, status = $4
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(output)
        .bind(duration_ms)
        .bind(status)
        .fetch_optional(self.db.pool())
        .await?;
        Ok(row)
    }

    async fn update_error(
        &self,
        id: Uuid,
        error_message: String,
    ) -> anyhow::Result<Option<EphemeralLog>> {
        let row = sqlx::query_as::<_, EphemeralLog>(
            r#"
            UPDATE ephemeral_logs
            SET error_message = $2, status = 'failed'
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(error_message)
        .fetch_optional(self.db.pool())
        .await?;
        Ok(row)
    }
}
