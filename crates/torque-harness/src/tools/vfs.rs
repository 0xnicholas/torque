use super::{Tool, ToolArc, ToolResult};
use crate::service::vfs::RoutedVfs;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(Debug, Deserialize)]
struct PathArgs {
    path: String,
}

#[derive(Debug, Deserialize)]
struct WriteFileArgs {
    path: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct EditFileArgs {
    path: String,
    old_string: String,
    new_string: String,
    #[serde(default)]
    replace_all: bool,
}

#[derive(Debug, Deserialize)]
struct GlobArgs {
    path: String,
    pattern: String,
}

pub fn create_vfs_tools(vfs: Arc<RoutedVfs>) -> Vec<ToolArc> {
    vec![
        Arc::new(LsTool::new(vfs.clone())) as ToolArc,
        Arc::new(ReadFileTool::new(vfs.clone())) as ToolArc,
        Arc::new(WriteFileTool::new(vfs.clone())) as ToolArc,
        Arc::new(EditFileTool::new(vfs.clone())) as ToolArc,
        Arc::new(GlobTool::new(vfs.clone())) as ToolArc,
        Arc::new(GrepTool::new(vfs)) as ToolArc,
    ]
}

macro_rules! tool_struct {
    ($name:ident) => {
        pub struct $name {
            vfs: Arc<RoutedVfs>,
        }

        impl $name {
            fn new(vfs: Arc<RoutedVfs>) -> Self {
                Self { vfs }
            }
        }
    };
}

tool_struct!(LsTool);
tool_struct!(ReadFileTool);
tool_struct!(WriteFileTool);
tool_struct!(EditFileTool);
tool_struct!(GlobTool);
tool_struct!(GrepTool);

#[async_trait]
impl Tool for LsTool {
    fn name(&self) -> &str {
        "ls"
    }

    fn description(&self) -> &str {
        "List files under /scratch or /workspace"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Directory path under /scratch or /workspace" }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let args: PathArgs = serde_json::from_value(args)?;
        respond(self.vfs.ls(&args.path).await)
    }
}

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read a UTF-8 text file from /scratch or /workspace"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "File path under /scratch or /workspace" }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let args: PathArgs = serde_json::from_value(args)?;
        match self.vfs.read(&args.path).await {
            Ok(content) => Ok(ToolResult {
                success: true,
                content,
                error: None,
            }),
            Err(err) => Ok(ToolResult {
                success: false,
                content: String::new(),
                error: Some(err.to_string()),
            }),
        }
    }
}

#[async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write a UTF-8 text file under /scratch or /workspace"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "content": { "type": "string" }
            },
            "required": ["path", "content"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let args: WriteFileArgs = serde_json::from_value(args)?;
        match self.vfs.write(&args.path, &args.content).await {
            Ok(()) => Ok(ToolResult {
                success: true,
                content: "ok".to_string(),
                error: None,
            }),
            Err(err) => Ok(ToolResult {
                success: false,
                content: String::new(),
                error: Some(err.to_string()),
            }),
        }
    }
}

#[async_trait]
impl Tool for EditFileTool {
    fn name(&self) -> &str {
        "edit_file"
    }

    fn description(&self) -> &str {
        "Edit text in a file under /scratch or /workspace"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "old_string": { "type": "string" },
                "new_string": { "type": "string" },
                "replace_all": { "type": "boolean" }
            },
            "required": ["path", "old_string", "new_string"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let args: EditFileArgs = serde_json::from_value(args)?;
        respond(self.vfs.edit(&args.path, &args.old_string, &args.new_string, args.replace_all).await)
    }
}

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }

    fn description(&self) -> &str {
        "Find files under /scratch or /workspace using a wildcard pattern"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "pattern": { "type": "string" }
            },
            "required": ["path", "pattern"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let args: GlobArgs = serde_json::from_value(args)?;
        respond(self.vfs.glob(&args.path, &args.pattern).await)
    }
}

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "Search for a substring inside files under /scratch or /workspace"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "pattern": { "type": "string" }
            },
            "required": ["path", "pattern"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let args: GlobArgs = serde_json::from_value(args)?;
        respond(self.vfs.grep(&args.path, &args.pattern).await)
    }
}

fn respond<T: serde::Serialize>(result: anyhow::Result<T>) -> anyhow::Result<ToolResult> {
    match result {
        Ok(value) => Ok(ToolResult {
            success: true,
            content: serde_json::to_string(&value)?,
            error: None,
        }),
        Err(err) => Ok(ToolResult {
            success: false,
            content: String::new(),
            error: Some(err.to_string()),
        }),
    }
}
