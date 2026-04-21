use std::collections::HashMap;
use std::pin::Pin;
use std::future::Future;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub output: String,
    pub error: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

pub type BoxedToolHandler = Box<dyn Fn(serde_json::Value) -> Pin<Box<dyn Future<Output = Result<ToolResult, String>> + Send>> + Send + Sync>;

pub struct ToolRegistry {
    tools: HashMap<String, BoxedToolHandler>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: HashMap::new() }
    }
    
    pub fn register<F, Fut>(&mut self, name: &str, handler: F)
    where
        F: Fn(serde_json::Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<ToolResult, String>> + Send + 'static,
    {
        self.tools.insert(name.to_string(), Box::new(move |args: serde_json::Value| {
            Box::pin(handler(args)) as Pin<Box<dyn Future<Output = Result<ToolResult, String>> + Send>>
        }));
    }
    
    pub async fn execute(&self, call: ToolCall) -> Result<ToolResult, String> {
        let handler = self.tools
            .get(&call.name)
            .ok_or_else(|| format!("Tool not found: {}", call.name))?;
        
        handler(call.arguments).await
    }
    
    pub fn get_tool_names(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}