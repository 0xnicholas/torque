use crate::tools::{ToolArc, ToolResult};
use serde_json::Value;
use std::collections::HashMap;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, Clone, Default)]
pub struct ToolExecutionContext {
    pub source_instance_id: Option<Uuid>,
}

tokio::task_local! {
    static CURRENT_TOOL_EXECUTION_CONTEXT: ToolExecutionContext;
}

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

    /// Remove a tool by name from the registry.
    /// Returns `true` if the tool was found and removed, `false` otherwise.
    pub async fn remove(&self, name: &str) -> bool {
        self.tools.write().await.remove(name).is_some()
    }

    /// Update an existing tool in-place.
    /// Returns `true` if the tool was found and updated, `false` if no tool
    /// with the given name exists (in which case the registry is unchanged).
    pub async fn update(&self, name: &str, tool: ToolArc) -> bool {
        let mut guard = self.tools.write().await;
        if guard.contains_key(name) {
            guard.insert(name.to_string(), tool);
            true
        } else {
            false
        }
    }

    pub async fn get(&self, name: &str) -> Option<ToolArc> {
        self.tools.read().await.get(name).cloned()
    }

    pub async fn list(&self) -> Vec<ToolArc> {
        self.tools.read().await.values().cloned().collect()
    }

    pub async fn execute(&self, name: &str, args: Value) -> anyhow::Result<ToolResult> {
        self.execute_with_context(name, args, ToolExecutionContext::default())
            .await
    }

    pub async fn execute_with_context(
        &self,
        name: &str,
        args: Value,
        context: ToolExecutionContext,
    ) -> anyhow::Result<ToolResult> {
        match self.get(name).await {
            Some(tool) => {
                CURRENT_TOOL_EXECUTION_CONTEXT
                    .scope(context, tool.execute(args))
                    .await
            }
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

pub fn current_tool_execution_context() -> Option<ToolExecutionContext> {
    CURRENT_TOOL_EXECUTION_CONTEXT.try_with(Clone::clone).ok()
}
