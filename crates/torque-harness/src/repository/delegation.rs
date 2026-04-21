use crate::db::Database;
use crate::models::v1::delegation::{Delegation, DelegationStatus};
use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait DelegationRepository: Send + Sync {
    async fn create(
        &self,
        task_id: Uuid,
        parent_instance_id: Uuid,
        selector: serde_json::Value,
    ) -> anyhow::Result<Delegation>;
    async fn list(&self, limit: i64) -> anyhow::Result<Vec<Delegation>>;
    async fn list_by_instance(
        &self,
        instance_id: Uuid,
        limit: i64,
    ) -> anyhow::Result<Vec<Delegation>>;
    async fn list_by_task(&self, task_id: Uuid, limit: i64) -> anyhow::Result<Vec<Delegation>>;
    async fn get(&self, id: Uuid) -> anyhow::Result<Option<Delegation>>;
    async fn update_status(&self, id: Uuid, status: &str) -> anyhow::Result<bool>;
    async fn complete(&self, id: Uuid, artifact_id: Uuid) -> anyhow::Result<bool>;
    async fn fail(&self, id: Uuid, error: &str) -> anyhow::Result<bool>;
    async fn reject(&self, id: Uuid, reason: &str) -> anyhow::Result<bool>;
    async fn list_by_status(&self, task_id: Uuid, status: DelegationStatus) -> anyhow::Result<Vec<Delegation>>;
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
            "SELECT * FROM v1_delegations ORDER BY created_at DESC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows)
    }

    async fn list_by_instance(
        &self,
        instance_id: Uuid,
        limit: i64,
    ) -> anyhow::Result<Vec<Delegation>> {
        let rows = sqlx::query_as::<_, Delegation>(
            "SELECT * FROM v1_delegations WHERE parent_agent_instance_id = $1 ORDER BY created_at DESC LIMIT $2"
        )
        .bind(instance_id)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows)
    }

    async fn list_by_task(&self, task_id: Uuid, limit: i64) -> anyhow::Result<Vec<Delegation>> {
        let rows = sqlx::query_as::<_, Delegation>(
            "SELECT * FROM v1_delegations WHERE task_id = $1 ORDER BY created_at DESC LIMIT $2",
        )
        .bind(task_id)
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
        let result = sqlx::query("UPDATE v1_delegations SET status = $1 WHERE id = $2")
            .bind(status)
            .bind(id)
            .execute(self.db.pool())
            .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn complete(&self, id: Uuid, artifact_id: Uuid) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "UPDATE v1_delegations SET status = 'COMPLETED', result_artifact_id = $1, updated_at = NOW() WHERE id = $2"
        )
        .bind(artifact_id)
        .bind(id)
        .execute(self.db.pool())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn fail(&self, id: Uuid, error: &str) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "UPDATE v1_delegations SET status = 'FAILED', error_message = $1, updated_at = NOW() WHERE id = $2"
        )
        .bind(error)
        .bind(id)
        .execute(self.db.pool())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn reject(&self, id: Uuid, reason: &str) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "UPDATE v1_delegations SET status = 'REJECTED', rejection_reason = $1, updated_at = NOW() WHERE id = $2"
        )
        .bind(reason)
        .bind(id)
        .execute(self.db.pool())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn list_by_status(&self, task_id: Uuid, status: DelegationStatus) -> anyhow::Result<Vec<Delegation>> {
        let rows = sqlx::query_as::<_, Delegation>(
            "SELECT * FROM v1_delegations WHERE task_id = $1 AND status = $2 ORDER BY created_at DESC"
        )
        .bind(task_id)
        .bind(status)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows)
    }
}
