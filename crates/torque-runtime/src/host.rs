use crate::checkpoint::{RuntimeCheckpointPayload, RuntimeCheckpointRef};
use crate::environment::{
    RuntimeCheckpointSink, RuntimeExecutionContext, RuntimeHydrationSource, RuntimeModelDriver,
    RuntimeOutputSink, RuntimeToolExecutor,
};
use crate::events::RuntimeFinishReason;
use crate::message::RuntimeMessage;
use std::sync::Arc;
use torque_kernel::{
    AgentDefinition, AgentInstanceId, ExecutionRequest, ExecutionResult, InMemoryKernelRuntime,
    KernelError, KernelRuntime, StepDecision,
};
pub const MAX_TOOL_CALLS: usize = 20;
pub const MAX_CONSECUTIVE_TOOL_FAILURES: usize = 3;

#[derive(Debug, thiserror::Error)]
pub enum RuntimeHostError {
    #[error("kernel error: {0}")]
    Kernel(#[from] KernelError),
    #[error("runtime error: {0}")]
    Runtime(#[from] anyhow::Error),
}

#[derive(Debug, Clone)]
pub struct RuntimeCheckpointPolicy {
    pub checkpoint_on_awaiting_llm: bool,
    pub checkpoint_on_task_complete: bool,
}

impl Default for RuntimeCheckpointPolicy {
    fn default() -> Self {
        Self {
            checkpoint_on_awaiting_llm: true,
            checkpoint_on_task_complete: true,
        }
    }
}

pub struct RuntimeHost {
    runtime: InMemoryKernelRuntime,
    event_sink: Arc<dyn crate::environment::RuntimeEventSink>,
    checkpoint_sink: Arc<dyn RuntimeCheckpointSink>,
    hydration_source: Option<Arc<dyn RuntimeHydrationSource>>,
    checkpoint_policy: RuntimeCheckpointPolicy,
}

impl RuntimeHost {
    pub fn new(
        agent_definitions: Vec<AgentDefinition>,
        event_sink: Arc<dyn crate::environment::RuntimeEventSink>,
        checkpoint_sink: Arc<dyn RuntimeCheckpointSink>,
    ) -> Self {
        Self {
            runtime: InMemoryKernelRuntime::new(agent_definitions),
            event_sink,
            checkpoint_sink,
            hydration_source: None,
            checkpoint_policy: RuntimeCheckpointPolicy::default(),
        }
    }

    pub fn with_hydration_source(
        mut self,
        hydration_source: Arc<dyn RuntimeHydrationSource>,
    ) -> Self {
        self.hydration_source = Some(hydration_source);
        self
    }

    pub fn with_checkpoint_policy(mut self, checkpoint_policy: RuntimeCheckpointPolicy) -> Self {
        self.checkpoint_policy = checkpoint_policy;
        self
    }

    pub async fn hydrate_runtime(
        &mut self,
        instance_id: AgentInstanceId,
    ) -> Result<(), RuntimeHostError> {
        if let Some(source) = &self.hydration_source {
            let _ = source.load_instance_state(instance_id).await?;
        }
        Ok(())
    }

    pub async fn execute_v1(
        &mut self,
        request: ExecutionRequest,
        model_driver: &dyn RuntimeModelDriver,
        tool_executor: &dyn RuntimeToolExecutor,
        output_sink: Option<&dyn RuntimeOutputSink>,
        initial_messages: Vec<RuntimeMessage>,
    ) -> Result<ExecutionResult, RuntimeHostError> {
        let result = self.runtime.handle(request, StepDecision::Continue)?;
        self.record_events(&result).await?;

        let instance_id = result.instance_id;
        let final_content = self
            .run_llm_conversation(
                instance_id,
                model_driver,
                tool_executor,
                output_sink,
                initial_messages,
            )
            .await?;

        let complete_request = self.reconstruct_request(instance_id)?;
        let result = self.runtime.handle(
            complete_request,
            StepDecision::CompleteTask(final_content.clone()),
        )?;
        self.record_events(&result).await?;

        if self.checkpoint_policy.checkpoint_on_task_complete {
            let checkpoint = self.create_checkpoint(instance_id, "task_complete").await?;
            self.record_checkpoint_event(&checkpoint, instance_id, "task_complete")
                .await?;
            if let Some(output_sink) = output_sink {
                output_sink.on_checkpoint(checkpoint.checkpoint_id, "task_complete");
            }
        }

        let mut result = result;
        result.summary = Some(final_content);

        Ok(result)
    }

    pub async fn execute_chat(
        &mut self,
        request: ExecutionRequest,
        model_driver: &dyn RuntimeModelDriver,
        tool_executor: &dyn RuntimeToolExecutor,
        output_sink: Option<&dyn RuntimeOutputSink>,
        messages: Vec<RuntimeMessage>,
    ) -> Result<ExecutionResult, RuntimeHostError> {
        self.execute_v1(request, model_driver, tool_executor, output_sink, messages)
            .await
    }

    async fn run_llm_conversation(
        &mut self,
        instance_id: AgentInstanceId,
        model_driver: &dyn RuntimeModelDriver,
        tool_executor: &dyn RuntimeToolExecutor,
        output_sink: Option<&dyn RuntimeOutputSink>,
        mut messages: Vec<RuntimeMessage>,
    ) -> Result<String, RuntimeHostError> {
        let tool_defs = tool_executor.tool_defs().await?;
        let mut tool_call_count = 0;
        let mut consecutive_failures = 0;

        loop {
            if tool_call_count >= MAX_TOOL_CALLS {
                return Err(RuntimeHostError::Runtime(anyhow::anyhow!(
                    "Maximum tool call limit reached"
                )));
            }

            let turn = model_driver
                .run_turn(messages.clone(), tool_defs.clone(), output_sink)
                .await?;

            match turn.finish_reason {
                RuntimeFinishReason::ToolCalls => {
                    tool_call_count += 1;
                    for tool_call in turn.tool_calls {
                        let result = tool_executor
                            .execute(
                                RuntimeExecutionContext {
                                    instance_id: instance_id.as_uuid(),
                                    request_id: None,
                                    source_task_id: None,
                                },
                                &tool_call.name,
                                tool_call.arguments.clone(),
                            )
                            .await?;

                        if let Some(output_sink) = output_sink {
                            output_sink.on_tool_result(&tool_call.name, &result);
                        }

                        if result.success {
                            consecutive_failures = 0;
                        } else {
                            consecutive_failures += 1;
                            if consecutive_failures >= MAX_CONSECUTIVE_TOOL_FAILURES {
                                return Err(RuntimeHostError::Runtime(anyhow::anyhow!(
                                    "Tool execution failed {} times consecutively",
                                    consecutive_failures
                                )));
                            }
                        }

                        messages.push(RuntimeMessage::user(format!(
                            "Tool '{}' result: {}",
                            tool_call.name, result.content
                        )));
                    }

                    if self.checkpoint_policy.checkpoint_on_awaiting_llm {
                        let checkpoint = self.create_checkpoint(instance_id, "awaiting_llm").await?;
                        self.record_checkpoint_event(&checkpoint, instance_id, "awaiting_llm")
                            .await?;
                        if let Some(output_sink) = output_sink {
                            output_sink.on_checkpoint(checkpoint.checkpoint_id, "awaiting_llm");
                        }
                    }
                }
                _ => {
                    return Ok(turn.assistant_text);
                }
            }
        }
    }

    fn reconstruct_request(
        &self,
        instance_id: AgentInstanceId,
    ) -> Result<ExecutionRequest, RuntimeHostError> {
        let instance = self.runtime.instance(instance_id).ok_or_else(|| {
            RuntimeHostError::Kernel(
                torque_kernel::ValidationError::new("Runtime", "instance missing").into(),
            )
        })?;
        let agent_def = self
            .runtime
            .store()
            .agent_definition(instance.agent_definition_id())
            .ok_or_else(|| {
                RuntimeHostError::Kernel(
                    torque_kernel::ValidationError::new("Runtime", "agent definition missing")
                        .into(),
                )
            })?;
        Ok(ExecutionRequest::new(
            agent_def.id,
            "continue".to_string(),
            vec![],
        ))
    }

    async fn record_events(&self, result: &ExecutionResult) -> Result<(), RuntimeHostError> {
        self.event_sink.record_execution_result(result).await?;
        Ok(())
    }

    async fn record_checkpoint_event(
        &self,
        checkpoint: &RuntimeCheckpointRef,
        instance_id: AgentInstanceId,
        reason: &str,
    ) -> Result<(), RuntimeHostError> {
        self.event_sink
            .record_checkpoint_created(checkpoint.checkpoint_id, instance_id, reason)
            .await?;
        Ok(())
    }

    async fn create_checkpoint(
        &mut self,
        instance_id: AgentInstanceId,
        reason: &str,
    ) -> Result<RuntimeCheckpointRef, RuntimeHostError> {
        let checkpoint = self.runtime.create_checkpoint(instance_id)?;
        let run_id = checkpoint.instance_id.as_uuid();
        let node_id = checkpoint.active_task_id.map(|id| id.as_uuid()).unwrap_or(run_id);

        let state = checkpointer::CheckpointState {
            messages: vec![],
            tool_call_count: 0,
            intermediate_results: vec![],
            custom_state: Some(serde_json::json!({
                "instance_state": format!("{:?}", checkpoint.instance_state),
                "checkpoint_reason": reason,
                "active_task_state": checkpoint.active_task_state.map(|s| format!("{:?}", s)),
                "pending_approval_ids": checkpoint.pending_approval_ids.iter().map(|id| id.as_uuid()).collect::<Vec<_>>(),
                "child_delegation_ids": checkpoint.child_delegation_ids.iter().map(|id| id.as_uuid()).collect::<Vec<_>>(),
                "event_sequence": checkpoint.event_sequence,
            })),
        };

        Ok(self
            .checkpoint_sink
            .save(RuntimeCheckpointPayload {
                instance_id,
                node_id,
                reason: reason.to_string(),
                state,
            })
            .await?)
    }
}
