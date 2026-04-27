use crate::db::Database;
use crate::models::v1::team::{
    ArtifactRef, Blocker, Decision, DelegationStatusEntry, PublishedFact, SharedTaskState,
    TeamDefinition, TeamDefinitionCreate, TeamEvent, TeamInstance, TeamInstanceCreate, TeamMember,
    TeamTask, TeamTaskStatus, TriageResult,
};
use async_trait::async_trait;
use chrono::Utc;
use uuid::Uuid;


#[derive(Debug, sqlx::FromRow)]
struct TeamTaskRow {
    id: Uuid,
    team_instance_id: Uuid,
    goal: String,
    instructions: Option<String>,
    status: String,
    triage_result: Option<serde_json::Value>,
    mode_selected: Option<String>,
    input_artifacts: serde_json::Value,
    parent_task_id: Option<Uuid>,
    idempotency_key: Option<String>,
    created_at: chrono::DateTime<Utc>,
    completed_at: Option<chrono::DateTime<Utc>>,
    retry_count: Option<i32>,
}

impl From<TeamTaskRow> for TeamTask {
    fn from(row: TeamTaskRow) -> Self {
        let status = TeamTaskStatus::try_from(row.status.as_str()).unwrap_or(TeamTaskStatus::Open);
        let triage_result = row
            .triage_result
            .and_then(|tr| serde_json::from_value(tr).ok());
        let input_artifacts: Vec<Uuid> =
            serde_json::from_value(row.input_artifacts).unwrap_or_default();
        TeamTask {
            id: row.id,
            team_instance_id: row.team_instance_id,
            goal: row.goal,
            instructions: row.instructions,
            status,
            triage_result,
            mode_selected: row.mode_selected,
            input_artifacts,
            parent_task_id: row.parent_task_id,
            idempotency_key: row.idempotency_key,
            created_at: row.created_at,
            completed_at: row.completed_at,
            retry_count: row.retry_count.unwrap_or(0) as u32,
        }
    }
}

#[async_trait]
pub trait TeamTaskRepository: Send + Sync {
    async fn create(
        &self,
        team_instance_id: Uuid,
        goal: &str,
        instructions: Option<&str>,
        input_artifacts: &[Uuid],
        parent_task_id: Option<Uuid>,
        idempotency_key: Option<&str>,
    ) -> anyhow::Result<TeamTask>;
    async fn get(&self, id: Uuid) -> anyhow::Result<Option<TeamTask>>;
    async fn get_by_idempotency_key(
        &self,
        team_instance_id: Uuid,
        idempotency_key: &str,
    ) -> anyhow::Result<Option<TeamTask>>;
    async fn list_by_team(
        &self,
        team_instance_id: Uuid,
        limit: i64,
    ) -> anyhow::Result<Vec<TeamTask>>;
    async fn list_open(&self, team_instance_id: Uuid, limit: i64) -> anyhow::Result<Vec<TeamTask>>;
    async fn update_status(&self, id: Uuid, status: TeamTaskStatus) -> anyhow::Result<bool>;
    async fn update_triage_result(&self, id: Uuid, triage: &TriageResult) -> anyhow::Result<bool>;
    async fn update_mode(&self, id: Uuid, mode: &str) -> anyhow::Result<bool>;
    async fn mark_completed(&self, id: Uuid) -> anyhow::Result<bool>;
    async fn update_retry_count(&self, id: Uuid, retry_count: u32) -> anyhow::Result<bool>;
}

pub struct PostgresTeamTaskRepository {
    db: Database,
}

impl PostgresTeamTaskRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

#[async_trait]
impl TeamTaskRepository for PostgresTeamTaskRepository {
    async fn create(
        &self,
        team_instance_id: Uuid,
        goal: &str,
        instructions: Option<&str>,
        input_artifacts: &[Uuid],
        parent_task_id: Option<Uuid>,
        idempotency_key: Option<&str>,
    ) -> anyhow::Result<TeamTask> {
        if let Some(key) = idempotency_key {
            if let Some(existing) = self.get_by_idempotency_key(team_instance_id, key).await? {
                return Ok(existing);
            }
        }

        let input_artifacts_json = serde_json::to_value(input_artifacts)?;
        let row = sqlx::query_as::<_, TeamTaskRow>(
            "INSERT INTO v1_team_tasks (team_instance_id, goal, instructions, input_artifacts, parent_task_id, idempotency_key) VALUES ($1, $2, $3, $4, $5, $6) RETURNING *"
        )
        .bind(team_instance_id)
        .bind(goal)
        .bind(instructions)
        .bind(input_artifacts_json)
        .bind(parent_task_id)
        .bind(idempotency_key)
        .fetch_one(self.db.pool())
        .await?;
        Ok(row.into())
    }

    async fn get_by_idempotency_key(
        &self,
        team_instance_id: Uuid,
        idempotency_key: &str,
    ) -> anyhow::Result<Option<TeamTask>> {
        let row = sqlx::query_as::<_, TeamTaskRow>(
            "SELECT * FROM v1_team_tasks WHERE team_instance_id = $1 AND idempotency_key = $2",
        )
        .bind(team_instance_id)
        .bind(idempotency_key)
        .fetch_optional(self.db.pool())
        .await?;
        Ok(row.map(|r| r.into()))
    }

    async fn get(&self, id: Uuid) -> anyhow::Result<Option<TeamTask>> {
        let row = sqlx::query_as::<_, TeamTaskRow>("SELECT * FROM v1_team_tasks WHERE id = $1")
            .bind(id)
            .fetch_optional(self.db.pool())
            .await?;
        Ok(row.map(|r| r.into()))
    }

    async fn list_by_team(
        &self,
        team_instance_id: Uuid,
        limit: i64,
    ) -> anyhow::Result<Vec<TeamTask>> {
        let rows = sqlx::query_as::<_, TeamTaskRow>(
            "SELECT * FROM v1_team_tasks WHERE team_instance_id = $1 ORDER BY created_at DESC LIMIT $2"
        )
        .bind(team_instance_id)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    async fn list_open(&self, team_instance_id: Uuid, limit: i64) -> anyhow::Result<Vec<TeamTask>> {
        let rows = sqlx::query_as::<_, TeamTaskRow>(
            "SELECT * FROM v1_team_tasks WHERE team_instance_id = $1 AND status IN ('OPEN', 'TRIAGED', 'IN_PROGRESS') ORDER BY created_at ASC LIMIT $2"
        )
        .bind(team_instance_id)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    async fn update_status(&self, id: Uuid, status: TeamTaskStatus) -> anyhow::Result<bool> {
        let result = sqlx::query("UPDATE v1_team_tasks SET status = $1 WHERE id = $2")
            .bind(status.to_string())
            .bind(id)
            .execute(self.db.pool())
            .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn update_triage_result(&self, id: Uuid, triage: &TriageResult) -> anyhow::Result<bool> {
        let triage_json = serde_json::to_value(triage)?;
        let result = sqlx::query("UPDATE v1_team_tasks SET triage_result = $1 WHERE id = $2")
            .bind(triage_json)
            .bind(id)
            .execute(self.db.pool())
            .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn update_mode(&self, id: Uuid, mode: &str) -> anyhow::Result<bool> {
        let result = sqlx::query("UPDATE v1_team_tasks SET mode_selected = $1 WHERE id = $2")
            .bind(mode)
            .bind(id)
            .execute(self.db.pool())
            .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn mark_completed(&self, id: Uuid) -> anyhow::Result<bool> {
        let result = sqlx::query("UPDATE v1_team_tasks SET completed_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(self.db.pool())
            .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn update_retry_count(&self, id: Uuid, retry_count: u32) -> anyhow::Result<bool> {
        let result = sqlx::query("UPDATE v1_team_tasks SET retry_count = $1 WHERE id = $2")
            .bind(retry_count as i32)
            .bind(id)
            .execute(self.db.pool())
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
