use async_trait::async_trait;
use crate::checkpoint::{HydrationState, RuntimeCheckpointPayload, RuntimeCheckpointRef};
use crate::events::ModelTurnResult;
use crate::message::RuntimeMessage;
use crate::tools::{RuntimeToolDef, RuntimeToolResult};
use torque_kernel::{AgentInstanceId, ApprovalRequestId, ExecutionResult};
use uuid::Uuid;

#[async_trait]
pub trait RuntimeEventSink: Send + Sync {
    async fn record_execution_result(&self, result: &ExecutionResult) -> anyhow::Result<()>;

    async fn record_checkpoint_created(
        &self,
        checkpoint_id: Uuid,
        instance_id: AgentInstanceId,
        reason: &str,
    ) -> anyhow::Result<()>;
}

#[async_trait]
pub trait RuntimeCheckpointSink: Send + Sync {
    async fn save(
        &self,
        checkpoint: RuntimeCheckpointPayload,
    ) -> anyhow::Result<RuntimeCheckpointRef>;
}

#[async_trait]
pub trait RuntimeToolExecutor: Send + Sync {
    async fn execute(
        &self,
        ctx: RuntimeExecutionContext,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> anyhow::Result<RuntimeToolResult>;

    async fn tool_defs(&self) -> anyhow::Result<Vec<RuntimeToolDef>>;
}

#[async_trait]
pub trait RuntimeModelDriver: Send + Sync {
    async fn run_turn(
        &self,
        messages: Vec<RuntimeMessage>,
        tools: Vec<RuntimeToolDef>,
        sink: Option<&dyn RuntimeOutputSink>,
    ) -> anyhow::Result<ModelTurnResult>;
}

#[async_trait]
pub trait RuntimeHydrationSource: Send + Sync {
    async fn load_instance_state(
        &self,
        instance_id: AgentInstanceId,
    ) -> anyhow::Result<Option<HydrationState>>;
}

pub trait RuntimeOutputSink: Send + Sync {
    fn on_text_chunk(&self, chunk: &str);
    fn on_tool_call(&self, tool_name: &str, arguments: &serde_json::Value);
    fn on_tool_result(&self, tool_name: &str, result: &RuntimeToolResult);
    fn on_checkpoint(&self, checkpoint_id: Uuid, reason: &str);
}

#[async_trait]
pub trait ApprovalGateway: Send + Sync {
    async fn notify_approval_required(
        &self,
        context: &RuntimeExecutionContext,
        approval_request_id: ApprovalRequestId,
    ) -> anyhow::Result<()>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeExecutionContext {
    pub instance_id: Uuid,
    pub request_id: Option<Uuid>,
    pub source_task_id: Option<Uuid>,
}
