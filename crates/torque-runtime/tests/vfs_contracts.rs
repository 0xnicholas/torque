use torque_runtime::vfs::{RoutedVfs, ScratchBackend, VfsBackend, WorkspaceBackend};
use std::sync::Arc;

fn test_vfs(workspace_root: std::path::PathBuf) -> RoutedVfs {
    RoutedVfs::new(vec![
        ("/scratch".to_string(), Arc::new(ScratchBackend::default())),
        ("/workspace".to_string(), Arc::new(WorkspaceBackend::new(workspace_root))),
    ])
}

#[tokio::test]
async fn scratch_write_and_read() {
    let vfs = test_vfs(std::env::temp_dir());

    let path = "/scratch/test.txt";
    VfsBackend::write(&vfs, path, "hello world").await.expect("write");
    let content = VfsBackend::read(&vfs, path).await.expect("read");
    assert_eq!(content, "hello world");
}

#[tokio::test]
async fn scratch_ls_lists_files() {
    let vfs = test_vfs(std::env::temp_dir());

    VfsBackend::write(&vfs, "/scratch/a.txt", "a").await.unwrap();
    VfsBackend::write(&vfs, "/scratch/b.txt", "b").await.unwrap();

    let files = vfs.ls("/scratch").await.expect("ls");
    assert!(files.iter().any(|f| f.path == "/scratch/a.txt"));
    assert!(files.iter().any(|f| f.path == "/scratch/b.txt"));
}

#[tokio::test]
async fn scratch_edit_replaces_text() {
    let vfs = test_vfs(std::env::temp_dir());

    VfsBackend::write(&vfs, "/scratch/edit.txt", "before").await.unwrap();
    let result = VfsBackend::edit(&vfs, "/scratch/edit.txt", "before", "after", false)
        .await
        .expect("edit");
    assert_eq!(result.occurrences, 1);

    let content = VfsBackend::read(&vfs, "/scratch/edit.txt").await.unwrap();
    assert_eq!(content, "after");
}

#[tokio::test]
async fn scratch_rejects_non_scratch_prefix() {
    let vfs = test_vfs(std::env::temp_dir());

    let result = VfsBackend::read(&vfs, "/other/file.txt").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn workspace_write_and_read() {
    let dir = tempfile::tempdir().expect("temp dir");
    let vfs = test_vfs(dir.path().to_path_buf());

    let path = "/workspace/test.txt";
    VfsBackend::write(&vfs, path, "workspace content").await.expect("write");
    let content = VfsBackend::read(&vfs, path).await.expect("read");
    assert_eq!(content, "workspace content");
}

#[tokio::test]
async fn workspace_rejects_path_traversal() {
    let dir = tempfile::tempdir().expect("temp dir");
    let vfs = test_vfs(dir.path().to_path_buf());

    // Attempt to escape via .. traversal
    let result = VfsBackend::read(&vfs, "/workspace/../../../etc/passwd").await;
    assert!(result.is_err(), "path traversal should be rejected");
}

#[tokio::test]
async fn workspace_glob_finds_files() {
    let dir = tempfile::tempdir().expect("temp dir");
    let vfs = test_vfs(dir.path().to_path_buf());

    VfsBackend::write(&vfs, "/workspace/foo.rs", "rust").await.unwrap();
    VfsBackend::write(&vfs, "/workspace/bar.txt", "text").await.unwrap();

    let files = VfsBackend::glob(&vfs, "/workspace", "*.rs").await.expect("glob");
    assert_eq!(files.len(), 1);
    assert!(files[0].path.contains("foo.rs"));
}

#[tokio::test]
async fn workspace_grep_finds_matches() {
    let dir = tempfile::tempdir().expect("temp dir");
    let vfs = test_vfs(dir.path().to_path_buf());

    VfsBackend::write(&vfs, "/workspace/grep.txt", "line one\nline two\nanother one").await.unwrap();

    // grep takes a directory path and searches recursively
    let matches = VfsBackend::grep(&vfs, "/workspace", "one")
        .await
        .expect("grep");
    assert!(matches.iter().any(|m| m.path.contains("grep.txt")));
    assert!(matches.len() >= 2);
}

#[tokio::test]
async fn routed_vfs_custom_routing_table() {
    let scratch = Arc::new(ScratchBackend::default());
    let ws = Arc::new(WorkspaceBackend::new(std::path::PathBuf::from(".")));

    let vfs = RoutedVfs::new(vec![
        ("/scratch".to_string(), scratch.clone()),
        ("/workspace".to_string(), ws.clone()),
    ]);

    vfs.write("/scratch/test.txt", "hello").await.unwrap();
    let content = vfs.read("/scratch/test.txt").await.unwrap();
    assert_eq!(content, "hello");
}

#[tokio::test]
async fn routed_vfs_root_aggregates_all_backends() {
    let scratch = Arc::new(ScratchBackend::default());
    scratch.write("/scratch/file.txt", "data").await.unwrap();
    let ws = Arc::new(WorkspaceBackend::new(std::path::PathBuf::from(".")));
    ws.write("/workspace/other.txt", "more").await.unwrap();

    let vfs = RoutedVfs::new(vec![
        ("/scratch".to_string(), scratch),
        ("/workspace".to_string(), ws),
    ]);

    let entries = vfs.ls("/").await.unwrap();
    assert!(!entries.is_empty());
    assert!(entries.iter().any(|e| e.path.contains("file.txt")));
    assert!(entries.iter().any(|e| e.path.contains("other.txt")));
}

#[tokio::test]
async fn routed_vfs_unknown_path_returns_error() {
    let vfs = RoutedVfs::new(vec![]);
    let result = vfs.read("/unknown/test.txt").await;
    assert!(result.is_err());
}
