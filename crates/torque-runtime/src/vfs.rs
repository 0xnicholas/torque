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

#[async_trait]
impl VfsBackend for RoutedVfs {
    async fn ls(&self, path: &str) -> anyhow::Result<Vec<FileInfo>> {
        RoutedVfs::ls(self, path).await
    }

    async fn read(&self, path: &str) -> anyhow::Result<String> {
        RoutedVfs::read(self, path).await
    }

    async fn write(&self, path: &str, content: &str) -> anyhow::Result<()> {
        RoutedVfs::write(self, path, content).await
    }

    async fn edit(
        &self,
        path: &str,
        old_string: &str,
        new_string: &str,
        replace_all: bool,
    ) -> anyhow::Result<EditResult> {
        RoutedVfs::edit(self, path, old_string, new_string, replace_all).await
    }

    async fn glob(&self, path: &str, pattern: &str) -> anyhow::Result<Vec<FileInfo>> {
        RoutedVfs::glob(self, path, pattern).await
    }

    async fn grep(&self, path: &str, pattern: &str) -> anyhow::Result<Vec<GrepMatch>> {
        RoutedVfs::grep(self, path, pattern).await
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
            .filter(|candidate| {
                wildcard_match(
                    Path::new(candidate)
                        .file_name()
                        .and_then(|v| v.to_str())
                        .unwrap_or(""),
                    pattern,
                )
            })
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
        let canonical = root.canonicalize().unwrap_or(root);
        Self { root: canonical }
    }

    fn resolve(&self, path: &str) -> anyhow::Result<PathBuf> {
        ensure_prefix(path, "/workspace")?;
        let relative = path.trim_start_matches("/workspace").trim_start_matches('/');

        if relative.is_empty() {
            return Ok(self.root.clone());
        }

        let resolved = self.root.join(relative);
        let canonical = resolved.canonicalize().unwrap_or(resolved);

        if !canonical.starts_with(&self.root) {
            return Err(anyhow::anyhow!(
                "path '{}' escapes workspace root",
                path
            ));
        }
        Ok(canonical)
    }
}

#[async_trait]
impl VfsBackend for WorkspaceBackend {
    async fn ls(&self, path: &str) -> anyhow::Result<Vec<FileInfo>> {
        let resolved = self.resolve(path)?;
        if !resolved.exists() {
            return Ok(vec![]);
        }
        let mut infos = Vec::new();
        for entry in fs::read_dir(&resolved)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            let path = format!(
                "/workspace/{}",
                entry
                    .path()
                    .strip_prefix(&self.root)?
                    .to_string_lossy()
                    .replace('\\', "/")
            );
            infos.push(FileInfo {
                path,
                is_dir: metadata.is_dir(),
                size: metadata.is_file().then_some(metadata.len()),
            });
        }
        Ok(infos)
    }

    async fn read(&self, path: &str) -> anyhow::Result<String> {
        Ok(fs::read_to_string(self.resolve(path)?)?)
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
        if !resolved.exists() {
            return Ok(vec![]);
        }

        let mut matches = Vec::new();
        collect_matches(&resolved, &self.root, pattern, &mut matches)?;
        Ok(matches)
    }

    async fn grep(&self, path: &str, pattern: &str) -> anyhow::Result<Vec<GrepMatch>> {
        let resolved = self.resolve(path)?;
        if !resolved.exists() {
            return Ok(vec![]);
        }
        let mut matches = Vec::new();
        collect_grep_matches(&resolved, &self.root, pattern, &mut matches)?;
        Ok(matches)
    }
}

fn ensure_prefix(path: &str, prefix: &str) -> anyhow::Result<()> {
    if path == prefix || path.starts_with(&format!("{prefix}/")) {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "path '{}' must stay under '{}'",
            path,
            prefix
        ))
    }
}

fn normalize_virtual_path(path: &str) -> String {
    if path.ends_with('/') {
        path.trim_end_matches('/').to_string()
    } else {
        path.to_string()
    }
}

fn validate_edit_occurrences(
    path: &str,
    old_string: &str,
    replace_all: bool,
    occurrences: usize,
) -> anyhow::Result<()> {
    if occurrences == 0 {
        return Err(anyhow::anyhow!(
            "edit target '{}' not found in {}",
            old_string,
            path
        ));
    }
    if !replace_all && occurrences != 1 {
        return Err(anyhow::anyhow!(
            "edit target '{}' matched {} locations in {}",
            old_string,
            occurrences,
            path
        ));
    }
    Ok(())
}

fn collect_matches(
    dir: &Path,
    root: &Path,
    pattern: &str,
    matches: &mut Vec<FileInfo>,
) -> anyhow::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            collect_matches(&path, root, pattern, matches)?;
            continue;
        }
        let file_name = path.file_name().and_then(|value| value.to_str()).unwrap_or("");
        if wildcard_match(file_name, pattern) {
            matches.push(FileInfo {
                path: format!(
                    "/workspace/{}",
                    path.strip_prefix(root)?.to_string_lossy().replace('\\', "/")
                ),
                is_dir: false,
                size: Some(metadata.len()),
            });
        }
    }
    Ok(())
}

fn collect_grep_matches(
    dir: &Path,
    root: &Path,
    pattern: &str,
    matches: &mut Vec<GrepMatch>,
) -> anyhow::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            collect_grep_matches(&path, root, pattern, matches)?;
            continue;
        }
        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(_) => continue,
        };
        for (idx, line) in content.lines().enumerate() {
            if line.contains(pattern) {
                matches.push(GrepMatch {
                    path: format!(
                        "/workspace/{}",
                        path.strip_prefix(root)?.to_string_lossy().replace('\\', "/")
                    ),
                    line: idx + 1,
                    text: line.to_string(),
                });
            }
        }
    }
    Ok(())
}

fn wildcard_match(candidate: &str, pattern: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some((prefix, suffix)) = pattern.split_once('*') {
        candidate.starts_with(prefix) && candidate.ends_with(suffix)
    } else {
        candidate == pattern
    }
}
