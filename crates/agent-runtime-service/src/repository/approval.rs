use async_trait::async_trait;
use crate::db::Database;
use crate::models::v1::approval::Approval;
use uuid::Uuid;

#[async_trait]
pub trait ApprovalRepository: Send + Sync {
    async fn list(&self, limit: i64) -> anyhow::Result<Vec<Approval>>;
    async fn get(&self, id: Uuid) -> anyhow::Result<Option<Approval>>;
    async fn resolve(&self, id: Uuid, status: &str) -> anyhow::Result<bool>;
}

pub struct PostgresApprovalRepository {
    db: Database,
}

impl PostgresApprovalRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

#[async_trait]
impl ApprovalRepository for PostgresApprovalRepository {
    async fn list(&self, limit: i64) -> anyhow::Result<Vec<Approval>> {
        let rows = sqlx::query_as::<_, Approval>(
            "SELECT * FROM v1_approvals ORDER BY requested_at DESC LIMIT $1"
        )
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows)
    }

    async fn get(&self, id: Uuid) -> anyhow::Result<Option<Approval>> {
        let row = sqlx::query_as::<_, Approval>("SELECT * FROM v1_approvals WHERE id = $1")
            .bind(id)
            .fetch_optional(self.db.pool())
            .await?;
        Ok(row)
    }

    async fn resolve(&self, id: Uuid, status: &str) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "UPDATE v1_approvals SET status = $1, resolved_at = NOW() WHERE id = $2"
        )
        .bind(status)
        .bind(id)
        .execute(self.db.pool())
        .await?;
        Ok(result.rows_affected() > 0)
    }
}
