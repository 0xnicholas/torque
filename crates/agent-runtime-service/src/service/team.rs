use crate::models::v1::task::{Task, TaskType};
use crate::models::v1::team::{
    TeamDefinition, TeamDefinitionCreate, TeamInstance, TeamInstanceCreate, TeamMember,
};
use crate::repository::{TaskRepository, TeamDefinitionRepository, TeamInstanceRepository, TeamMemberRepository};
use std::sync::Arc;
use uuid::Uuid;

pub struct TeamService {
    definition_repo: Arc<dyn TeamDefinitionRepository>,
    instance_repo: Arc<dyn TeamInstanceRepository>,
    member_repo: Arc<dyn TeamMemberRepository>,
    task_repo: Arc<dyn TaskRepository>,
}

impl TeamService {
    pub fn new(
        definition_repo: Arc<dyn TeamDefinitionRepository>,
        instance_repo: Arc<dyn TeamInstanceRepository>,
        member_repo: Arc<dyn TeamMemberRepository>,
        task_repo: Arc<dyn TaskRepository>,
    ) -> Self {
        Self { definition_repo, instance_repo, member_repo, task_repo }
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

    pub async fn create_team_task(
        &self,
        team_instance_id: Uuid,
        goal: &str,
        instructions: Option<&str>,
    ) -> anyhow::Result<Task> {
        // Verify team instance exists
        let _instance = self.instance_repo.get(team_instance_id).await?
            .ok_or_else(|| anyhow::anyhow!("Team instance not found: {}", team_instance_id))?;

        // Create team task
        let task = self.task_repo.create(
            TaskType::TeamTask,
            goal,
            instructions,
            None, // agent_instance_id will be set when supervisor claims it
            serde_json::json!([]),
        ).await?;

        // Note: In a full implementation, we would:
        // 1. Create supervisor agent instance from team definition
        // 2. Add supervisor as team member
        // 3. Set task.agent_instance_id to supervisor
        // 4. Trigger supervisor execution
        // For MVP, we return the created task and let the caller handle execution

        Ok(task)
    }

    pub async fn list_team_tasks(
        &self,
        team_instance_id: Uuid,
        limit: i64,
    ) -> anyhow::Result<Vec<Task>> {
        self.task_repo.list_by_team(team_instance_id, limit).await
    }
}
