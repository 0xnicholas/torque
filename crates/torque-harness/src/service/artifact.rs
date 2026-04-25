use crate::models::v1::artifact::{Artifact, ArtifactScope};
use crate::repository::ArtifactRepository;
use std::sync::Arc;
use uuid::Uuid;

pub const TODO_DOCUMENT_KIND: &str = "todo_document";
const JSON_MIME_TYPE: &str = "application/json";

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

    pub async fn create_json_document(
        &self,
        kind: &str,
        scope: ArtifactScope,
        content: serde_json::Value,
    ) -> anyhow::Result<Artifact> {
        self.create(kind, scope, JSON_MIME_TYPE, content).await
    }

    pub async fn create_json_document_with_source_instance(
        &self,
        kind: &str,
        scope: ArtifactScope,
        content: serde_json::Value,
        source_instance_id: Option<Uuid>,
    ) -> anyhow::Result<Artifact> {
        self.repo
            .create_with_source_instance(kind, scope, JSON_MIME_TYPE, content, source_instance_id)
            .await
    }

    pub async fn find_latest_by_kind_scope_and_content_string(
        &self,
        kind: &str,
        scope: ArtifactScope,
        content_key: &str,
        content_value: &str,
    ) -> anyhow::Result<Option<Artifact>> {
        self.repo
            .find_latest_by_kind_scope_and_content_string(kind, scope, content_key, content_value)
            .await
    }

    pub async fn find_latest_by_kind_scope_and_content_string_with_source_instance(
        &self,
        kind: &str,
        scope: ArtifactScope,
        content_key: &str,
        content_value: &str,
        source_instance_id: Option<Uuid>,
    ) -> anyhow::Result<Option<Artifact>> {
        self.repo
            .find_latest_by_kind_scope_and_content_string_with_source_instance(
                kind,
                scope,
                content_key,
                content_value,
                source_instance_id,
            )
            .await
    }
}
