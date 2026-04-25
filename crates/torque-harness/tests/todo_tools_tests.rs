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

    async fn find_latest_by_kind_scope_and_content_string(
        &self,
        kind: &str,
        scope: ArtifactScope,
        content_key: &str,
        content_value: &str,
    ) -> anyhow::Result<Option<Artifact>> {
        let artifacts = self.artifacts.lock().expect("lock poisoned");
        let scope_match = |left: &ArtifactScope, right: &ArtifactScope| match (left, right) {
            (ArtifactScope::Private, ArtifactScope::Private) => true,
            (ArtifactScope::TeamShared, ArtifactScope::TeamShared) => true,
            (ArtifactScope::ExternalPublished, ArtifactScope::ExternalPublished) => true,
            _ => false,
        };

        Ok(artifacts
            .iter()
            .find(|artifact| {
                artifact.kind == kind
                    && scope_match(&artifact.scope, &scope)
                    && artifact
                        .content
                        .get(content_key)
                        .and_then(|value| value.as_str())
                        .is_some_and(|value| value == content_value)
            })
            .map(copy_artifact))
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
async fn todo_tools_tests_write_todos_creates_todo_document_artifact_for_scope() {
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
    assert_eq!(todo_artifact.content["scope_key"], "private");
    assert_eq!(todo_artifact.content["items"][0]["id"], "todo-1");
}

#[tokio::test]
async fn todo_tools_tests_read_todos_returns_latest_document() {
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
async fn todo_tools_tests_update_todo_status_updates_single_item_without_replacing_document() {
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

#[tokio::test]
async fn todo_tools_tests_read_todos_is_isolated_by_logical_scope_key() {
    let (registry, _artifact_service, _repo) = setup_registry().await;

    let _ = execute_ok(
        &registry,
        "write_todos",
        json!({
            "scope": "feature_alpha",
            "replace": true,
            "items": [
                { "id": "a-1", "content": "Alpha", "status": "pending" }
            ]
        }),
    )
    .await;

    let _ = execute_ok(
        &registry,
        "write_todos",
        json!({
            "scope": "feature_beta",
            "replace": true,
            "items": [
                { "id": "b-1", "content": "Beta", "status": "in_progress" }
            ]
        }),
    )
    .await;

    let alpha = execute_ok(&registry, "read_todos", json!({ "scope": "feature_alpha" })).await;
    let beta = execute_ok(&registry, "read_todos", json!({ "scope": "feature_beta" })).await;

    let alpha_payload: serde_json::Value =
        serde_json::from_str(&alpha.content).expect("read_todos returns JSON");
    let beta_payload: serde_json::Value =
        serde_json::from_str(&beta.content).expect("read_todos returns JSON");

    assert_eq!(alpha_payload["scope_key"], "feature_alpha");
    assert_eq!(alpha_payload["items"][0]["id"], "a-1");
    assert_eq!(beta_payload["scope_key"], "feature_beta");
    assert_eq!(beta_payload["items"][0]["id"], "b-1");
}

#[tokio::test]
async fn todo_tools_tests_write_todos_merges_and_upserts_when_replace_false() {
    let (registry, _artifact_service, _repo) = setup_registry().await;

    let _ = execute_ok(
        &registry,
        "write_todos",
        json!({
            "scope": "private",
            "replace": true,
            "items": [
                { "id": "todo-1", "content": "Task A", "status": "pending" }
            ]
        }),
    )
    .await;

    let _ = execute_ok(
        &registry,
        "write_todos",
        json!({
            "scope": "private",
            "replace": false,
            "items": [
                { "id": "todo-1", "content": "Task A updated", "status": "in_progress", "notes": "started" },
                { "id": "todo-2", "content": "Task B", "status": "pending" }
            ]
        }),
    )
    .await;

    let result = execute_ok(&registry, "read_todos", json!({ "scope": "private" })).await;
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("read_todos returns JSON");
    let items = payload["items"].as_array().expect("items array");

    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["id"], "todo-1");
    assert_eq!(items[0]["content"], "Task A updated");
    assert_eq!(items[0]["status"], "in_progress");
    assert_eq!(items[0]["notes"], "started");
    assert_eq!(items[1]["id"], "todo-2");
}

#[tokio::test]
async fn todo_tools_tests_update_todo_notes_omitted_preserves_existing_notes() {
    let (registry, _artifact_service, _repo) = setup_registry().await;

    let _ = execute_ok(
        &registry,
        "write_todos",
        json!({
            "scope": "private",
            "replace": true,
            "items": [
                { "id": "todo-1", "content": "Task A", "status": "pending", "notes": "seed" }
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
            "status": "in_progress"
        }),
    )
    .await;

    let result = execute_ok(&registry, "read_todos", json!({ "scope": "private" })).await;
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("read_todos returns JSON");

    assert_eq!(payload["items"][0]["notes"], "seed");
}

#[tokio::test]
async fn todo_tools_tests_update_todo_notes_null_clears_existing_notes() {
    let (registry, _artifact_service, _repo) = setup_registry().await;

    let _ = execute_ok(
        &registry,
        "write_todos",
        json!({
            "scope": "private",
            "replace": true,
            "items": [
                { "id": "todo-1", "content": "Task A", "status": "pending", "notes": "seed" }
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
            "status": "in_progress",
            "notes": null
        }),
    )
    .await;

    let result = execute_ok(&registry, "read_todos", json!({ "scope": "private" })).await;
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("read_todos returns JSON");

    assert_eq!(payload["items"][0]["notes"], serde_json::Value::Null);
}

#[tokio::test]
async fn todo_tools_tests_update_todo_notes_wrong_type_is_rejected() {
    let (registry, _artifact_service, _repo) = setup_registry().await;

    let _ = execute_ok(
        &registry,
        "write_todos",
        json!({
            "scope": "private",
            "replace": true,
            "items": [
                { "id": "todo-1", "content": "Task A", "status": "pending" }
            ]
        }),
    )
    .await;

    let err = registry
        .execute(
            "update_todo",
            json!({
                "scope": "private",
                "id": "todo-1",
                "status": "completed",
                "notes": 123
            }),
        )
        .await
        .expect_err("invalid notes type must be rejected");

    assert!(err.to_string().contains("invalid update_todo args"));
}

#[tokio::test]
async fn todo_tools_tests_scope_team_shared_maps_to_team_shared_artifact_scope() {
    let (registry, _artifact_service, repo) = setup_registry().await;

    let _ = execute_ok(
        &registry,
        "write_todos",
        json!({
            "scope": "team_shared",
            "replace": true,
            "items": [
                { "id": "todo-1", "content": "Shared task", "status": "pending" }
            ]
        }),
    )
    .await;

    let artifact = repo
        .list(10)
        .await
        .expect("artifacts should list")
        .into_iter()
        .find(|a| a.kind == TODO_DOCUMENT_KIND && a.content["scope_key"] == "team_shared")
        .expect("team_shared todo artifact should exist");

    assert!(matches!(artifact.scope, ArtifactScope::TeamShared));
}

#[tokio::test]
async fn todo_tools_tests_scope_external_published_maps_to_external_published_artifact_scope() {
    let (registry, _artifact_service, repo) = setup_registry().await;

    let _ = execute_ok(
        &registry,
        "write_todos",
        json!({
            "scope": "external_published",
            "replace": true,
            "items": [
                { "id": "todo-1", "content": "Published task", "status": "pending" }
            ]
        }),
    )
    .await;

    let artifact = repo
        .list(10)
        .await
        .expect("artifacts should list")
        .into_iter()
        .find(|a| a.kind == TODO_DOCUMENT_KIND && a.content["scope_key"] == "external_published")
        .expect("external_published todo artifact should exist");

    assert!(matches!(artifact.scope, ArtifactScope::ExternalPublished));
}
