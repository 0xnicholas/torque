use async_trait::async_trait;
use crate::db::Database;
use crate::models::v1::delegation::Delegation;
use uuid::Uuid;

#[async_trait]
pub trait DelegationRepository: Send + Sync {
    async fn create(&self, task_id: Uuid, parent_instance_id: Uuid, selector: serde_json::Value) -> anyhow::Result<Delegation>;
    async fn list(&self, limit: i64) -> anyhow::Result<Vec<Delegation>>;
    async fn get(&self, id: Uuid) -> anyhow::Result<Option<Delegation>>;
    async fn update_status(&self, id: Uuid, status: &str) -> anyhow::Result<bool>;
}

pub struct PostgresDelegationRepository {
    db: Database,
}

impl PostgresDelegationRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

#[async_trait]
impl DelegationRepository for PostgresDelegationRepository {
    async fn create(
        &self,
        task_id: Uuid,
        parent_instance_id: Uuid,
        selector: serde_json::Value,
    ) -> anyhow::Result<Delegation> {
        let row = sqlx::query_as::<_, Delegation>(
            "INSERT INTO v1_delegations (task_id, parent_agent_instance_id, child_agent_definition_selector, status) VALUES ($1, $2, $3, 'PENDING') RETURNING *"
        )
        .bind(task_id)
        .bind(parent_instance_id)
        .bind(selector)
        .fetch_one(self.db.pool())
        .await?;
        Ok(row)
    }

    async fn list(&self, limit: i64) -> anyhow::Result<Vec<Delegation>> {
        let rows = sqlx::query_as::<_, Delegation>(
            "SELECT * FROM v1_delegations ORDER BY created_at DESC LIMIT $1"
        )
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows)
    }

    async fn get(&self, id: Uuid) -> anyhow::Result<Option<Delegation>> {
        let row = sqlx::query_as::<_, Delegation>("SELECT * FROM v1_delegations WHERE id = $1")
            .bind(id)
            .fetch_optional(self.db.pool())
            .await?;
        Ok(row)
    }

    async fn update_status(&self, id: Uuid, status: &str) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "UPDATE v1_delegations SET status = $1 WHERE id = $2"
        )
        .bind(status)
        .bind(id)
        .execute(self.db.pool())
        .await?;
        Ok(result.rows_affected() > 0)
    }
}
