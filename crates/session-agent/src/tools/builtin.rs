use super::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct WebSearchTool;

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Search the web for information"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
        
        Ok(ToolResult {
            success: true,
            content: format!("Mock search results for: {}", query),
            error: None,
        })
    }
}

pub struct FileReadTool;

#[async_trait]
impl Tool for FileReadTool {
    fn name(&self) -> &str {
        "file_read"
    }

    fn description(&self) -> &str {
        "Read content from a file"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File path"
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        
        Ok(ToolResult {
            success: true,
            content: format!("Mock file content for: {}", path),
            error: None,
        })
    }
}

pub struct CodeExecuteTool;

#[async_trait]
impl Tool for CodeExecuteTool {
    fn name(&self) -> &str {
        "code_execute"
    }

    fn description(&self) -> &str {
        "Execute code and return the result"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "code": {
                    "type": "string",
                    "description": "Code to execute"
                },
                "language": {
                    "type": "string",
                    "description": "Programming language",
                    "enum": ["python", "javascript", "rust"]
                }
            },
            "required": ["code", "language"]
        })
    }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let code = args.get("code").and_then(|v| v.as_str()).unwrap_or("");
        let lang = args.get("language").and_then(|v| v.as_str()).unwrap_or("");
        
        Ok(ToolResult {
            success: true,
            content: format!("Mock execution of {} code:\n{}", lang, code),
            error: None,
        })
    }
}

pub fn create_builtin_tools() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(WebSearchTool),
        Box::new(FileReadTool),
        Box::new(CodeExecuteTool),
    ]
}