use crate::checkpoint::{RuntimeCheckpointPayload, RuntimeCheckpointRef};
use crate::context::{ContextCompactionPolicy, ContextCompactionService};
use crate::environment::{
    ApprovalGateway, RuntimeCheckpointSink, RuntimeExecutionContext, RuntimeHydrationSource,
    RuntimeModelDriver, RuntimeOutputSink, RuntimeToolExecutor,
};
use crate::events::RuntimeFinishReason;
use crate::message::{RuntimeMessage, RuntimeMessageRole, StructuredMessage};
use crate::offload::ToolOffloadPolicy;
use crate::message_queue::{InMemoryMessageQueue, MessageQueue};
use llm::Message as LlmMessage;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use torque_kernel::{
    AgentDefinition, AgentInstanceId, ExecutionOutcome, ExecutionRequest, ExecutionResult,
    InMemoryKernelRuntime, KernelError, KernelRuntime, StepDecision,
};

/// Maximum consecutive tool failures before aborting execution.
pub const MAX_CONSECUTIVE_TOOL_FAILURES: usize = 3;

/// Accumulated token usage during an agent loop execution.
#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeHostError {
    #[error("kernel error: {0}")]
    Kernel(#[from] KernelError),
    #[error("runtime error: {0}")]
    Runtime(#[from] anyhow::Error),
}

/// Controls when the runtime host creates checkpoints during execution.
#[derive(Debug, Clone)]
pub struct RuntimeCheckpointPolicy {
    /// Create a checkpoint after each LLM turn that returns tool calls.
    pub checkpoint_on_awaiting_llm: bool,
    /// Create a checkpoint after task completion.
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

/// Production execution host that orchestrates kernel steps with
/// model turns and tool calls.
///
/// Wraps an [`InMemoryKernelRuntime`] and delegates I/O to pluggable
/// port implementations ([`RuntimeModelDriver`], [`RuntimeToolExecutor`],
/// [`RuntimeOutputSink`]). Constructed with at least an
/// [`RuntimeEventSink`] and [`RuntimeCheckpointSink`]; hydration source
/// and checkpoint policy are optional builder methods.
pub struct RuntimeHost {
    runtime: InMemoryKernelRuntime,
    event_sink: Arc<dyn crate::environment::RuntimeEventSink>,
    checkpoint_sink: Arc<dyn RuntimeCheckpointSink>,
    hydration_source: Option<Arc<dyn RuntimeHydrationSource>>,
    checkpoint_policy: RuntimeCheckpointPolicy,
    approval_gateway: Option<Arc<dyn ApprovalGateway>>,
    offload_policy: Option<Arc<ToolOffloadPolicy>>,
    compaction_service: ContextCompactionService,
    cancel_signal: Option<Arc<AtomicBool>>,
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
            approval_gateway: None,
            offload_policy: None,
            compaction_service: ContextCompactionService::new(ContextCompactionPolicy::default()),
            cancel_signal: None,
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

    pub fn with_approval_gateway(
        mut self,
        approval_gateway: Arc<dyn ApprovalGateway>,
    ) -> Self {
        self.approval_gateway = Some(approval_gateway);
        self
    }

    pub fn with_offload_policy(mut self, offload_policy: Arc<ToolOffloadPolicy>) -> Self {
        self.offload_policy = Some(offload_policy);
        self
    }

    pub fn with_compaction_policy(mut self, policy: ContextCompactionPolicy) -> Self {
        self.compaction_service = ContextCompactionService::new(policy);
        self
    }

    /// Attach a cancellation signal. The agent loop checks this signal
    /// at the beginning of each turn and aborts if set.
    pub fn with_cancel_signal(mut self, signal: Arc<AtomicBool>) -> Self {
        self.cancel_signal = Some(signal);
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
        let mut queue = InMemoryMessageQueue::new(
            initial_messages
                .iter()
                .map(StructuredMessage::from_runtime)
                .collect(),
        );
        self.execute_v1_with_queue(
            request,
            model_driver,
            tool_executor,
            output_sink,
            &mut queue,
        )
        .await
    }

    /// Execute with an externally-managed `MessageQueue`.  This is the
    /// preferred entry point when the caller needs to inspect or feed
    /// the queue before / after execution (e.g. for `followUp` chaining
    /// or `nextTurn` merging).
    pub async fn execute_v1_with_queue(
        &mut self,
        request: ExecutionRequest,
        model_driver: &dyn RuntimeModelDriver,
        tool_executor: &dyn RuntimeToolExecutor,
        output_sink: Option<&dyn RuntimeOutputSink>,
        queue: &mut dyn MessageQueue,
    ) -> Result<ExecutionResult, RuntimeHostError> {
        let result = self.runtime.handle(request, StepDecision::Continue)?;
        self.record_events(&result).await?;
        self.notify_approval_if_needed(&result).await;

        let instance_id = result.instance_id;

        // Merge any nextTurn messages into the active conversation
        // before the agent loop starts.
        queue.merge_next_turn();

        let final_content = self
            .run_llm_conversation(
                instance_id,
                model_driver,
                tool_executor,
                output_sink,
                queue,
            )
            .await?;

        let complete_request = self.reconstruct_request(instance_id)?;
        let result = self.runtime.handle(
            complete_request,
            StepDecision::CompleteTask(final_content.clone()),
        )?;
        self.record_events(&result).await?;
        self.notify_approval_if_needed(&result).await;

        if self.checkpoint_policy.checkpoint_on_task_complete {
            let checkpoint = self.create_checkpoint(instance_id, "task_complete", None).await?;
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
        queue: &mut dyn MessageQueue,
    ) -> Result<String, RuntimeHostError> {
        let tool_defs = tool_executor.tool_defs().await?;
        let mut turn_count: u32 = 0;
        let mut consecutive_failures = 0;
        let mut token_usage = TokenUsage::default();

        loop {
            // Check cancellation signal before each turn.
            if let Some(ref signal) = self.cancel_signal {
                if signal.load(Ordering::Relaxed) {
                    tracing::info!(instance_id = %instance_id.as_uuid(), turn = turn_count, "Execution cancelled");
                    return Err(RuntimeHostError::Runtime(anyhow::anyhow!(
                        "Execution cancelled"
                    )));
                }
            }

            turn_count += 1;
            tracing::debug!(
                instance_id = %instance_id.as_uuid(),
                turn = turn_count,
                active_messages = queue.active_messages().len(),
                "turn_start"
            );

            if let Some(output_sink) = output_sink {
                output_sink.on_turn_start(turn_count);
            }

            // Auto-compact context before model turn if threshold exceeded.
            if let Some(compacted) = queue.compact(self.compaction_service.policy()).await {
                tracing::info!(
                    instance_id = %instance_id.as_uuid(),
                    turn = turn_count,
                    "Context compacted: {} older messages summarised",
                    compacted.key_facts.len()
                );
            }

            // Convert queue messages to RuntimeMessages for model_driver (bridge).
            let llm_msgs: Vec<LlmMessage> = queue.to_llm_messages();
            let rt_msgs: Vec<RuntimeMessage> =
                llm_msgs.into_iter().map(RuntimeMessage::from).collect();

            let turn = model_driver
                .run_turn(rt_msgs, tool_defs.clone(), output_sink)
                .await?;

            // Accumulate token usage.
            token_usage.prompt_tokens += turn.prompt_tokens.unwrap_or(0);
            token_usage.completion_tokens += turn.completion_tokens.unwrap_or(0);
            token_usage.total_tokens =
                token_usage.prompt_tokens + token_usage.completion_tokens;

            // Store assistant response in message history.
            if !turn.assistant_text.is_empty() || !turn.tool_calls.is_empty() {
                let assistant_msg = if !turn.tool_calls.is_empty() {
                    StructuredMessage::assistant_with_tools(
                        turn.assistant_text.clone(),
                        turn.tool_calls.clone(),
                    )
                } else {
                    StructuredMessage::assistant(turn.assistant_text.clone())
                };
                queue.push_conversation_message(assistant_msg);
            }

            match turn.finish_reason {
                RuntimeFinishReason::ToolCalls => {
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

                        let result = if let Some(offload) = &self.offload_policy {
                            offload
                                .offload(
                                    &tool_call.name,
                                    result,
                                    Some(instance_id.as_uuid()),
                                )
                                .await?
                        } else {
                            result
                        };

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

                        let tool_msg = StructuredMessage::tool_result(
                            &tool_call.id,
                            &tool_call.name,
                            result,
                        );
                        queue.push_conversation_message(tool_msg);
                    }

                    // ★ steer injection: poll supervisor messages after tools
                    while let Some(steer_msg) = queue.poll_steer() {
                        tracing::info!(
                            instance_id = %instance_id.as_uuid(),
                            source = ?steer_msg, "steer injected"
                        );
                    }

                    if self.checkpoint_policy.checkpoint_on_awaiting_llm {
                        let rt_msgs_for_cp: Vec<RuntimeMessage> = queue
                            .active_messages()
                            .iter()
                            .cloned()
                            .map(RuntimeMessage::from)
                            .collect();
                        let checkpoint = self
                            .create_checkpoint(instance_id, "awaiting_llm", Some(&rt_msgs_for_cp))
                            .await?;
                        self.record_checkpoint_event(&checkpoint, instance_id, "awaiting_llm")
                            .await?;
                        if let Some(output_sink) = output_sink {
                            output_sink.on_checkpoint(checkpoint.checkpoint_id, "awaiting_llm");
                        }
                    }
                }
                _ => {
                    tracing::debug!(
                        instance_id = %instance_id.as_uuid(),
                        turn = turn_count,
                        total_turns = turn_count,
                        tokens = token_usage.total_tokens,
                        "agent_end"
                    );
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

    async fn notify_approval_if_needed(
        &self,
        result: &ExecutionResult,
    ) {
        if let Some(gateway) = &self.approval_gateway {
            if matches!(result.outcome, torque_kernel::ExecutionOutcome::AwaitApproval) {
                for approval_id in &result.approval_request_ids {
                    let ctx = RuntimeExecutionContext {
                        instance_id: result.instance_id.as_uuid(),
                        request_id: None,
                        source_task_id: Some(result.task_id.as_uuid()),
                    };
                    if let Err(e) = gateway
                        .notify_approval_required(&ctx, *approval_id)
                        .await
                    {
                        tracing::warn!(
                            "Approval gateway notification failed for {}: {}",
                            approval_id.as_uuid(),
                            e
                        );
                    }
                }
            }
        }
    }

    async fn create_checkpoint(
        &mut self,
        instance_id: AgentInstanceId,
        reason: &str,
        messages: Option<&[RuntimeMessage]>,
    ) -> Result<RuntimeCheckpointRef, RuntimeHostError> {
        let checkpoint = self.runtime.create_checkpoint(instance_id)?;
        let run_id = checkpoint.instance_id.as_uuid();
        let node_id = checkpoint.active_task_id.map(|id| id.as_uuid()).unwrap_or(run_id);

        let serialized_messages: Vec<serde_json::Value> = messages
            .map(|msgs| {
                msgs.iter()
                    .map(|m| {
                        let mut obj = serde_json::json!({
                            "role": match m.role {
                                RuntimeMessageRole::User => "user",
                                RuntimeMessageRole::Assistant => "assistant",
                                RuntimeMessageRole::System => "system",
                                RuntimeMessageRole::Tool => "tool",
                            },
                            "content": m.content,
                        });
                        if let Some(ref tc) = m.tool_calls {
                            obj["tool_calls"] = serde_json::to_value(tc).unwrap_or_default();
                        }
                        if let Some(ref tci) = m.tool_call_id {
                            obj["tool_call_id"] = serde_json::Value::String(tci.clone());
                        }
                        if let Some(ref name) = m.name {
                            obj["name"] = serde_json::Value::String(name.clone());
                        }
                        obj
                    })
                    .collect()
            })
            .unwrap_or_default();

        let state = serde_json::json!({
            "messages": serialized_messages,
            "tool_call_count": 0,
            "intermediate_results": [],
            "custom_state": {
                "instance_state": format!("{:?}", checkpoint.instance_state),
                "checkpoint_reason": reason,
                "active_task_state": checkpoint.active_task_state.map(|s| format!("{:?}", s)),
                "pending_approval_ids": checkpoint.pending_approval_ids.iter().map(|id| id.as_uuid()).collect::<Vec<_>>(),
                "child_delegation_ids": checkpoint.child_delegation_ids.iter().map(|id| id.as_uuid()).collect::<Vec<_>>(),
                "event_sequence": checkpoint.event_sequence,
            },
        });

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
