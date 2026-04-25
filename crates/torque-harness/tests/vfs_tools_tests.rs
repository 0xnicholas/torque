use async_trait::async_trait;
use chrono::Utc;
use serde_json::json;
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

fn setup_tool_service() -> ToolService {
    let artifact_service = Arc::new(ArtifactService::new(Arc::new(NoopArtifactRepository)));
    ToolService::new_with_builtins(artifact_service)
}

#[tokio::test]
async fn vfs_tools_tests_lists_scratch_root() {
    let tool_service = setup_tool_service();

    let result = tool_service
        .registry()
        .execute("ls", json!({ "path": "/scratch" }))
        .await
        .expect("ls should execute");

    assert!(result.success, "ls should succeed once implemented");
}

#[tokio::test]
async fn vfs_tools_tests_writes_and_reads_scratch_file() {
    let tool_service = setup_tool_service();

    let write = tool_service
        .registry()
        .execute(
            "write_file",
            json!({ "path": "/scratch/notes.txt", "content": "hello scratch" }),
        )
        .await
        .expect("write_file should execute");
    assert!(write.success, "write_file should succeed once implemented");

    let read = tool_service
        .registry()
        .execute("read_file", json!({ "path": "/scratch/notes.txt" }))
        .await
        .expect("read_file should execute");
    assert!(read.success, "read_file should succeed once implemented");
    assert_eq!(read.content, "hello scratch");
}

#[tokio::test]
async fn vfs_tools_tests_rejects_unknown_prefix() {
    let tool_service = setup_tool_service();

    let result = tool_service
        .registry()
        .execute("read_file", json!({ "path": "/tmp/notes.txt" }))
        .await
        .expect("read_file should execute");

    assert!(!result.success, "unknown prefixes must be rejected");
}

#[tokio::test]
async fn vfs_tools_tests_edits_workspace_file_with_unique_match() {
    let tool_service = setup_tool_service();
    let path = "/workspace/.vfs-tests/tmp-vfs-edit.txt";

    let write = tool_service
        .registry()
        .execute(
            "write_file",
            json!({ "path": path, "content": "alpha beta gamma" }),
        )
        .await
        .expect("write_file should execute");
    assert!(write.success, "workspace write should succeed once implemented");

    let edit = tool_service
        .registry()
        .execute(
            "edit_file",
            json!({
                "path": path,
                "old_string": "beta",
                "new_string": "BETA",
                "replace_all": false
            }),
        )
        .await
        .expect("edit_file should execute");
    assert!(edit.success, "edit_file should succeed once implemented");

    let read = tool_service
        .registry()
        .execute("read_file", json!({ "path": path }))
        .await
        .expect("read_file should execute");
    assert!(read.success);
    assert_eq!(read.content, "alpha BETA gamma");
}

#[tokio::test]
async fn vfs_tools_tests_edit_requires_unique_match() {
    let tool_service = setup_tool_service();
    let path = "/workspace/.vfs-tests/tmp-vfs-nonunique.txt";

    let write = tool_service
        .registry()
        .execute(
            "write_file",
            json!({ "path": path, "content": "dup dup" }),
        )
        .await
        .expect("write_file should execute");
    assert!(write.success);

    let edit = tool_service
        .registry()
        .execute(
            "edit_file",
            json!({
                "path": path,
                "old_string": "dup",
                "new_string": "changed",
                "replace_all": false
            }),
        )
        .await
        .expect("edit_file should execute");

    assert!(
        !edit.success,
        "edit_file must fail when old_string matches multiple locations and replace_all is false"
    );
}

#[tokio::test]
async fn vfs_tools_tests_glob_and_grep_workspace_files() {
    let tool_service = setup_tool_service();
    let root = "/workspace/.vfs-tests";

    for (path, content) in [
        ("/workspace/.vfs-tests/tmp-vfs-a.txt", "alpha needle"),
        ("/workspace/.vfs-tests/tmp-vfs-b.txt", "beta haystack"),
    ] {
        let write = tool_service
            .registry()
            .execute("write_file", json!({ "path": path, "content": content }))
            .await
            .expect("write_file should execute");
        assert!(write.success);
    }

    let glob = tool_service
        .registry()
        .execute(
            "glob",
            json!({ "path": root, "pattern": "tmp-vfs-*.txt" }),
        )
        .await
        .expect("glob should execute");
    assert!(glob.success, "glob should succeed once implemented");

    let grep = tool_service
        .registry()
        .execute("grep", json!({ "path": root, "pattern": "needle" }))
        .await
        .expect("grep should execute");
    assert!(grep.success, "grep should succeed once implemented");
    assert!(
        grep.content.contains("tmp-vfs-a.txt"),
        "grep output should mention matching file once implemented"
    );
}
