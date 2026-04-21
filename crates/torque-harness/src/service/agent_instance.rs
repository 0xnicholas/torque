use crate::models::v1::agent_instance::{AgentInstance, AgentInstanceCreate, AgentInstanceStatus};
use crate::repository::AgentInstanceRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct AgentInstanceService {
    repo: Arc<dyn AgentInstanceRepository>,
}

impl AgentInstanceService {
    pub fn new(repo: Arc<dyn AgentInstanceRepository>) -> Self {
        Self { repo }
    }
    pub async fn create(&self, req: AgentInstanceCreate) -> anyhow::Result<AgentInstance> {
        self.repo.create(&req).await
    }
    pub async fn list(&self, limit: i64) -> anyhow::Result<Vec<AgentInstance>> {
        self.repo.list(limit).await
    }
    pub async fn get(&self, id: Uuid) -> anyhow::Result<Option<AgentInstance>> {
        self.repo.get(id).await
    }
    pub async fn delete(&self, id: Uuid) -> anyhow::Result<bool> {
        self.repo.delete(id).await
    }
    pub async fn update_status(
        &self,
        id: Uuid,
        status: AgentInstanceStatus,
    ) -> anyhow::Result<bool> {
        self.repo.update_status(id, status).await
    }
}
