use crate::models::v1::agent_definition::{AgentDefinition, AgentDefinitionCreate};
use crate::repository::AgentDefinitionRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct AgentDefinitionService {
    repo: Arc<dyn AgentDefinitionRepository>,
}

impl AgentDefinitionService {
    pub fn new(repo: Arc<dyn AgentDefinitionRepository>) -> Self {
        Self { repo }
    }

    pub async fn create(&self, req: AgentDefinitionCreate) -> anyhow::Result<AgentDefinition> {
        self.repo.create(&req).await
    }

    pub async fn list(
        &self,
        limit: i64,
        cursor: Option<Uuid>,
        sort: Option<&str>,
    ) -> anyhow::Result<Vec<AgentDefinition>> {
        self.repo.list(limit, cursor, sort).await
    }

    pub async fn get(&self, id: Uuid) -> anyhow::Result<Option<AgentDefinition>> {
        self.repo.get(id).await
    }

    pub async fn delete(&self, id: Uuid) -> anyhow::Result<bool> {
        self.repo.delete(id).await
    }
}
