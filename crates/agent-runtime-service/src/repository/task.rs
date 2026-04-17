use async_trait::async_trait;
use crate::db::Database;
use crate::models::v1::task::Task;
use uuid::Uuid;

#[async_trait]
pub trait TaskRepository: Send + Sync {
    async fn list(&self, limit: i64) -> anyhow::Result<Vec<Task>>;
    async fn get(&self, id: Uuid) -> anyhow::Result<Option<Task>>;
    async fn cancel(&self, id: Uuid) -> anyhow::Result<bool>;
}

pub struct PostgresTaskRepository {
    db: Database,
}

impl PostgresTaskRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

#[async_trait]
impl TaskRepository for PostgresTaskRepository {
    async fn list(&self, limit: i64) -> anyhow::Result<Vec<Task>> {
        let rows = sqlx::query_as::<_, Task>(
            "SELECT * FROM v1_tasks ORDER BY created_at DESC LIMIT $1"
        )
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows)
    }

    async fn get(&self, id: Uuid) -> anyhow::Result<Option<Task>> {
        let row = sqlx::query_as::<_, Task>("SELECT * FROM v1_tasks WHERE id = $1")
            .bind(id)
            .fetch_optional(self.db.pool())
            .await?;
        Ok(row)
    }

    async fn cancel(&self, id: Uuid) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "UPDATE v1_tasks SET status = 'CANCELLED', updated_at = NOW() WHERE id = $1"
        )
        .bind(id)
        .execute(self.db.pool())
        .await?;
        Ok(result.rows_affected() > 0)
    }
}
