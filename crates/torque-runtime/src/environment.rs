use async_trait::async_trait;
use crate::checkpoint::{HydrationState, RuntimeCheckpointPayload, RuntimeCheckpointRef};
use crate::events::ModelTurnResult;
use crate::message::RuntimeMessage;
use crate::tools::{RuntimeToolDef, RuntimeToolResult};
use torque_kernel::{AgentInstanceId, ApprovalRequestId, ExecutionResult};
use uuid::Uuid;

/// Persists execution results and checkpoint-creation events.
///
/// Implementations record durable audit events. The runtime host calls
/// `record_execution_result` after each kernel step and
/// `record_checkpoint_created` when a checkpoint is saved.
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

/// Persists checkpoint state snapshots.
///
/// Called by the runtime host after building a checkpoint from kernel
/// state. Implementations store the payload and return a reference
/// that the host uses to emit checkpoint events.
#[async_trait]
pub trait RuntimeCheckpointSink: Send + Sync {
    async fn save(
        &self,
        checkpoint: RuntimeCheckpointPayload,
    ) -> anyhow::Result<RuntimeCheckpointRef>;
}

/// Executes named tools on behalf of an agent instance.
///
/// `execute` receives the tool name, JSON arguments, and execution context.
/// `tool_defs` returns the set of available tool definitions for the model.
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

/// Drives a single LLM conversation turn.
///
/// Takes the current message history, available tool definitions, and an
/// optional output sink for streaming. Returns the model's response with
/// any tool-call intents.
#[async_trait]
pub trait RuntimeModelDriver: Send + Sync {
    async fn run_turn(
        &self,
        messages: Vec<RuntimeMessage>,
        tools: Vec<RuntimeToolDef>,
        sink: Option<&dyn RuntimeOutputSink>,
    ) -> anyhow::Result<ModelTurnResult>;
}

/// Loads persisted state to rehydrate an agent instance.
///
/// Called during instance recovery. Returns `None` if no state exists
/// for the given instance (fresh start).
#[async_trait]
pub trait RuntimeHydrationSource: Send + Sync {
    async fn load_instance_state(
        &self,
        instance_id: AgentInstanceId,
    ) -> anyhow::Result<Option<HydrationState>>;
}

/// Streaming output callbacks for real-time client feedback.
///
/// Unlike the other ports, this is synchronous: the host fires callbacks
/// during execution without awaiting. Implementations translate these
/// into transport-specific events (SSE, WebSocket, stdout).
pub trait RuntimeOutputSink: Send + Sync {
    fn on_text_chunk(&self, chunk: &str);
    fn on_tool_call(&self, tool_name: &str, arguments: &serde_json::Value);
    fn on_tool_result(&self, tool_name: &str, result: &RuntimeToolResult);
    fn on_checkpoint(&self, checkpoint_id: Uuid, reason: &str);
}

/// Notifies an external system when an approval is required.
///
/// Called when the kernel transitions an instance to an await-approval
/// state. Implementations surface the request to an operator or
/// automated approval policy.
///
/// Note: this port is defined but not yet wired into RuntimeHost.
#[async_trait]
pub trait ApprovalGateway: Send + Sync {
    async fn notify_approval_required(
        &self,
        context: &RuntimeExecutionContext,
        approval_request_id: ApprovalRequestId,
    ) -> anyhow::Result<()>;
}

/// Identifies the calling context for a tool execution.
///
/// Includes the agent instance ID and optionally the originating
/// request and parent task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeExecutionContext {
    pub instance_id: Uuid,
    pub request_id: Option<Uuid>,
    pub source_task_id: Option<Uuid>,
}
