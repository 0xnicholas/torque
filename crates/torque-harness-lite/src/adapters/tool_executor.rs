use async_trait::async_trait;
use torque_runtime::environment::{RuntimeExecutionContext, RuntimeToolExecutor};
use torque_runtime::tools::{RuntimeToolDef, RuntimeToolResult};
use torque_runtime::vfs::{RoutedVfs, VfsBackend};

pub struct LiteToolExecutor {
    vfs: RoutedVfs,
}

impl LiteToolExecutor {
    pub fn new() -> Self {
        Self {
            vfs: RoutedVfs::for_current_workspace(),
        }
    }
}

#[async_trait]
impl RuntimeToolExecutor for LiteToolExecutor {
    async fn execute(
        &self,
        _ctx: RuntimeExecutionContext,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> anyhow::Result<RuntimeToolResult> {
        let result = match tool_name {
            "read_file" => {
                let path = arguments["path"].as_str().unwrap_or("");
                match VfsBackend::read(&self.vfs, path).await {
                    Ok(content) => RuntimeToolResult::success(content),
                    Err(e) => RuntimeToolResult::failure(e.to_string()),
                }
            }
            "write_file" => {
                let path = arguments["path"].as_str().unwrap_or("");
                let content = arguments["content"].as_str().unwrap_or("");
                match VfsBackend::write(&self.vfs, path, content).await {
                    Ok(()) => RuntimeToolResult::success(format!("wrote {}", path)),
                    Err(e) => RuntimeToolResult::failure(e.to_string()),
                }
            }
            "ls" => {
                let path = arguments["path"].as_str().unwrap_or("/");
                match self.vfs.ls(path).await {
                    Ok(files) => RuntimeToolResult::success(
                        files
                            .iter()
                            .map(|f| f.path.clone())
                            .collect::<Vec<_>>()
                            .join("\n"),
                    ),
                    Err(e) => RuntimeToolResult::failure(e.to_string()),
                }
            }
            "edit_file" => {
                let path = arguments["path"].as_str().unwrap_or("");
                let old = arguments["old_string"].as_str().unwrap_or("");
                let new = arguments["new_string"].as_str().unwrap_or("");
                let replace_all = arguments["replace_all"].as_bool().unwrap_or(false);
                match self.vfs.edit(path, old, new, replace_all).await {
                    Ok(result) => RuntimeToolResult::success(format!(
                        "replaced {} occurrence(s)",
                        result.occurrences
                    )),
                    Err(e) => RuntimeToolResult::failure(e.to_string()),
                }
            }
            "glob" => {
                let path = arguments["path"].as_str().unwrap_or("/");
                let pattern = arguments["pattern"].as_str().unwrap_or("*");
                match self.vfs.glob(path, pattern).await {
                    Ok(files) => RuntimeToolResult::success(
                        files
                            .iter()
                            .map(|f| f.path.clone())
                            .collect::<Vec<_>>()
                            .join("\n"),
                    ),
                    Err(e) => RuntimeToolResult::failure(e.to_string()),
                }
            }
            "grep" => {
                let path = arguments["path"].as_str().unwrap_or("/");
                let pattern = arguments["pattern"].as_str().unwrap_or("");
                match self.vfs.grep(path, pattern).await {
                    Ok(matches) => RuntimeToolResult::success(
                        matches
                            .iter()
                            .map(|m| format!("{}:{}: {}", m.path, m.line, m.text))
                            .collect::<Vec<_>>()
                            .join("\n"),
                    ),
                    Err(e) => RuntimeToolResult::failure(e.to_string()),
                }
            }
            _ => RuntimeToolResult::failure(format!("unknown tool: {}", tool_name)),
        };

        Ok(result)
    }

    async fn tool_defs(&self) -> anyhow::Result<Vec<RuntimeToolDef>> {
        Ok(vec![
            RuntimeToolDef {
                name: "read_file".into(),
                description: "Read a file from the filesystem".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {"type": "string", "description": "Path to the file"}
                    },
                    "required": ["path"]
                }),
            },
            RuntimeToolDef {
                name: "write_file".into(),
                description: "Write content to a file".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {"type": "string", "description": "Path to the file"},
                        "content": {"type": "string", "description": "Content to write"}
                    },
                    "required": ["path", "content"]
                }),
            },
            RuntimeToolDef {
                name: "ls".into(),
                description: "List files in a directory".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {"type": "string", "description": "Directory path, default /"}
                    }
                }),
            },
            RuntimeToolDef {
                name: "edit_file".into(),
                description: "Find and replace text in a file".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"},
                        "old_string": {"type": "string"},
                        "new_string": {"type": "string"},
                        "replace_all": {"type": "boolean"}
                    },
                    "required": ["path", "old_string", "new_string"]
                }),
            },
            RuntimeToolDef {
                name: "glob".into(),
                description: "Find files by glob pattern".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"},
                        "pattern": {"type": "string"}
                    },
                    "required": ["path", "pattern"]
                }),
            },
            RuntimeToolDef {
                name: "grep".into(),
                description: "Search file contents with regex".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"},
                        "pattern": {"type": "string"}
                    },
                    "required": ["path", "pattern"]
                }),
            },
        ])
    }
}
