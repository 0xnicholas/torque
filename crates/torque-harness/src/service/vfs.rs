use async_trait::async_trait;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FileInfo {
    pub path: String,
    pub is_dir: bool,
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EditResult {
    pub occurrences: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct GrepMatch {
    pub path: String,
    pub line: usize,
    pub text: String,
}

#[async_trait]
pub trait VfsBackend: Send + Sync {
    async fn ls(&self, path: &str) -> anyhow::Result<Vec<FileInfo>>;
    async fn read(&self, path: &str) -> anyhow::Result<String>;
    async fn write(&self, path: &str, content: &str) -> anyhow::Result<()>;
    async fn edit(
        &self,
        path: &str,
        old_string: &str,
        new_string: &str,
        replace_all: bool,
    ) -> anyhow::Result<EditResult>;
    async fn glob(&self, path: &str, pattern: &str) -> anyhow::Result<Vec<FileInfo>>;
    async fn grep(&self, path: &str, pattern: &str) -> anyhow::Result<Vec<GrepMatch>>;
}

pub struct RoutedVfs {
    scratch: Arc<dyn VfsBackend>,
    workspace: Arc<dyn VfsBackend>,
}

impl RoutedVfs {
    pub fn new(scratch: Arc<dyn VfsBackend>, workspace: Arc<dyn VfsBackend>) -> Self {
        Self { scratch, workspace }
    }

    pub fn for_current_workspace() -> Self {
        let workspace_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self::new(
            Arc::new(ScratchBackend::default()),
            Arc::new(WorkspaceBackend::new(workspace_root)),
        )
    }

    pub async fn ls(&self, path: &str) -> anyhow::Result<Vec<FileInfo>> {
        match route_backend(path, &self.scratch, &self.workspace)? {
            RoutedBackend::Scratch(backend) => backend.ls(path).await,
            RoutedBackend::Workspace(backend) => backend.ls(path).await,
        }
    }

    pub async fn read(&self, path: &str) -> anyhow::Result<String> {
        match route_backend(path, &self.scratch, &self.workspace)? {
            RoutedBackend::Scratch(backend) => backend.read(path).await,
            RoutedBackend::Workspace(backend) => backend.read(path).await,
        }
    }

    pub async fn write(&self, path: &str, content: &str) -> anyhow::Result<()> {
        match route_backend(path, &self.scratch, &self.workspace)? {
            RoutedBackend::Scratch(backend) => backend.write(path, content).await,
            RoutedBackend::Workspace(backend) => backend.write(path, content).await,
        }
    }

    pub async fn edit(
        &self,
        path: &str,
        old_string: &str,
        new_string: &str,
        replace_all: bool,
    ) -> anyhow::Result<EditResult> {
        match route_backend(path, &self.scratch, &self.workspace)? {
            RoutedBackend::Scratch(backend) => {
                backend.edit(path, old_string, new_string, replace_all).await
            }
            RoutedBackend::Workspace(backend) => {
                backend.edit(path, old_string, new_string, replace_all).await
            }
        }
    }

    pub async fn glob(&self, path: &str, pattern: &str) -> anyhow::Result<Vec<FileInfo>> {
        match route_backend(path, &self.scratch, &self.workspace)? {
            RoutedBackend::Scratch(backend) => backend.glob(path, pattern).await,
            RoutedBackend::Workspace(backend) => backend.glob(path, pattern).await,
        }
    }

    pub async fn grep(&self, path: &str, pattern: &str) -> anyhow::Result<Vec<GrepMatch>> {
        match route_backend(path, &self.scratch, &self.workspace)? {
            RoutedBackend::Scratch(backend) => backend.grep(path, pattern).await,
            RoutedBackend::Workspace(backend) => backend.grep(path, pattern).await,
        }
    }
}

enum RoutedBackend<'a> {
    Scratch(&'a Arc<dyn VfsBackend>),
    Workspace(&'a Arc<dyn VfsBackend>),
}

fn route_backend<'a>(
    path: &str,
    scratch: &'a Arc<dyn VfsBackend>,
    workspace: &'a Arc<dyn VfsBackend>,
) -> anyhow::Result<RoutedBackend<'a>> {
    if path == "/scratch" || path.starts_with("/scratch/") {
        Ok(RoutedBackend::Scratch(scratch))
    } else if path == "/workspace" || path.starts_with("/workspace/") {
        Ok(RoutedBackend::Workspace(workspace))
    } else {
        Err(anyhow::anyhow!(
            "unsupported VFS path '{}': expected /scratch or /workspace",
            path
        ))
    }
}

#[derive(Default)]
pub struct ScratchBackend {
    files: Mutex<HashMap<String, String>>,
}

#[async_trait]
impl VfsBackend for ScratchBackend {
    async fn ls(&self, path: &str) -> anyhow::Result<Vec<FileInfo>> {
        ensure_prefix(path, "/scratch")?;
        let files = self.files.lock().expect("scratch lock poisoned");
        let target = normalize_virtual_path(path);
        let infos = files
            .keys()
            .filter(|candidate| candidate.starts_with(&target))
            .map(|candidate| FileInfo {
                path: candidate.clone(),
                is_dir: false,
                size: files.get(candidate).map(|content| content.len() as u64),
            })
            .collect();
        Ok(infos)
    }

    async fn read(&self, path: &str) -> anyhow::Result<String> {
        ensure_prefix(path, "/scratch")?;
        self.files
            .lock()
            .expect("scratch lock poisoned")
            .get(path)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("scratch file not found: {}", path))
    }

    async fn write(&self, path: &str, content: &str) -> anyhow::Result<()> {
        ensure_prefix(path, "/scratch")?;
        self.files
            .lock()
            .expect("scratch lock poisoned")
            .insert(path.to_string(), content.to_string());
        Ok(())
    }

    async fn edit(
        &self,
        path: &str,
        old_string: &str,
        new_string: &str,
        replace_all: bool,
    ) -> anyhow::Result<EditResult> {
        let mut files = self.files.lock().expect("scratch lock poisoned");
        let content = files
            .get_mut(path)
            .ok_or_else(|| anyhow::anyhow!("scratch file not found: {}", path))?;
        let occurrences = content.matches(old_string).count();
        validate_edit_occurrences(path, old_string, replace_all, occurrences)?;
        if replace_all {
            *content = content.replace(old_string, new_string);
        } else {
            *content = content.replacen(old_string, new_string, 1);
        }
        Ok(EditResult { occurrences })
    }

    async fn glob(&self, path: &str, pattern: &str) -> anyhow::Result<Vec<FileInfo>> {
        ensure_prefix(path, "/scratch")?;
        let files = self.files.lock().expect("scratch lock poisoned");
        Ok(files
            .keys()
            .filter(|candidate| candidate.starts_with(path))
            .filter(|candidate| wildcard_match(Path::new(candidate).file_name().and_then(|v| v.to_str()).unwrap_or(""), pattern))
            .map(|candidate| FileInfo {
                path: candidate.clone(),
                is_dir: false,
                size: files.get(candidate).map(|content| content.len() as u64),
            })
            .collect())
    }

    async fn grep(&self, path: &str, pattern: &str) -> anyhow::Result<Vec<GrepMatch>> {
        ensure_prefix(path, "/scratch")?;
        let files = self.files.lock().expect("scratch lock poisoned");
        let mut matches = Vec::new();
        for (candidate, content) in files.iter() {
            if !candidate.starts_with(path) {
                continue;
            }
            for (idx, line) in content.lines().enumerate() {
                if line.contains(pattern) {
                    matches.push(GrepMatch {
                        path: candidate.clone(),
                        line: idx + 1,
                        text: line.to_string(),
                    });
                }
            }
        }
        Ok(matches)
    }
}

pub struct WorkspaceBackend {
    root: PathBuf,
}

impl WorkspaceBackend {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn resolve(&self, path: &str) -> anyhow::Result<PathBuf> {
        ensure_prefix(path, "/workspace")?;
        let relative = path.trim_start_matches("/workspace").trim_start_matches('/');
        let resolved = self.root.join(relative);
        Ok(resolved)
    }
}

#[async_trait]
impl VfsBackend for WorkspaceBackend {
    async fn ls(&self, path: &str) -> anyhow::Result<Vec<FileInfo>> {
        let dir = self.resolve(path)?;
        let mut infos = Vec::new();
        if dir.is_dir() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let metadata = entry.metadata()?;
                infos.push(FileInfo {
                    path: workspace_display_path(&self.root, &entry.path()),
                    is_dir: metadata.is_dir(),
                    size: if metadata.is_file() { Some(metadata.len()) } else { None },
                });
            }
        }
        infos.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(infos)
    }

    async fn read(&self, path: &str) -> anyhow::Result<String> {
        let resolved = self.resolve(path)?;
        Ok(fs::read_to_string(resolved)?)
    }

    async fn write(&self, path: &str, content: &str) -> anyhow::Result<()> {
        let resolved = self.resolve(path)?;
        if let Some(parent) = resolved.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(resolved, content)?;
        Ok(())
    }

    async fn edit(
        &self,
        path: &str,
        old_string: &str,
        new_string: &str,
        replace_all: bool,
    ) -> anyhow::Result<EditResult> {
        let resolved = self.resolve(path)?;
        let mut content = fs::read_to_string(&resolved)?;
        let occurrences = content.matches(old_string).count();
        validate_edit_occurrences(path, old_string, replace_all, occurrences)?;
        if replace_all {
            content = content.replace(old_string, new_string);
        } else {
            content = content.replacen(old_string, new_string, 1);
        }
        fs::write(resolved, content)?;
        Ok(EditResult { occurrences })
    }

    async fn glob(&self, path: &str, pattern: &str) -> anyhow::Result<Vec<FileInfo>> {
        let resolved = self.resolve(path)?;
        let mut infos = Vec::new();
        visit_files(&resolved, &mut |candidate| {
            let file_name = candidate.file_name().and_then(|v| v.to_str()).unwrap_or("");
            if wildcard_match(file_name, pattern) {
                let metadata = fs::metadata(candidate)?;
                infos.push(FileInfo {
                    path: workspace_display_path(&self.root, candidate),
                    is_dir: metadata.is_dir(),
                    size: if metadata.is_file() { Some(metadata.len()) } else { None },
                });
            }
            Ok(())
        })?;
        infos.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(infos)
    }

    async fn grep(&self, path: &str, pattern: &str) -> anyhow::Result<Vec<GrepMatch>> {
        let resolved = self.resolve(path)?;
        let mut found = Vec::new();
        visit_files(&resolved, &mut |candidate| {
            if !candidate.is_file() {
                return Ok(());
            }
            let content = fs::read_to_string(candidate)?;
            for (idx, line) in content.lines().enumerate() {
                if line.contains(pattern) {
                    found.push(GrepMatch {
                        path: workspace_display_path(&self.root, candidate),
                        line: idx + 1,
                        text: line.to_string(),
                    });
                }
            }
            Ok(())
        })?;
        Ok(found)
    }
}

fn normalize_virtual_path(path: &str) -> String {
    path.trim_end_matches('/').to_string()
}

fn ensure_prefix(path: &str, prefix: &str) -> anyhow::Result<()> {
    if path == prefix || path.starts_with(&format!("{}/", prefix)) {
        Ok(())
    } else {
        Err(anyhow::anyhow!("path '{}' is outside {}", path, prefix))
    }
}

fn validate_edit_occurrences(
    path: &str,
    old_string: &str,
    replace_all: bool,
    occurrences: usize,
) -> anyhow::Result<()> {
    if occurrences == 0 {
        Err(anyhow::anyhow!(
            "edit_file could not find '{}' in {}",
            old_string,
            path
        ))
    } else if occurrences > 1 && !replace_all {
        Err(anyhow::anyhow!(
            "edit_file found {} matches for '{}' in {}; set replace_all=true or narrow the match",
            occurrences,
            old_string,
            path
        ))
    } else {
        Ok(())
    }
}

fn workspace_display_path(root: &Path, candidate: &Path) -> String {
    let relative = candidate
        .strip_prefix(root)
        .unwrap_or(candidate)
        .to_string_lossy()
        .replace('\\', "/");
    if relative.is_empty() {
        "/workspace".to_string()
    } else {
        format!("/workspace/{}", relative)
    }
}

fn visit_files<F>(root: &Path, visitor: &mut F) -> anyhow::Result<()>
where
    F: FnMut(&Path) -> anyhow::Result<()>,
{
    if root.is_file() {
        visitor(root)?;
        return Ok(());
    }
    if !root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            visit_files(&path, visitor)?;
        } else {
            visitor(&path)?;
        }
    }
    Ok(())
}

fn wildcard_match(candidate: &str, pattern: &str) -> bool {
    wildcard_match_bytes(candidate.as_bytes(), pattern.as_bytes())
}

fn wildcard_match_bytes(candidate: &[u8], pattern: &[u8]) -> bool {
    if pattern.is_empty() {
        return candidate.is_empty();
    }
    match pattern[0] {
        b'*' => {
            wildcard_match_bytes(candidate, &pattern[1..])
                || (!candidate.is_empty() && wildcard_match_bytes(&candidate[1..], pattern))
        }
        b'?' => !candidate.is_empty() && wildcard_match_bytes(&candidate[1..], &pattern[1..]),
        ch => !candidate.is_empty() && candidate[0] == ch && wildcard_match_bytes(&candidate[1..], &pattern[1..]),
    }
}
