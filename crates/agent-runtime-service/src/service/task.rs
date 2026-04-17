use crate::models::v1::task::Task;
use crate::repository::TaskRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct TaskService {
    repo: Arc<dyn TaskRepository>,
}

impl TaskService {
    pub fn new(repo: Arc<dyn TaskRepository>) -> Self {
        Self { repo }
    }

    pub async fn list(&self, limit: i64) -> anyhow::Result<Vec<Task>> {
        self.repo.list(limit).await
    }

    pub async fn get(&self, id: Uuid) -> anyhow::Result<Option<Task>> {
        self.repo.get(id).await
    }

    pub async fn cancel(&self, id: Uuid) -> anyhow::Result<bool> {
        self.repo.cancel(id).await
    }
}
