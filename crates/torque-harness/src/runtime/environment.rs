use crate::agent::stream::StreamEvent;
use crate::infra::llm::LlmMessage;
use crate::tools::ToolResult;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::mpsc;
use torque_kernel::{AgentInstanceId, ExecutionResult};
use uuid::Uuid;

#[async_trait]
pub trait RuntimeEventSink: Send + Sync {
    async fn record_execution_result(&self, result: &ExecutionResult) -> anyhow::Result<()>;
}

#[async_trait]
pub trait RuntimeCheckpointSink: Send + Sync {
    async fn create_checkpoint(
        &self,
        instance_id: AgentInstanceId,
        reason: &str,
    ) -> anyhow::Result<Uuid>;
}

#[async_trait]
pub trait RuntimeToolExecutor: Send + Sync {
    async fn execute_tool(
        &self,
        instance_id: Uuid,
        name: &str,
        arguments: serde_json::Value,
    ) -> anyhow::Result<ToolResult>;
}

#[async_trait]
pub trait RuntimeConversationDriver: Send + Sync {
    async fn run_turn(
        &self,
        messages: Vec<LlmMessage>,
        event_sink: mpsc::Sender<StreamEvent>,
    ) -> anyhow::Result<Vec<LlmMessage>>;
}

pub type SharedEventSink = Arc<dyn RuntimeEventSink>;
