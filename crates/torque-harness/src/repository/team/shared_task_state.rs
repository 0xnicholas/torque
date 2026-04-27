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
struct SharedTaskStateRow {
    id: Uuid,
    team_instance_id: Uuid,
    accepted_artifact_refs: serde_json::Value,
    published_facts: serde_json::Value,
    delegation_status: serde_json::Value,
    open_blockers: serde_json::Value,
    decisions: serde_json::Value,
    updated_at: chrono::DateTime<Utc>,
}

impl From<SharedTaskStateRow> for SharedTaskState {
    fn from(row: SharedTaskStateRow) -> Self {
        SharedTaskState {
            id: row.id,
            team_instance_id: row.team_instance_id,
            accepted_artifact_refs: serde_json::from_value(row.accepted_artifact_refs)
                .unwrap_or_default(),
            published_facts: serde_json::from_value(row.published_facts).unwrap_or_default(),
            delegation_status: serde_json::from_value(row.delegation_status).unwrap_or_default(),
            open_blockers: serde_json::from_value(row.open_blockers).unwrap_or_default(),
            decisions: serde_json::from_value(row.decisions).unwrap_or_default(),
            updated_at: row.updated_at,
        }
    }
}

#[async_trait]
pub trait SharedTaskStateRepository: Send + Sync {
    async fn get_or_create(&self, team_instance_id: Uuid) -> anyhow::Result<SharedTaskState>;
    async fn get(&self, team_instance_id: Uuid) -> anyhow::Result<Option<SharedTaskState>>;
    async fn add_accepted_artifact(
        &self,
        team_instance_id: Uuid,
        artifact_ref: ArtifactRef,
    ) -> anyhow::Result<bool>;
    async fn add_published_fact(
        &self,
        team_instance_id: Uuid,
        fact: PublishedFact,
    ) -> anyhow::Result<bool>;
    async fn update_delegation_status(
        &self,
        team_instance_id: Uuid,
        entry: DelegationStatusEntry,
    ) -> anyhow::Result<bool>;
    async fn add_blocker(&self, team_instance_id: Uuid, blocker: Blocker) -> anyhow::Result<bool>;
    async fn resolve_blocker(
        &self,
        team_instance_id: Uuid,
        blocker_id: Uuid,
    ) -> anyhow::Result<bool>;
    async fn add_decision(
        &self,
        team_instance_id: Uuid,
        decision: Decision,
    ) -> anyhow::Result<bool>;
    async fn update_with_lock(
        &self,
        team_instance_id: Uuid,
        updates: Vec<SharedTaskStateUpdate>,
    ) -> anyhow::Result<SharedTaskState>;
}

pub struct PostgresSharedTaskStateRepository {
    db: Database,
}

impl PostgresSharedTaskStateRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

#[async_trait]
impl SharedTaskStateRepository for PostgresSharedTaskStateRepository {
    async fn get_or_create(&self, team_instance_id: Uuid) -> anyhow::Result<SharedTaskState> {
        let row = sqlx::query_as::<_, SharedTaskStateRow>(
            "INSERT INTO v1_team_shared_state (team_instance_id) VALUES ($1) ON CONFLICT (team_instance_id) DO UPDATE SET updated_at = NOW() RETURNING *"
        )
        .bind(team_instance_id)
        .fetch_one(self.db.pool())
        .await?;
        Ok(row.into())
    }

    async fn get(&self, team_instance_id: Uuid) -> anyhow::Result<Option<SharedTaskState>> {
        let row = sqlx::query_as::<_, SharedTaskStateRow>(
            "SELECT * FROM v1_team_shared_state WHERE team_instance_id = $1",
        )
        .bind(team_instance_id)
        .fetch_optional(self.db.pool())
        .await?;
        Ok(row.map(|r| r.into()))
    }

    async fn add_accepted_artifact(
        &self,
        team_instance_id: Uuid,
        artifact_ref: ArtifactRef,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "UPDATE v1_team_shared_state SET accepted_artifact_refs = accepted_artifact_refs || $1::jsonb, updated_at = NOW() WHERE team_instance_id = $2"
        )
        .bind(serde_json::to_value(vec![artifact_ref])?)
        .bind(team_instance_id)
        .execute(self.db.pool())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn add_published_fact(
        &self,
        team_instance_id: Uuid,
        fact: PublishedFact,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "UPDATE v1_team_shared_state SET published_facts = published_facts || $1::jsonb, updated_at = NOW() WHERE team_instance_id = $2"
        )
        .bind(serde_json::to_value(vec![fact])?)
        .bind(team_instance_id)
        .execute(self.db.pool())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn update_delegation_status(
        &self,
        team_instance_id: Uuid,
        entry: DelegationStatusEntry,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "UPDATE v1_team_shared_state SET delegation_status = delegation_status || $1::jsonb, updated_at = NOW() WHERE team_instance_id = $2"
        )
        .bind(serde_json::to_value(vec![entry])?)
        .bind(team_instance_id)
        .execute(self.db.pool())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn add_blocker(&self, team_instance_id: Uuid, blocker: Blocker) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "UPDATE v1_team_shared_state SET open_blockers = open_blockers || $1::jsonb, updated_at = NOW() WHERE team_instance_id = $2"
        )
        .bind(serde_json::to_value(vec![blocker])?)
        .bind(team_instance_id)
        .execute(self.db.pool())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn resolve_blocker(
        &self,
        team_instance_id: Uuid,
        blocker_id: Uuid,
    ) -> anyhow::Result<bool> {
        let mut tx = self.db.pool().begin().await?;

        let row = sqlx::query_as::<_, SharedTaskStateRow>(
            "SELECT * FROM v1_team_shared_state WHERE team_instance_id = $1 FOR UPDATE",
        )
        .bind(team_instance_id)
        .fetch_one(&mut *tx)
        .await?;

        let state: SharedTaskState = row.into();
        let remaining: Vec<Blocker> = state
            .open_blockers
            .into_iter()
            .filter(|b| b.blocker_id != blocker_id)
            .collect();

        let result = sqlx::query(
            "UPDATE v1_team_shared_state SET open_blockers = $1::jsonb, updated_at = NOW() WHERE team_instance_id = $2"
        )
        .bind(serde_json::to_value(remaining)?)
        .bind(team_instance_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(result.rows_affected() > 0)
    }

    async fn add_decision(
        &self,
        team_instance_id: Uuid,
        decision: Decision,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "UPDATE v1_team_shared_state SET decisions = decisions || $1::jsonb, updated_at = NOW() WHERE team_instance_id = $2"
        )
        .bind(serde_json::to_value(vec![decision])?)
        .bind(team_instance_id)
        .execute(self.db.pool())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn update_with_lock(
        &self,
        team_instance_id: Uuid,
        updates: Vec<SharedTaskStateUpdate>,
    ) -> anyhow::Result<SharedTaskState> {
        let mut tx = self.db.pool().begin().await?;

        let state = sqlx::query_as::<_, SharedTaskStateRow>(
            "SELECT * FROM v1_team_shared_state WHERE team_instance_id = $1 FOR UPDATE",
        )
        .bind(team_instance_id)
        .fetch_one(&mut *tx)
        .await?;

        let mut shared_state: SharedTaskState = state.into();
        for update in updates {
            update.apply(&mut shared_state);
        }

        let accepted = serde_json::to_value(&shared_state.accepted_artifact_refs)?;
        let published = serde_json::to_value(&shared_state.published_facts)?;
        let delegation = serde_json::to_value(&shared_state.delegation_status)?;
        let blockers = serde_json::to_value(&shared_state.open_blockers)?;
        let decisions = serde_json::to_value(&shared_state.decisions)?;

        sqlx::query(
            "UPDATE v1_team_shared_state SET accepted_artifact_refs = $1, published_facts = $2, delegation_status = $3, open_blockers = $4, decisions = $5, updated_at = NOW() WHERE team_instance_id = $6"
        )
        .bind(accepted)
        .bind(published)
        .bind(delegation)
        .bind(blockers)
        .bind(decisions)
        .bind(team_instance_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(shared_state)
    }
}

pub enum SharedTaskStateUpdate {
    AddArtifact(ArtifactRef),
    AddFact(PublishedFact),
    UpdateDelegationStatus(DelegationStatusEntry),
    AddBlocker(Blocker),
    ResolveBlocker(Uuid),
    AddDecision(Decision),
}

impl SharedTaskStateUpdate {
    fn apply(self, state: &mut SharedTaskState) {
        match self {
            SharedTaskStateUpdate::AddArtifact(artifact) => {
                state.accepted_artifact_refs.push(artifact);
            }
            SharedTaskStateUpdate::AddFact(fact) => {
                state.published_facts.push(fact);
            }
            SharedTaskStateUpdate::UpdateDelegationStatus(entry) => {
                state.delegation_status.push(entry);
            }
            SharedTaskStateUpdate::AddBlocker(blocker) => {
                state.open_blockers.push(blocker);
            }
            SharedTaskStateUpdate::ResolveBlocker(blocker_id) => {
                state.open_blockers.retain(|b| b.blocker_id != blocker_id);
            }
            SharedTaskStateUpdate::AddDecision(decision) => {
                state.decisions.push(decision);
            }
        }
    }
}
