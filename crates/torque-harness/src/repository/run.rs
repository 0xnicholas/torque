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
            INSERT INTO runs (id, webhook_url, async_execution, status)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(run.id)
        .bind(&run.webhook_url)
        .bind(run.async_execution)
        .bind(&run.status)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    async fn get(&self, id: Uuid) -> anyhow::Result<Option<Run>> {
        let row = sqlx::query_as::<_, Run>(
            "SELECT id, webhook_url, async_execution, status FROM runs WHERE id = $1",
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
            "SELECT id, webhook_url, async_execution, status FROM runs WHERE status = $1 LIMIT $2",
        )
        .bind(status)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows)
    }
}