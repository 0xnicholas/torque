use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use llm::{OpenAiClient, ChatRequest, Message, LlmClient};
use context_store::ContextStore;
use tool_executor::{ToolExecutor, ToolRegistry};
use checkpointer::{Checkpointer, CheckpointId, CheckpointState, CheckpointMeta};
use crate::error::AgentError;
use uuid::Uuid;

pub struct AgentRuntime {
    llm: Arc<OpenAiClient>,
    context_store: Arc<dyn ContextStore>,
    tool_executor: Arc<ToolExecutor>,
    checkpointer: Arc<dyn Checkpointer>,
    checkpoint_interval_secs: u64,
    tool_call_count: AtomicU32,
    run_id: Option<Uuid>,
    node_id: Option<Uuid>,
}

impl AgentRuntime {
    pub fn new(
        llm: OpenAiClient,
        context_store: Arc<dyn ContextStore>,
        tool_registry: Arc<ToolRegistry>,
        checkpointer: Arc<dyn Checkpointer>,
        checkpoint_interval_secs: u64,
    ) -> Self {
        Self {
            llm: Arc::new(llm),
            context_store,
            tool_executor: Arc::new(ToolExecutor::new(tool_registry)),
            checkpointer,
            checkpoint_interval_secs,
            tool_call_count: AtomicU32::new(0),
            run_id: None,
            node_id: None,
        }
    }

    pub fn with_node_context(mut self, run_id: Uuid, node_id: Uuid) -> Self {
        self.run_id = Some(run_id);
        self.node_id = Some(node_id);
        self
    }
    
    pub fn should_checkpoint(&self) -> bool {
        const TOOL_CALLS_PER_CHECKPOINT: u32 = 5;
        let count = self.tool_call_count.load(Ordering::SeqCst);
        count > 0 && count % TOOL_CALLS_PER_CHECKPOINT == 0
    }

    pub async fn create_checkpoint(&self, state: CheckpointState) -> Result<CheckpointId, AgentError> {
        let run_id = self.run_id.ok_or_else(|| AgentError::Context("run_id not set".to_string()))?;
        let node_id = self.node_id.ok_or_else(|| AgentError::Context("node_id not set".to_string()))?;
        
        self.checkpointer
            .save(run_id, node_id, state)
            .await
            .map_err(|e| AgentError::Context(e.to_string()))
    }

    pub async fn restore_from_checkpoint(&self, checkpoint_id: CheckpointId) -> Result<CheckpointState, AgentError> {
        self.checkpointer
            .load(checkpoint_id)
            .await
            .map_err(|e| AgentError::Context(e.to_string()))
    }

    pub async fn list_node_checkpoints(&self, node_id: Uuid) -> Result<Vec<CheckpointMeta>, AgentError> {
        self.checkpointer
            .list_node_checkpoints(node_id)
            .await
            .map_err(|e| AgentError::Context(e.to_string()))
    }

    pub async fn delete_checkpoint(&self, checkpoint_id: CheckpointId) -> Result<(), AgentError> {
        self.checkpointer
            .delete(checkpoint_id)
            .await
            .map_err(|e| AgentError::Context(e.to_string()))
    }

    pub fn track_tool_call(&self) {
        self.tool_call_count.fetch_add(1, Ordering::SeqCst);
    }

    pub fn reset_tool_call_count(&self) {
        self.tool_call_count.store(0, Ordering::SeqCst);
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