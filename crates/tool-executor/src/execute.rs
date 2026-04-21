use std::sync::Arc;
use crate::error::ToolError;
use crate::registry::{ToolRegistry, ToolCall, ToolResult};

pub struct ToolExecutor {
    registry: Arc<ToolRegistry>,
}

impl ToolExecutor {
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        Self { registry }
    }
    
    pub async fn execute(
        &self,
        agent_tools: &[String],
        call: ToolCall,
        allowed_tools: Option<&[String]>,
    ) -> Result<ToolResult, ToolError> {
        crate::permission::PermissionChecker::check(agent_tools, &call.name, allowed_tools)
            .map_err(|e| ToolError::PermissionDenied(e.to_string()))?;
        
        let result = self.registry.execute(call).await
            .map_err(|e| ToolError::Execution(e))?;
        
        Ok(result)
    }
}