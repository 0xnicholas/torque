use async_trait::async_trait;
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
    async fn execute(
        &self,
        instance_id: Uuid,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> anyhow::Result<crate::tools::ToolExecutionResult>;
}
