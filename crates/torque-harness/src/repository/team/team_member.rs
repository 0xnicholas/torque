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
pub trait TeamMemberRepository: Send + Sync {
    async fn create(
        &self,
        team_instance_id: Uuid,
        agent_instance_id: Uuid,
        role: &str,
    ) -> anyhow::Result<TeamMember>;
    async fn list_by_team(
        &self,
        team_instance_id: Uuid,
        limit: i64,
    ) -> anyhow::Result<Vec<TeamMember>>;
    async fn remove(&self, team_instance_id: Uuid, agent_instance_id: Uuid)
        -> anyhow::Result<bool>;
}

pub struct PostgresTeamMemberRepository {
    db: Database,
}

impl PostgresTeamMemberRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

#[async_trait]
impl TeamMemberRepository for PostgresTeamMemberRepository {
    async fn create(
        &self,
        team_instance_id: Uuid,
        agent_instance_id: Uuid,
        role: &str,
    ) -> anyhow::Result<TeamMember> {
        let row = sqlx::query_as::<_, TeamMember>(
            "INSERT INTO v1_team_members (team_instance_id, agent_instance_id, role) VALUES ($1, $2, $3) RETURNING *"
        )
        .bind(team_instance_id)
        .bind(agent_instance_id)
        .bind(role)
        .fetch_one(self.db.pool())
        .await?;
        Ok(row)
    }

    async fn list_by_team(
        &self,
        team_instance_id: Uuid,
        limit: i64,
    ) -> anyhow::Result<Vec<TeamMember>> {
        let rows = sqlx::query_as::<_, TeamMember>(
            "SELECT * FROM v1_team_members WHERE team_instance_id = $1 ORDER BY created_at DESC LIMIT $2"
        )
        .bind(team_instance_id)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows)
    }

    async fn remove(
        &self,
        team_instance_id: Uuid,
        agent_instance_id: Uuid,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "DELETE FROM v1_team_members WHERE team_instance_id = $1 AND agent_instance_id = $2",
        )
        .bind(team_instance_id)
        .bind(agent_instance_id)
        .execute(self.db.pool())
        .await?;
        Ok(result.rows_affected() > 0)
    }
}
