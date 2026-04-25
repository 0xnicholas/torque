use async_trait::async_trait;
use chrono::Utc;
use std::sync::Arc;
use torque_harness::models::v1::artifact::{Artifact, ArtifactScope};
use torque_harness::repository::ArtifactRepository;
use torque_harness::service::{
    tool_offload::TOOL_OUTPUT_ARTIFACT_KIND, ArtifactService, RoutedVfs, ToolOffloadConfig,
    ToolOffloadService,
};
use torque_harness::service::vfs::{ScratchBackend, WorkspaceBackend};
use torque_harness::tools::ToolResult;
use uuid::Uuid;

#[derive(Default)]
struct InMemoryArtifactRepository {
    artifacts: std::sync::Mutex<Vec<Artifact>>,
}

fn copy_scope(scope: &ArtifactScope) -> ArtifactScope {
    match scope {
        ArtifactScope::Private => ArtifactScope::Private,
        ArtifactScope::TeamShared => ArtifactScope::TeamShared,
        ArtifactScope::ExternalPublished => ArtifactScope::ExternalPublished,
    }
}

fn copy_artifact(artifact: &Artifact) -> Artifact {
    Artifact {
        id: artifact.id,
        kind: artifact.kind.clone(),
        scope: copy_scope(&artifact.scope),
        source_instance_id: artifact.source_instance_id,
        published_to_team_instance_id: artifact.published_to_team_instance_id,
        mime_type: artifact.mime_type.clone(),
        size_bytes: artifact.size_bytes,
        summary: artifact.summary.clone(),
        content: artifact.content.clone(),
        created_at: artifact.created_at,
    }
}

#[async_trait]
impl ArtifactRepository for InMemoryArtifactRepository {
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
        let artifact = Artifact {
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
        };
        self.artifacts.lock().unwrap().push(copy_artifact(&artifact));
        Ok(artifact)
    }

    async fn list(&self, _limit: i64) -> anyhow::Result<Vec<Artifact>> {
        Ok(self
            .artifacts
            .lock()
            .unwrap()
            .iter()
            .map(copy_artifact)
            .collect())
    }

    async fn list_by_instance(
        &self,
        instance_id: Uuid,
        _limit: i64,
    ) -> anyhow::Result<Vec<Artifact>> {
        Ok(self
            .artifacts
            .lock()
            .unwrap()
            .iter()
            .filter(|artifact| artifact.source_instance_id == Some(instance_id))
            .map(copy_artifact)
            .collect())
    }

    async fn get(&self, id: Uuid) -> anyhow::Result<Option<Artifact>> {
        Ok(self
            .artifacts
            .lock()
            .unwrap()
            .iter()
            .find(|artifact| artifact.id == id)
            .map(copy_artifact))
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

fn test_vfs() -> Arc<RoutedVfs> {
    Arc::new(RoutedVfs::new(
        Arc::new(ScratchBackend::default()),
        Arc::new(WorkspaceBackend::new(std::env::current_dir().unwrap())),
    ))
}

#[tokio::test]
async fn tool_offload_tests_small_result_stays_inline() {
    let repo = Arc::new(InMemoryArtifactRepository::default());
    let service = ToolOffloadService::new(
        Some(Arc::new(ArtifactService::new(repo))),
        Some(test_vfs()),
    )
    .with_config(ToolOffloadConfig {
        inline_max_bytes: 16,
        scratch_max_bytes: 32,
    });

    let result = service
        .offload(
            "demo",
            ToolResult {
                success: true,
                content: "short".to_string(),
                error: None,
            },
            None,
        )
        .await
        .expect("offload should succeed");

    assert_eq!(result.content, "short");
}

#[tokio::test]
async fn tool_offload_tests_medium_result_offloads_to_scratch() {
    let repo = Arc::new(InMemoryArtifactRepository::default());
    let vfs = test_vfs();
    let service = ToolOffloadService::new(
        Some(Arc::new(ArtifactService::new(repo))),
        Some(vfs.clone()),
    )
    .with_config(ToolOffloadConfig {
        inline_max_bytes: 8,
        scratch_max_bytes: 64,
    });

    let raw = "x".repeat(32);
    let result = service
        .offload(
            "demo",
            ToolResult {
                success: true,
                content: raw.clone(),
                error: None,
            },
            None,
        )
        .await
        .expect("offload should succeed");

    assert!(result.content.contains("/scratch/tool-results/"));
    let path = result
        .content
        .lines()
        .next()
        .unwrap()
        .split(": ")
        .nth(1)
        .unwrap()
        .split(" (")
        .next()
        .unwrap()
        .to_string();
    let stored = vfs.read(&path).await.expect("scratch file should exist");
    assert_eq!(stored, raw);
}

#[tokio::test]
async fn tool_offload_tests_large_result_offloads_to_artifact() {
    let repo = Arc::new(InMemoryArtifactRepository::default());
    let repo_for_assert = repo.clone();
    let service = ToolOffloadService::new(
        Some(Arc::new(ArtifactService::new(repo))),
        Some(test_vfs()),
    )
    .with_config(ToolOffloadConfig {
        inline_max_bytes: 8,
        scratch_max_bytes: 16,
    });

    let instance_id = Uuid::new_v4();
    let result = service
        .offload(
            "demo",
            ToolResult {
                success: true,
                content: "y".repeat(64),
                error: None,
            },
            Some(instance_id),
        )
        .await
        .expect("offload should succeed");

    assert!(result.content.contains("artifact:"));
    let stored = repo_for_assert
        .list_by_instance(instance_id, 10)
        .await
        .expect("artifact list should succeed");
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].kind, TOOL_OUTPUT_ARTIFACT_KIND);
    assert_eq!(stored[0].content["tool_name"], "demo");
}
