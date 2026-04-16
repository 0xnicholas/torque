use async_trait::async_trait;
use crate::db::Database;
use uuid::Uuid;

#[async_trait]
pub trait AgentDefinitionRepository: Send + Sync {
    async fn get_by_id(&self, id: Uuid) -> anyhow::Result<Option<()>>;
}

pub struct PostgresAgentDefinitionRepository {
    _db: Database,
}

impl PostgresAgentDefinitionRepository {
    pub fn new(_db: Database) -> Self {
        Self { _db }
    }
}

#[async_trait]
impl AgentDefinitionRepository for PostgresAgentDefinitionRepository {
    async fn get_by_id(&self, _id: Uuid) -> anyhow::Result<Option<()>> {
        todo!("implement when v1 agent definitions table exists")
    }
}
