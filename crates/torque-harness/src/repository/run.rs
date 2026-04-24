use crate::db::Database;
use crate::models::v1::{Run, RunStatus};
use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait RunRepository: Send + Sync {
    async fn create(&self, run: &Run) -> anyhow::Result<()>;
    async fn get(&self, id: Uuid) -> anyhow::Result<Option<Run>>;
    async fn update_status(&self, id: Uuid, status: RunStatus) -> anyhow::Result<()>;
    async fn get_by_status(&self, status: RunStatus, limit: i64) -> anyhow::Result<Vec<Run>>;
}

pub struct PostgresRunRepository {
    db: Database,
}

impl PostgresRunRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

#[async_trait]
impl RunRepository for PostgresRunRepository {
    async fn create(&self, run: &Run) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO runs (id, tenant_id, status, instruction, failure_policy, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(run.id)
        .bind(run.tenant_id)
        .bind(run.status.to_string())
        .bind(&run.instruction)
        .bind(&run.failure_policy)
        .bind(run.created_at)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    async fn get(&self, id: Uuid) -> anyhow::Result<Option<Run>> {
        let row = sqlx::query_as::<_, Run>(
            "SELECT id, tenant_id, status, instruction, failure_policy, webhook_url, async_execution, created_at, started_at, completed_at, error FROM runs WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(self.db.pool())
        .await?;
        Ok(row)
    }

    async fn update_status(&self, id: Uuid, status: RunStatus) -> anyhow::Result<()> {
        sqlx::query("UPDATE runs SET status = $1 WHERE id = $2")
            .bind(&status)
            .bind(id)
            .execute(self.db.pool())
            .await?;
        Ok(())
    }

    async fn get_by_status(&self, status: RunStatus, limit: i64) -> anyhow::Result<Vec<Run>> {
        let rows = sqlx::query_as::<_, Run>(
            "SELECT id, tenant_id, status, instruction, failure_policy, webhook_url, async_execution, created_at, started_at, completed_at, error FROM runs WHERE status = $1 LIMIT $2",
        )
        .bind(status.to_string())
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows)
    }
}