use crate::models::v1::team::{
    TeamDefinition, TeamDefinitionCreate, TeamInstance, TeamInstanceCreate, TeamMember,
};
use crate::repository::{TeamDefinitionRepository, TeamInstanceRepository, TeamMemberRepository};
use std::sync::Arc;
use uuid::Uuid;

pub struct TeamService {
    definition_repo: Arc<dyn TeamDefinitionRepository>,
    instance_repo: Arc<dyn TeamInstanceRepository>,
    member_repo: Arc<dyn TeamMemberRepository>,
}

impl TeamService {
    pub fn new(
        definition_repo: Arc<dyn TeamDefinitionRepository>,
        instance_repo: Arc<dyn TeamInstanceRepository>,
        member_repo: Arc<dyn TeamMemberRepository>,
    ) -> Self {
        Self { definition_repo, instance_repo, member_repo }
    }

    pub async fn create_definition(
        &self,
        req: TeamDefinitionCreate,
    ) -> anyhow::Result<TeamDefinition> {
        self.definition_repo.create(&req).await
    }

    pub async fn list_definitions(
        &self, limit: i64) -> anyhow::Result<Vec<TeamDefinition>> {
        self.definition_repo.list(limit).await
    }

    pub async fn get_definition(
        &self, id: Uuid) -> anyhow::Result<Option<TeamDefinition>> {
        self.definition_repo.get(id).await
    }

    pub async fn delete_definition(&self, id: Uuid) -> anyhow::Result<bool> {
        self.definition_repo.delete(id).await
    }

    pub async fn create_instance(
        &self,
        req: TeamInstanceCreate,
    ) -> anyhow::Result<TeamInstance> {
        self.instance_repo.create(&req).await
    }

    pub async fn list_instances(
        &self, limit: i64) -> anyhow::Result<Vec<TeamInstance>> {
        self.instance_repo.list(limit).await
    }

    pub async fn get_instance(
        &self, id: Uuid) -> anyhow::Result<Option<TeamInstance>> {
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
        self.member_repo.create(team_instance_id, agent_instance_id, role).await
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
        self.member_repo.remove(team_instance_id, agent_instance_id).await
    }
}
