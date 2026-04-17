use crate::models::v1::checkpoint::Checkpoint;
use crate::repository::checkpoint_ext::CheckpointRepositoryExt;
use std::sync::Arc;
use uuid::Uuid;

pub struct CheckpointService {
    repo: Arc<dyn CheckpointRepositoryExt>,
}

impl CheckpointService {
    pub fn new(repo: Arc<dyn CheckpointRepositoryExt>) -> Self {
        Self { repo }
    }

    pub async fn list_by_instance(
        &self,
        instance_id: Uuid,
        limit: i64,
    ) -> anyhow::Result<Vec<Checkpoint>> {
        self.repo.list_by_instance(instance_id, limit).await
    }

    pub async fn get(&self, id: Uuid) -> anyhow::Result<Option<Checkpoint>> {
        self.repo.get(id).await
    }
}
