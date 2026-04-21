use crate::models::v1::artifact::{Artifact, ArtifactScope};
use crate::repository::ArtifactRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct ArtifactService {
    repo: Arc<dyn ArtifactRepository>,
}

impl ArtifactService {
    pub fn new(repo: Arc<dyn ArtifactRepository>) -> Self {
        Self { repo }
    }

    pub async fn create(
        &self,
        kind: &str,
        scope: ArtifactScope,
        mime_type: &str,
        content: serde_json::Value,
    ) -> anyhow::Result<Artifact> {
        self.repo.create(kind, scope, mime_type, content).await
    }

    pub async fn list(&self, limit: i64) -> anyhow::Result<Vec<Artifact>> {
        self.repo.list(limit).await
    }

    pub async fn list_by_instance(
        &self,
        instance_id: Uuid,
        limit: i64,
    ) -> anyhow::Result<Vec<Artifact>> {
        self.repo.list_by_instance(instance_id, limit).await
    }

    pub async fn get(&self, id: Uuid) -> anyhow::Result<Option<Artifact>> {
        self.repo.get(id).await
    }

    pub async fn delete(&self, id: Uuid) -> anyhow::Result<bool> {
        self.repo.delete(id).await
    }

    pub async fn update_scope(&self, id: Uuid, scope: ArtifactScope) -> anyhow::Result<bool> {
        self.repo.update_scope(id, scope).await
    }
}
