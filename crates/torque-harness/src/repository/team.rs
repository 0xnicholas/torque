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
}

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
