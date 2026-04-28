use crate::models::v1::team::{
    ArtifactRef, PublishScope, TeamDefinition, TeamDefinitionCreate, TeamInstance,
    TeamInstanceCreate, TeamMember, TeamTask, TeamTaskCreate,
};
use crate::repository::{
    SharedTaskStateRepository, TeamDefinitionRepository, TeamEventRepository,
    TeamInstanceRepository, TeamMemberRepository, TeamTaskRepository,
};
use chrono::Utc;
use std::sync::Arc;
use uuid::Uuid;

pub struct TeamService {
    definition_repo: Arc<dyn TeamDefinitionRepository>,
    instance_repo: Arc<dyn TeamInstanceRepository>,
    member_repo: Arc<dyn TeamMemberRepository>,
    team_task_repo: Arc<dyn TeamTaskRepository>,
    shared_state_repo: Arc<dyn SharedTaskStateRepository>,
    team_event_repo: Arc<dyn TeamEventRepository>,
}

impl TeamService {
    pub fn new(
        definition_repo: Arc<dyn TeamDefinitionRepository>,
        instance_repo: Arc<dyn TeamInstanceRepository>,
        member_repo: Arc<dyn TeamMemberRepository>,
        team_task_repo: Arc<dyn TeamTaskRepository>,
        shared_state_repo: Arc<dyn SharedTaskStateRepository>,
        team_event_repo: Arc<dyn TeamEventRepository>,
    ) -> Self {
        Self {
            definition_repo,
            instance_repo,
            member_repo,
            team_task_repo,
            shared_state_repo,
            team_event_repo,
        }
    }

    pub async fn create_definition(
        &self,
        req: TeamDefinitionCreate,
    ) -> anyhow::Result<TeamDefinition> {
        self.definition_repo.create(&req).await
    }

    pub async fn list_definitions(&self, limit: i64) -> anyhow::Result<Vec<TeamDefinition>> {
        self.definition_repo.list(limit).await
    }

    pub async fn get_definition(&self, id: Uuid) -> anyhow::Result<Option<TeamDefinition>> {
        self.definition_repo.get(id).await
    }

    pub async fn delete_definition(&self, id: Uuid) -> anyhow::Result<bool> {
        self.definition_repo.delete(id).await
    }

    pub async fn create_instance(&self, req: TeamInstanceCreate) -> anyhow::Result<TeamInstance> {
        self.instance_repo.create(&req).await
    }

    pub async fn list_instances(&self, limit: i64) -> anyhow::Result<Vec<TeamInstance>> {
        self.instance_repo.list(limit).await
    }

    pub async fn get_instance(&self, id: Uuid) -> anyhow::Result<Option<TeamInstance>> {
        self.instance_repo.get(id).await
    }

    pub async fn delete_instance(&self, id: Uuid) -> anyhow::Result<bool> {
        self.instance_repo.delete(id).await
    }

    pub async fn add_member(
        &self,
        team_instance_id: Uuid,
        agent_instance_id: Uuid,
        role: &str,
    ) -> anyhow::Result<TeamMember> {
        self.member_repo
            .create(team_instance_id, agent_instance_id, role)
            .await
    }

    pub async fn list_members(
        &self,
        team_instance_id: Uuid,
        limit: i64,
    ) -> anyhow::Result<Vec<TeamMember>> {
        self.member_repo.list_by_team(team_instance_id, limit).await
    }

    pub async fn remove_member(
        &self,
        team_instance_id: Uuid,
        agent_instance_id: Uuid,
    ) -> anyhow::Result<bool> {
        self.member_repo
            .remove(team_instance_id, agent_instance_id)
            .await
    }

    pub async fn create_team_task(
        &self,
        team_instance_id: Uuid,
        req: &TeamTaskCreate,
    ) -> anyhow::Result<TeamTask> {
        let _instance = self
            .instance_repo
            .get(team_instance_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Team instance not found: {}", team_instance_id))?;

        let task = self
            .team_task_repo
            .create(
                team_instance_id,
                &req.goal,
                req.instructions.as_deref(),
                &req.input_artifacts,
                req.parent_task_id,
                req.idempotency_key.as_deref(),
            )
            .await?;
        Ok(task)
    }

    pub async fn list_team_tasks(
        &self,
        team_instance_id: Uuid,
        limit: i64,
    ) -> anyhow::Result<Vec<TeamTask>> {
        self.team_task_repo
            .list_by_team(team_instance_id, limit)
            .await
    }

    pub async fn get_team_task(&self, id: Uuid) -> anyhow::Result<Option<TeamTask>> {
        self.team_task_repo.get(id).await
    }

    pub async fn get_shared_state(
        &self,
        team_instance_id: Uuid,
    ) -> anyhow::Result<Option<crate::models::v1::team::SharedTaskState>> {
        self.shared_state_repo.get(team_instance_id).await
    }

    pub async fn get_or_create_shared_state(
        &self,
        team_instance_id: Uuid,
    ) -> anyhow::Result<crate::models::v1::team::SharedTaskState> {
        self.shared_state_repo.get_or_create(team_instance_id).await
    }

    pub async fn get_team_events(
        &self,
        team_instance_id: Uuid,
        limit: i64,
    ) -> anyhow::Result<Vec<crate::models::v1::team::TeamEvent>> {
        self.team_event_repo
            .list_by_team(team_instance_id, limit)
            .await
    }

    pub async fn publish_artifact(
        &self,
        team_instance_id: Uuid,
        artifact_id: Uuid,
        scope: PublishScope,
        published_by: &str,
    ) -> anyhow::Result<bool> {
        let artifact_ref = ArtifactRef {
            artifact_id,
            scope,
            published_by: published_by.to_string(),
            published_at: Utc::now(),
        };
        self.shared_state_repo
            .add_accepted_artifact(team_instance_id, artifact_ref)
            .await
    }
}
