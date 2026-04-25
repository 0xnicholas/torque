use super::{Tool, ToolArc, ToolResult};
use crate::service::ArtifactService;
use crate::tools::todos::create_todo_tools;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;
pub use crate::tools::todos::{TodoDocument, TodoItem, TodoStatus};

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

pub fn create_demo_builtin_tools() -> Vec<Box<dyn Tool>> {
    vec![Box::new(WebSearchTool)]
}

pub fn create_builtin_tools(artifact_service: Arc<ArtifactService>) -> Vec<ToolArc> {
    let mut tools: Vec<ToolArc> = create_demo_builtin_tools().into_iter().map(Arc::from).collect();
    tools.extend(create_todo_tools(artifact_service));
    tools
}
