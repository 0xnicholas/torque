use crate::tools::{ToolArc, ToolResult};
use serde_json::Value;
use std::collections::HashMap;
use tokio::sync::RwLock;

pub struct ToolRegistry {
    tools: RwLock<HashMap<String, ToolArc>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: RwLock::new(HashMap::new()),
        }
    }

    pub async fn register(&self, tool: ToolArc) {
        let name = tool.name().to_string();
        self.tools.write().await.insert(name, tool);
    }

    pub async fn get(&self, name: &str) -> Option<ToolArc> {
        self.tools.read().await.get(name).cloned()
    }

    pub async fn list(&self) -> Vec<ToolArc> {
        self.tools.read().await.values().cloned().collect()
    }

    pub async fn execute(&self, name: &str, args: Value) -> anyhow::Result<ToolResult> {
        match self.get(name).await {
            Some(tool) => tool.execute(args).await,
            None => Ok(ToolResult {
                success: false,
                content: String::new(),
                error: Some(format!("Tool '{}' not found", name)),
            }),
        }
    }

    pub async fn to_llm_tools(&self) -> Vec<llm::ToolDef> {
        let tools = self.list().await;
        tools
            .into_iter()
            .map(|t| {
                llm::ToolDef::new(t.name(), t.description()).with_parameters(t.parameters_schema())
            })
            .collect()
    }

    pub async fn list_tool_names(&self) -> Vec<String> {
        self.tools.read().await.keys().cloned().collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
