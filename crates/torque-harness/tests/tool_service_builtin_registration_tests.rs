use async_trait::async_trait;
use chrono::Utc;
use std::sync::Arc;
use torque_harness::models::v1::artifact::{Artifact, ArtifactScope};
use torque_harness::repository::ArtifactRepository;
use torque_harness::service::{ArtifactService, ToolService};
use uuid::Uuid;

struct NoopArtifactRepository;

#[async_trait]
impl ArtifactRepository for NoopArtifactRepository {
    async fn create(
        &self,
        kind: &str,
        scope: ArtifactScope,
        mime_type: &str,
        content: serde_json::Value,
    ) -> anyhow::Result<Artifact> {
        self.create_with_source_instance(kind, scope, mime_type, content, None)
            .await
    }

    async fn create_with_source_instance(
        &self,
        kind: &str,
        scope: ArtifactScope,
        mime_type: &str,
        content: serde_json::Value,
        source_instance_id: Option<Uuid>,
    ) -> anyhow::Result<Artifact> {
        Ok(Artifact {
            id: Uuid::new_v4(),
            kind: kind.to_string(),
            scope,
            source_instance_id,
            published_to_team_instance_id: None,
            mime_type: mime_type.to_string(),
            size_bytes: serde_json::to_string(&content)?.len() as i64,
            summary: None,
            content,
            created_at: Utc::now(),
        })
    }

    async fn list(&self, _limit: i64) -> anyhow::Result<Vec<Artifact>> {
        Ok(vec![])
    }

    async fn list_by_instance(
        &self,
        _instance_id: Uuid,
        _limit: i64,
    ) -> anyhow::Result<Vec<Artifact>> {
        Ok(vec![])
    }

    async fn get(&self, _id: Uuid) -> anyhow::Result<Option<Artifact>> {
        Ok(None)
    }

    async fn delete(&self, _id: Uuid) -> anyhow::Result<bool> {
        Ok(false)
    }

    async fn update_scope(&self, _id: Uuid, _scope: ArtifactScope) -> anyhow::Result<bool> {
        Ok(false)
    }

    async fn find_latest_by_kind_scope_and_content_string(
        &self,
        _kind: &str,
        _scope: ArtifactScope,
        _content_key: &str,
        _content_value: &str,
    ) -> anyhow::Result<Option<Artifact>> {
        Ok(None)
    }

    async fn find_latest_by_kind_scope_and_content_string_with_source_instance(
        &self,
        _kind: &str,
        _scope: ArtifactScope,
        _content_key: &str,
        _content_value: &str,
        _source_instance_id: Option<Uuid>,
    ) -> anyhow::Result<Option<Artifact>> {
        Ok(None)
    }
}

#[tokio::test]
async fn tool_service_bootstraps_todo_tools_without_manual_registration() {
    let artifact_service = Arc::new(ArtifactService::new(Arc::new(NoopArtifactRepository)));
    let tool_service = ToolService::new_with_builtins(artifact_service);

    let names = tool_service.registry().list_tool_names().await;

    assert!(
        names.contains(&"write_todos".to_string()),
        "write_todos should be preloaded"
    );
    assert!(
        names.contains(&"read_todos".to_string()),
        "read_todos should be preloaded"
    );
    assert!(
        names.contains(&"update_todo".to_string()),
        "update_todo should be preloaded"
    );
}
