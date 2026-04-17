use async_trait::async_trait;
use crate::db::Database;
use crate::models::v1::agent_instance::{AgentInstance, AgentInstanceCreate, AgentInstanceStatus};
use uuid::Uuid;

#[async_trait]
pub trait AgentInstanceRepository: Send + Sync {
    async fn create(&self, req: &AgentInstanceCreate) -> anyhow::Result<AgentInstance>;
    async fn list(&self, limit: i64) -> anyhow::Result<Vec<AgentInstance>>;
    async fn get(&self, id: Uuid) -> anyhow::Result<Option<AgentInstance>>;
    async fn delete(&self, id: Uuid) -> anyhow::Result<bool>;
    async fn update_status(&self, id: Uuid, status: AgentInstanceStatus) -> anyhow::Result<bool>;
    async fn update_current_task(&self, id: Uuid, task_id: Option<Uuid>) -> anyhow::Result<bool>;
}

pub struct PostgresAgentInstanceRepository { db: Database }
impl PostgresAgentInstanceRepository { pub fn new(db: Database) -> Self { Self { db } } }

#[async_trait]
impl AgentInstanceRepository for PostgresAgentInstanceRepository {
    async fn create(&self, req: &AgentInstanceCreate) -> anyhow::Result<AgentInstance> {
        let row = sqlx::query_as::<_, AgentInstance>(
            "INSERT INTO v1_agent_instances (agent_definition_id, external_context_refs) VALUES ($1, $2) RETURNING *"
        )
        .bind(req.agent_definition_id)
        .bind(serde_json::to_value(&req.external_context_refs)?)
        .fetch_one(self.db.pool())
        .await?;
        Ok(row)
    }

    async fn list(&self, limit: i64) -> anyhow::Result<Vec<AgentInstance>> {
        let rows = sqlx::query_as::<_, AgentInstance>(
            "SELECT * FROM v1_agent_instances ORDER BY created_at DESC LIMIT $1"
        )
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows)
    }

    async fn get(&self, id: Uuid) -> anyhow::Result<Option<AgentInstance>> {
        let row = sqlx::query_as::<_, AgentInstance>("SELECT * FROM v1_agent_instances WHERE id = $1")
            .bind(id)
            .fetch_optional(self.db.pool())
            .await?;
        Ok(row)
    }

    async fn delete(&self, id: Uuid) -> anyhow::Result<bool> {
        let result = sqlx::query("DELETE FROM v1_agent_instances WHERE id = $1")
            .bind(id)
            .execute(self.db.pool())
            .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn update_status(&self, id: Uuid, status: AgentInstanceStatus) -> anyhow::Result<bool> {
        let result = sqlx::query("UPDATE v1_agent_instances SET status = $1, updated_at = NOW() WHERE id = $2")
            .bind(format!("{:?}", status).to_uppercase())
            .bind(id)
            .execute(self.db.pool())
            .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn update_current_task(&self, id: Uuid, task_id: Option<Uuid>) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "UPDATE v1_agent_instances SET current_task_id = $1, updated_at = NOW() WHERE id = $2"
        )
        .bind(task_id)
        .bind(id)
        .execute(self.db.pool())
        .await?;
        Ok(result.rows_affected() > 0)
    }
}
