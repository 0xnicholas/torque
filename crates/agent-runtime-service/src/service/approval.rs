use crate::models::v1::approval::Approval;
use crate::repository::ApprovalRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct ApprovalService {
    repo: Arc<dyn ApprovalRepository>,
}

impl ApprovalService {
    pub fn new(repo: Arc<dyn ApprovalRepository>) -> Self {
        Self { repo }
    }

    pub async fn list(&self, limit: i64) -> anyhow::Result<Vec<Approval>> {
        self.repo.list(limit).await
    }

    pub async fn get(&self, id: Uuid) -> anyhow::Result<Option<Approval>> {
        self.repo.get(id).await
    }

    pub async fn resolve(&self, id: Uuid, resolution: &str) -> anyhow::Result<bool> {
        self.repo.resolve(id, resolution).await
    }
}
