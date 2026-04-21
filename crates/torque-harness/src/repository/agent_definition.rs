use crate::db::Database;
use crate::models::v1::agent_definition::{AgentDefinition, AgentDefinitionCreate};
use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait AgentDefinitionRepository: Send + Sync {
    async fn create(&self, req: &AgentDefinitionCreate) -> anyhow::Result<AgentDefinition>;
    async fn list(
        &self,
        limit: i64,
        cursor: Option<Uuid>,
        sort: Option<&str>,
    ) -> anyhow::Result<Vec<AgentDefinition>>;
    async fn get(&self, id: Uuid) -> anyhow::Result<Option<AgentDefinition>>;
    async fn delete(&self, id: Uuid) -> anyhow::Result<bool>;
}

pub struct PostgresAgentDefinitionRepository {
    db: Database,
}

impl PostgresAgentDefinitionRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

#[async_trait]
impl AgentDefinitionRepository for PostgresAgentDefinitionRepository {
    async fn create(&self, req: &AgentDefinitionCreate) -> anyhow::Result<AgentDefinition> {
        let row = sqlx::query_as::<_, AgentDefinition>(
            r#"
            INSERT INTO v1_agent_definitions (name, description, system_prompt, tool_policy, memory_policy, delegation_policy, limits, default_model_policy)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING *
            "#
        )
        .bind(&req.name)
        .bind(&req.description)
        .bind(&req.system_prompt)
        .bind(&req.tool_policy)
        .bind(&req.memory_policy)
        .bind(&req.delegation_policy)
        .bind(&req.limits)
        .bind(&req.default_model_policy)
        .fetch_one(self.db.pool())
        .await?;
        Ok(row)
    }

    async fn list(
        &self,
        limit: i64,
        cursor: Option<Uuid>,
        sort: Option<&str>,
    ) -> anyhow::Result<Vec<AgentDefinition>> {
        let order = match sort {
            Some("-created_at") => "created_at DESC, id DESC",
            Some("created_at") => "created_at ASC, id ASC",
            _ => "id ASC",
        };
        let rows = if let Some(after) = cursor {
            sqlx::query_as::<_, AgentDefinition>(&format!(
                "SELECT * FROM v1_agent_definitions WHERE id > $1 ORDER BY {} LIMIT $2",
                order
            ))
            .bind(after)
            .bind(limit)
            .fetch_all(self.db.pool())
            .await?
        } else {
            sqlx::query_as::<_, AgentDefinition>(&format!(
                "SELECT * FROM v1_agent_definitions ORDER BY {} LIMIT $1",
                order
            ))
            .bind(limit)
            .fetch_all(self.db.pool())
            .await?
        };
        Ok(rows)
    }

    async fn get(&self, id: Uuid) -> anyhow::Result<Option<AgentDefinition>> {
        let row = sqlx::query_as::<_, AgentDefinition>(
            "SELECT * FROM v1_agent_definitions WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(self.db.pool())
        .await?;
        Ok(row)
    }

    async fn delete(&self, id: Uuid) -> anyhow::Result<bool> {
        let result = sqlx::query("DELETE FROM v1_agent_definitions WHERE id = $1")
            .bind(id)
            .execute(self.db.pool())
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
