use crate::agent::stream::StreamEvent;
use crate::infra::llm::{Chunk, LlmClient, LlmMessage};
use crate::infra::tool_registry::ToolRegistry;
use crate::kernel_bridge::events::EventRecorder;
use crate::repository::{CheckpointRepository, EventRepository, SessionRepository};
use checkpointer::Checkpointer;
use llm::{ChatRequest, FinishReason, ToolCall};
use std::sync::Arc;
use tokio::sync::mpsc;
use torque_kernel::{
    AgentDefinition, AgentInstanceId, ExecutionRequest, ExecutionResult, InMemoryKernelRuntime,
    KernelError, KernelRuntime, StepDecision,
};

pub const MAX_TOOL_CALLS: usize = 20;
pub const MAX_CONSECUTIVE_TOOL_FAILURES: usize = 3;

#[derive(Debug, thiserror::Error)]
pub enum KernelBridgeError {
    #[error("kernel error: {0}")]
    Kernel(#[from] KernelError),
    #[error("db error: {0}")]
    Db(#[from] anyhow::Error),
    #[error("checkpoint error: {0}")]
    Checkpoint(String),
    #[error("no checkpoint for instance {0:?}")]
    NoCheckpoint(AgentInstanceId),
    #[error("checkpoint not found")]
    CheckpointNotFound,
}

pub struct KernelRuntimeHandle {
    runtime: InMemoryKernelRuntime,
    event_repo: Arc<dyn EventRepository>,
    #[allow(dead_code)]
    checkpoint_repo: Arc<dyn CheckpointRepository>,
    checkpointer: Arc<dyn Checkpointer>,
}

impl KernelRuntimeHandle {
    pub fn new(
        agent_definitions: Vec<AgentDefinition>,
        event_repo: Arc<dyn EventRepository>,
        checkpoint_repo: Arc<dyn CheckpointRepository>,
        checkpointer: Arc<dyn Checkpointer>,
    ) -> Self {
        Self {
            runtime: InMemoryKernelRuntime::new(agent_definitions),
            event_repo,
            checkpoint_repo,
            checkpointer,
        }
    }

    pub async fn hydrate_runtime(
        &mut self,
        instance_id: AgentInstanceId,
        session_repo: &dyn SessionRepository,
    ) -> Result<(), KernelBridgeError> {
        let _state = session_repo.get_kernel_state(instance_id.as_uuid()).await?;
        Ok(())
    }

    pub async fn execute_chat(
        &mut self,
        request: ExecutionRequest,
        llm: Arc<dyn LlmClient>,
        tools: Arc<ToolRegistry>,
        event_sink: mpsc::Sender<StreamEvent>,
        llm_messages: Vec<LlmMessage>,
    ) -> Result<ExecutionResult, KernelBridgeError> {
        // 1. Start kernel instance/task
        let result = self.runtime.handle(request, StepDecision::Continue)?;
        self.record_events(&result).await?;

        let instance_id = result.instance_id;
        let _ = event_sink
            .send(StreamEvent::Start {
                session_id: instance_id.as_uuid(),
            })
            .await;

        let final_content = self
            .run_llm_conversation(llm, tools, event_sink.clone(), llm_messages)
            .await?;

        let complete_request = self.reconstruct_request(instance_id)?;
        let result = self
            .runtime
            .handle(complete_request, StepDecision::CompleteTask(final_content.clone()))?;
        self.record_events(&result).await?;

        self.create_checkpoint(instance_id).await?;

        let mut result = result;
        result.summary = Some(final_content);

        Ok(result)
    }

    async fn run_llm_conversation(
        &mut self,
        llm: Arc<dyn LlmClient>,
        tools: Arc<ToolRegistry>,
        event_sink: mpsc::Sender<StreamEvent>,
        mut messages: Vec<LlmMessage>,
    ) -> Result<String, KernelBridgeError> {
        let tool_defs = tools.to_llm_tools().await;
        let mut tool_call_count = 0;
        let mut consecutive_failures = 0;

        loop {
            if tool_call_count >= MAX_TOOL_CALLS {
                return Err(KernelBridgeError::Db(anyhow::anyhow!(
                    "Maximum tool call limit reached"
                )));
            }

            let request = ChatRequest::new(llm.model().to_string(), messages.clone())
                .with_tools(tool_defs.clone());

            let content_buffer = Arc::new(std::sync::Mutex::new(String::new()));
            let tool_calls_buffer: Arc<std::sync::Mutex<Vec<ToolCall>>> =
                Arc::new(std::sync::Mutex::new(Vec::new()));
            let content_buffer_clone = content_buffer.clone();
            let tool_calls_buffer_clone = tool_calls_buffer.clone();
            let tx_clone = event_sink.clone();

            let callback = Box::new(move |chunk: Chunk| {
                if let Some(tool_call) = &chunk.tool_call {
                    let mut calls = tool_calls_buffer_clone.lock().unwrap();
                    if !calls.iter().any(|t| t.id == tool_call.id) {
                        calls.push(tool_call.clone());
                    }
                }
                if !chunk.content.is_empty() {
                    content_buffer_clone.lock().unwrap().push_str(&chunk.content);
                    let _ = tx_clone.try_send(StreamEvent::Chunk {
                        content: chunk.content,
                    });
                }
            });

            let response = llm.chat_streaming(request, callback).await.map_err(|e| {
                KernelBridgeError::Db(anyhow::anyhow!("LLM streaming error: {e}"))
            })?;

            let content = Arc::try_unwrap(content_buffer)
                .map(|m| m.into_inner().unwrap_or_default())
                .unwrap_or_default();
            let tool_calls = Arc::try_unwrap(tool_calls_buffer)
                .map(|m| m.into_inner().unwrap_or_default())
                .unwrap_or_default();

            match response.finish_reason {
                FinishReason::ToolCalls => {
                    tool_call_count += 1;

                    for tool_call in &tool_calls {
                        let _ = event_sink
                            .send(StreamEvent::ToolCall {
                                name: tool_call.name.clone(),
                                arguments: tool_call.arguments.clone(),
                            })
                            .await;

                        let result = tools
                            .execute(&tool_call.name, tool_call.arguments.clone())
                            .await;

                        let result = match result {
                            Ok(r) => r,
                            Err(e) => crate::tools::ToolResult {
                                success: false,
                                content: String::new(),
                                error: Some(e.to_string()),
                            },
                        };

                        let _ = event_sink
                            .send(StreamEvent::ToolResult {
                                name: tool_call.name.clone(),
                                success: result.success,
                                content: result.content.clone(),
                                error: result.error.clone(),
                            })
                            .await;

                        if result.success {
                            consecutive_failures = 0;
                        } else {
                            consecutive_failures += 1;
                            if consecutive_failures >= MAX_CONSECUTIVE_TOOL_FAILURES {
                                return Err(KernelBridgeError::Db(anyhow::anyhow!(
                                    "Tool execution failed {} times consecutively",
                                    consecutive_failures
                                )));
                            }
                        }

                        messages.push(LlmMessage::user(format!(
                            "Tool '{}' result: {}",
                            tool_call.name, result.content
                        )));
                    }
                }
                _ => {
                    return Ok(content);
                }
            }
        }
    }

    fn reconstruct_request(
        &self,
        instance_id: AgentInstanceId,
    ) -> Result<ExecutionRequest, KernelBridgeError> {
        let instance = self
            .runtime
            .instance(instance_id)
            .ok_or_else(|| {
                KernelBridgeError::Kernel(
                    torque_kernel::ValidationError::new("Runtime", "instance missing").into(),
                )
            })?;
        let agent_def = self
            .runtime
            .store()
            .agent_definition(instance.agent_definition_id())
            .ok_or_else(|| {
                KernelBridgeError::Kernel(
                    torque_kernel::ValidationError::new("Runtime", "agent definition missing")
                        .into(),
                )
            })?;
        Ok(torque_kernel::ExecutionRequest::new(
            agent_def.id,
            "continue".to_string(),
            vec![],
        ))
    }

    async fn record_events(
        &self,
        result: &ExecutionResult,
    ) -> Result<(), KernelBridgeError> {
        let db_events = EventRecorder::to_db_events(result, result.sequence_number);
        for event in db_events {
            self.event_repo.create(event).await?;
        }
        Ok(())
    }

    async fn create_checkpoint(
        &mut self,
        instance_id: AgentInstanceId,
    ) -> Result<(), KernelBridgeError> {
        let checkpoint = self.runtime.create_checkpoint(instance_id)?;

        let run_id = checkpoint.instance_id.as_uuid();
        let node_id = checkpoint
            .active_task_id
            .map(|id| id.as_uuid())
            .unwrap_or(run_id);

        let state = checkpointer::CheckpointState {
            data: serde_json::json!({
                "instance_state": format!("{:?}", checkpoint.instance_state),
                "active_task_state": checkpoint.active_task_state.map(|s| format!("{:?}", s)),
                "pending_approval_ids": checkpoint.pending_approval_ids.iter().map(|id| id.as_uuid()).collect::<Vec<_>>(),
                "child_delegation_ids": checkpoint.child_delegation_ids.iter().map(|id| id.as_uuid()).collect::<Vec<_>>(),
                "event_sequence": checkpoint.event_sequence,
            }),
        };

        self.checkpointer
            .save(run_id, node_id, state)
            .await
            .map_err(|e| KernelBridgeError::Checkpoint(e.to_string()))?;

        Ok(())
    }
}
