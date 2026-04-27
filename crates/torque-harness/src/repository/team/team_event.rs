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
struct TeamEventRow {
    id: Uuid,
    team_instance_id: Uuid,
    event_type: String,
    timestamp: chrono::DateTime<Utc>,
    actor_ref: String,
    team_task_ref: Option<Uuid>,
    related_instance_refs: serde_json::Value,
    related_artifact_refs: serde_json::Value,
    payload: serde_json::Value,
    causal_event_refs: serde_json::Value,
}

impl From<TeamEventRow> for TeamEvent {
    fn from(row: TeamEventRow) -> Self {
        TeamEvent {
            id: row.id,
            team_instance_id: row.team_instance_id,
            event_type: row.event_type,
            timestamp: row.timestamp,
            actor_ref: row.actor_ref,
            team_task_ref: row.team_task_ref,
            related_instance_refs: serde_json::from_value(row.related_instance_refs)
                .unwrap_or_default(),
            related_artifact_refs: serde_json::from_value(row.related_artifact_refs)
                .unwrap_or_default(),
            payload: row.payload,
            causal_event_refs: serde_json::from_value(row.causal_event_refs).unwrap_or_default(),
        }
    }
}

#[async_trait]
pub trait TeamEventRepository: Send + Sync {
    async fn create(&self, event: &TeamEvent) -> anyhow::Result<TeamEvent>;
    async fn list_by_team(
        &self,
        team_instance_id: Uuid,
        limit: i64,
    ) -> anyhow::Result<Vec<TeamEvent>>;
    async fn list_by_task(&self, team_task_id: Uuid, limit: i64) -> anyhow::Result<Vec<TeamEvent>>;
}

pub struct PostgresTeamEventRepository {
    db: Database,
}

impl PostgresTeamEventRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

#[async_trait]
impl TeamEventRepository for PostgresTeamEventRepository {
    async fn create(&self, event: &TeamEvent) -> anyhow::Result<TeamEvent> {
        let row = sqlx::query_as::<_, TeamEventRow>(
            "INSERT INTO v1_team_events (team_instance_id, event_type, actor_ref, team_task_ref, related_instance_refs, related_artifact_refs, payload, causal_event_refs) VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING *"
        )
        .bind(event.team_instance_id)
        .bind(&event.event_type)
        .bind(&event.actor_ref)
        .bind(event.team_task_ref)
        .bind(serde_json::to_value(&event.related_instance_refs)?)
        .bind(serde_json::to_value(&event.related_artifact_refs)?)
        .bind(&event.payload)
        .bind(serde_json::to_value(&event.causal_event_refs)?)
        .fetch_one(self.db.pool())
        .await?;
        Ok(row.into())
    }

    async fn list_by_team(
        &self,
        team_instance_id: Uuid,
        limit: i64,
    ) -> anyhow::Result<Vec<TeamEvent>> {
        let rows = sqlx::query_as::<_, TeamEventRow>(
            "SELECT * FROM v1_team_events WHERE team_instance_id = $1 ORDER BY timestamp DESC LIMIT $2"
        )
        .bind(team_instance_id)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    async fn list_by_task(&self, team_task_id: Uuid, limit: i64) -> anyhow::Result<Vec<TeamEvent>> {
        let rows = sqlx::query_as::<_, TeamEventRow>(
            "SELECT * FROM v1_team_events WHERE team_task_ref = $1 ORDER BY timestamp DESC LIMIT $2"
        )
        .bind(team_task_id)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows.into_iter().map(|r| r.into()).collect())
    }
}

