use async_trait::async_trait;
use chrono::Utc;
use std::sync::Arc;
use torque_harness::models::v1::artifact::{Artifact, ArtifactScope};
use torque_harness::policy::filesystem::{FilesystemPermissionRule, FsAction, RuleEffect};
use torque_harness::repository::ArtifactRepository;
use torque_harness::service::{ArtifactService, RoutedVfs};
use torque_harness::tools::vfs::create_vfs_tools_with_rules;
use torque_harness::tools::ToolRegistry;
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
    async fn list_by_instance(&self, _instance_id: Uuid, _limit: i64) -> anyhow::Result<Vec<Artifact>> {
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

async fn registry_with_rules(rules: Vec<FilesystemPermissionRule>) -> ToolRegistry {
    let registry = ToolRegistry::new();
    let artifact_service = Arc::new(ArtifactService::new(Arc::new(NoopArtifactRepository)));
    let _artifact_service = artifact_service;
    let vfs = Arc::new(RoutedVfs::for_current_workspace());
    for tool in create_vfs_tools_with_rules(vfs, rules) {
        registry.register(tool).await;
    }
    registry
}

#[tokio::test]
async fn file_approval_flow_tests_workspace_write_can_require_approval() {
    let registry = registry_with_rules(vec![
        FilesystemPermissionRule::new(RuleEffect::RequireApproval, FsAction::Write, "/workspace/**"),
    ])
    .await;

    let result = registry
        .execute(
            "write_file",
            serde_json::json!({ "path": "/workspace/.approval-tests/file.txt", "content": "data" }),
        )
        .await
        .expect("write_file should execute");

    assert!(!result.success);
    assert!(
        result
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("approval required")
    );
}

#[tokio::test]
async fn file_approval_flow_tests_scratch_write_bypasses_approval() {
    let registry = registry_with_rules(vec![
        FilesystemPermissionRule::new(RuleEffect::RequireApproval, FsAction::Write, "/workspace/**"),
        FilesystemPermissionRule::new(RuleEffect::Allow, FsAction::Write, "/scratch/**"),
    ])
    .await;

    let result = registry
        .execute(
            "write_file",
            serde_json::json!({ "path": "/scratch/approval-ok.txt", "content": "data" }),
        )
        .await
        .expect("write_file should execute");

    assert!(result.success);
}
