use async_trait::async_trait;
use chrono::Utc;
use serde_json::json;
use std::sync::{Arc, Mutex};
use torque_harness::models::v1::artifact::{Artifact, ArtifactScope};
use torque_harness::repository::ArtifactRepository;
use torque_harness::service::ArtifactService;
use torque_harness::service::artifact::TODO_DOCUMENT_KIND;
use torque_harness::tools::registry::register_builtin_tools;
use torque_harness::tools::{ToolRegistry, ToolResult};
use uuid::Uuid;

#[derive(Default)]
struct InMemoryArtifactRepository {
    artifacts: Mutex<Vec<Artifact>>,
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
        let artifact = Artifact {
            id: Uuid::new_v4(),
            kind: kind.to_string(),
            scope,
            source_instance_id: None,
            published_to_team_instance_id: None,
            mime_type: mime_type.to_string(),
            size_bytes: serde_json::to_string(&content)?.len() as i64,
            summary: None,
            content,
            created_at: Utc::now(),
        };

        self.artifacts
            .lock()
            .expect("lock poisoned")
            .insert(0, copy_artifact(&artifact));
        Ok(artifact)
    }

    async fn list(&self, limit: i64) -> anyhow::Result<Vec<Artifact>> {
        let artifacts = self.artifacts.lock().expect("lock poisoned");
        Ok(artifacts
            .iter()
            .take(limit as usize)
            .map(copy_artifact)
            .collect())
    }

    async fn list_by_instance(&self, _instance_id: Uuid, _limit: i64) -> anyhow::Result<Vec<Artifact>> {
        Ok(vec![])
    }

    async fn get(&self, id: Uuid) -> anyhow::Result<Option<Artifact>> {
        let artifacts = self.artifacts.lock().expect("lock poisoned");
        Ok(artifacts.iter().find(|a| a.id == id).map(copy_artifact))
    }

    async fn delete(&self, id: Uuid) -> anyhow::Result<bool> {
        let mut artifacts = self.artifacts.lock().expect("lock poisoned");
        let len_before = artifacts.len();
        artifacts.retain(|a| a.id != id);
        Ok(len_before != artifacts.len())
    }

    async fn update_scope(&self, id: Uuid, scope: ArtifactScope) -> anyhow::Result<bool> {
        let mut artifacts = self.artifacts.lock().expect("lock poisoned");
        if let Some(artifact) = artifacts.iter_mut().find(|a| a.id == id) {
            artifact.scope = scope;
            return Ok(true);
        }
        Ok(false)
    }
}

async fn setup_registry() -> (ToolRegistry, Arc<ArtifactService>, Arc<InMemoryArtifactRepository>) {
    let repo = Arc::new(InMemoryArtifactRepository::default());
    let artifact_service = Arc::new(ArtifactService::new(repo.clone()));
    let registry = ToolRegistry::new();
    register_builtin_tools(&registry, artifact_service.clone()).await;
    (registry, artifact_service, repo)
}

async fn execute_ok(registry: &ToolRegistry, name: &str, args: serde_json::Value) -> ToolResult {
    let result = registry
        .execute(name, args)
        .await
        .expect("tool execution should succeed");
    assert!(result.success, "tool should report success: {:?}", result.error);
    result
}

#[tokio::test]
async fn write_todos_creates_todo_document_artifact_for_scope() {
    let (registry, _artifact_service, repo) = setup_registry().await;

    let _ = execute_ok(
        &registry,
        "write_todos",
        json!({
            "scope": "private",
            "replace": true,
            "items": [
                { "id": "todo-1", "content": "Ship feature", "status": "pending" }
            ]
        }),
    )
    .await;

    let artifacts = repo.list(10).await.expect("artifacts should list");
    let todo_artifact = artifacts
        .iter()
        .find(|artifact| artifact.kind == TODO_DOCUMENT_KIND)
        .expect("todo_document artifact should exist");

    assert!(matches!(&todo_artifact.scope, ArtifactScope::Private));
    assert_eq!(todo_artifact.content["items"][0]["id"], "todo-1");
}

#[tokio::test]
async fn read_todos_returns_latest_document() {
    let (registry, _artifact_service, _repo) = setup_registry().await;

    let _ = execute_ok(
        &registry,
        "write_todos",
        json!({
            "scope": "private",
            "replace": true,
            "items": [
                { "id": "todo-1", "content": "Prepare tests", "status": "in_progress" }
            ]
        }),
    )
    .await;

    let result = execute_ok(&registry, "read_todos", json!({ "scope": "private" })).await;
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("read_todos returns JSON");

    assert_eq!(payload["items"].as_array().expect("items array").len(), 1);
    assert_eq!(payload["items"][0]["id"], "todo-1");
    assert_eq!(payload["items"][0]["status"], "in_progress");
}

#[tokio::test]
async fn update_todo_status_updates_single_item_without_replacing_document() {
    let (registry, _artifact_service, _repo) = setup_registry().await;

    let _ = execute_ok(
        &registry,
        "write_todos",
        json!({
            "scope": "private",
            "replace": true,
            "items": [
                { "id": "todo-1", "content": "Task A", "status": "pending" },
                { "id": "todo-2", "content": "Task B", "status": "pending" }
            ]
        }),
    )
    .await;

    let _ = execute_ok(
        &registry,
        "update_todo",
        json!({
            "scope": "private",
            "id": "todo-1",
            "status": "completed",
            "notes": "done"
        }),
    )
    .await;

    let result = execute_ok(&registry, "read_todos", json!({ "scope": "private" })).await;
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("read_todos returns JSON");
    let items = payload["items"].as_array().expect("items array");

    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["id"], "todo-1");
    assert_eq!(items[0]["status"], "completed");
    assert_eq!(items[0]["notes"], "done");
    assert_eq!(items[1]["id"], "todo-2");
    assert_eq!(items[1]["status"], "pending");
}
