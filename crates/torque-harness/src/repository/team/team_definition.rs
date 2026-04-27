use crate::db::Database;
use crate::models::v1::team::{
    ArtifactRef, Blocker, Decision, DelegationStatusEntry, PublishedFact, SharedTaskState,
    TeamDefinition, TeamDefinitionCreate, TeamEvent, TeamInstance, TeamInstanceCreate, TeamMember,
    TeamTask, TeamTaskStatus, TriageResult,
};
use async_trait::async_trait;
use chrono::Utc;
use uuid::Uuid;


#[async_trait]
pub trait TeamDefinitionRepository: Send + Sync {
    async fn create(&self, req: &TeamDefinitionCreate) -> anyhow::Result<TeamDefinition>;
    async fn list(&self, limit: i64) -> anyhow::Result<Vec<TeamDefinition>>;
    async fn get(&self, id: Uuid) -> anyhow::Result<Option<TeamDefinition>>;
    async fn delete(&self, id: Uuid) -> anyhow::Result<bool>;
}

pub struct PostgresTeamDefinitionRepository {
    db: Database,
}

impl PostgresTeamDefinitionRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

#[async_trait]
impl TeamDefinitionRepository for PostgresTeamDefinitionRepository {
    async fn create(&self, req: &TeamDefinitionCreate) -> anyhow::Result<TeamDefinition> {
        let row = sqlx::query_as::<_, TeamDefinition>(
            "INSERT INTO v1_team_definitions (name, description, supervisor_agent_definition_id, sub_agents, policy) VALUES ($1, $2, $3, $4, $5) RETURNING *"
        )
        .bind(&req.name)
        .bind(&req.description)
        .bind(req.supervisor_agent_definition_id)
        .bind(serde_json::to_value(&req.sub_agents)?)
        .bind(&req.policy)
        .fetch_one(self.db.pool())
        .await?;
        Ok(row)
    }

    async fn list(&self, limit: i64) -> anyhow::Result<Vec<TeamDefinition>> {
        let rows = sqlx::query_as::<_, TeamDefinition>(
            "SELECT * FROM v1_team_definitions ORDER BY created_at DESC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows)
    }

    async fn get(&self, id: Uuid) -> anyhow::Result<Option<TeamDefinition>> {
        let row =
            sqlx::query_as::<_, TeamDefinition>("SELECT * FROM v1_team_definitions WHERE id = $1")
                .bind(id)
                .fetch_optional(self.db.pool())
                .await?;
        Ok(row)
    }

    async fn delete(&self, id: Uuid) -> anyhow::Result<bool> {
        let result = sqlx::query("DELETE FROM v1_team_definitions WHERE id = $1")
            .bind(id)
            .execute(self.db.pool())
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
