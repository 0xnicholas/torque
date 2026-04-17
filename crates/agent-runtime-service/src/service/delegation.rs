use crate::models::v1::delegation::Delegation;
use crate::repository::DelegationRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct DelegationService {
    repo: Arc<dyn DelegationRepository>,
}

impl DelegationService {
    pub fn new(repo: Arc<dyn DelegationRepository>) -> Self {
        Self { repo }
    }

    pub async fn create(
        &self,
        task_id: Uuid,
        parent_instance_id: Uuid,
        selector: serde_json::Value,
    ) -> anyhow::Result<Delegation> {
        self.repo.create(task_id, parent_instance_id, selector).await
    }

    pub async fn list(&self, limit: i64) -> anyhow::Result<Vec<Delegation>> {
        self.repo.list(limit).await
    }

    pub async fn list_by_instance(&self, instance_id: Uuid, limit: i64) -> anyhow::Result<Vec<Delegation>> {
        self.repo.list_by_instance(instance_id, limit).await
    }

    pub async fn list_by_task(&self, task_id: Uuid, limit: i64) -> anyhow::Result<Vec<Delegation>> {
        self.repo.list_by_task(task_id, limit).await
    }

    pub async fn get(&self, id: Uuid) -> anyhow::Result<Option<Delegation>> {
        self.repo.get(id).await
    }

    pub async fn accept(&self, id: Uuid) -> anyhow::Result<bool> {
        self.repo.update_status(id, "ACCEPTED").await
    }

    pub async fn reject(&self, id: Uuid) -> anyhow::Result<bool> {
        self.repo.update_status(id, "REJECTED").await
    }
}
