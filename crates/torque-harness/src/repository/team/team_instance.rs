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
pub trait TeamInstanceRepository: Send + Sync {
    async fn create(&self, req: &TeamInstanceCreate) -> anyhow::Result<TeamInstance>;
    async fn list(&self, limit: i64) -> anyhow::Result<Vec<TeamInstance>>;
    async fn get(&self, id: Uuid) -> anyhow::Result<Option<TeamInstance>>;
    async fn delete(&self, id: Uuid) -> anyhow::Result<bool>;
}

pub struct PostgresTeamInstanceRepository {
    db: Database,
}

impl PostgresTeamInstanceRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

#[async_trait]
impl TeamInstanceRepository for PostgresTeamInstanceRepository {
    async fn create(&self, req: &TeamInstanceCreate) -> anyhow::Result<TeamInstance> {
        let row = sqlx::query_as::<_, TeamInstance>(
            "INSERT INTO v1_team_instances (team_definition_id) VALUES ($1) RETURNING *",
        )
        .bind(req.team_definition_id)
        .fetch_one(self.db.pool())
        .await?;
        Ok(row)
    }

    async fn list(&self, limit: i64) -> anyhow::Result<Vec<TeamInstance>> {
        let rows = sqlx::query_as::<_, TeamInstance>(
            "SELECT * FROM v1_team_instances ORDER BY created_at DESC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows)
    }

    async fn get(&self, id: Uuid) -> anyhow::Result<Option<TeamInstance>> {
        let row =
            sqlx::query_as::<_, TeamInstance>("SELECT * FROM v1_team_instances WHERE id = $1")
                .bind(id)
                .fetch_optional(self.db.pool())
                .await?;
        Ok(row)
    }

    async fn delete(&self, id: Uuid) -> anyhow::Result<bool> {
        let result = sqlx::query("DELETE FROM v1_team_instances WHERE id = $1")
            .bind(id)
            .execute(self.db.pool())
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
