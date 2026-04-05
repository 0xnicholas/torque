use std::sync::Arc;
use llm::{OpenAiClient, ChatRequest, Message, LlmClient};
use context_store::ContextStore;
use tool_executor::{ToolExecutor, ToolRegistry};
use crate::error::AgentError;

pub struct AgentRuntime {
    llm: Arc<OpenAiClient>,
    context_store: Arc<dyn ContextStore>,
    tool_executor: Arc<ToolExecutor>,
}

impl AgentRuntime {
    pub fn new(
        llm: OpenAiClient,
        context_store: Arc<dyn ContextStore>,
        tool_registry: Arc<ToolRegistry>,
    ) -> Self {
        Self {
            llm: Arc::new(llm),
            context_store,
            tool_executor: Arc::new(ToolExecutor::new(tool_registry)),
        }
    }
    
    pub async fn execute(&self, instruction: &str) -> Result<String, AgentError> {
        let request = ChatRequest::new("gpt-4", vec![
            Message::user(instruction.to_string()),
        ]);
        
        let response = self.llm.chat(request)
            .await
            .map_err(|e| AgentError::Llm(e.to_string()))?;
        
        let output = &response.message.content;
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(output) {
            if let Some(o) = parsed.get("output") {
                return Ok(o.to_string());
            }
        }
        Ok(output.clone())
    }
}