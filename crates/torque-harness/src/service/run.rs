use crate::agent::stream::StreamEvent;
use crate::config;
use crate::infra::llm::LlmClient;
use crate::runtime::message::RuntimeMessage;
use crate::runtime::mapping::{run_request_to_execution_request, v1_agent_definition_to_kernel};
use crate::models::v1::agent_instance::AgentInstanceStatus;
use crate::models::v1::gating::ExecutionSummary;
use crate::models::v1::run::RunRequest;
use crate::models::v1::task::{TaskStatus, TaskType};
use crate::policy::{PolicyEvaluator, PolicyInput};
use crate::repository::{
    AgentDefinitionRepository, AgentInstanceRepository, TaskRepository,
};
use crate::service::candidate_generator::CandidateGenerator;
use crate::service::gating::MemoryGatingService;
use crate::service::memory_pipeline::MemoryPipelineService;
use crate::service::reflexion::ReflexionService;
use crate::service::governed_tool::GovernedToolRegistry;
use crate::service::{RuntimeFactory, ToolService};
use std::sync::Arc;
use torque_runtime::checkpoint::RuntimeCheckpointPayload;
use torque_runtime::message::StructuredMessage;
use torque_runtime::message_queue::{DeliveryMode, InMemoryMessageQueue, MessageQueue};
use tokio::sync::mpsc;
use tracing::warn;
use uuid::Uuid;

use crate::policy::ToolGovernanceService;

pub struct RunService {
    agent_definition_repo: Arc<dyn AgentDefinitionRepository>,
    agent_instance_repo: Arc<dyn AgentInstanceRepository>,
    task_repo: Arc<dyn TaskRepository>,
    runtime_factory: Arc<RuntimeFactory>,
    llm: Arc<dyn LlmClient>,
    tools: Arc<ToolService>,
    tool_governance: Arc<ToolGovernanceService>,
    policy_evaluator: PolicyEvaluator,
    candidate_generator: Arc<dyn CandidateGenerator>,
    gating: Arc<MemoryGatingService>,
    memory_pipeline: Arc<MemoryPipelineService>,
    reflexion: Option<Arc<ReflexionService>>,
}

impl RunService {
    pub fn new(
        agent_definition_repo: Arc<dyn AgentDefinitionRepository>,
        agent_instance_repo: Arc<dyn AgentInstanceRepository>,
        task_repo: Arc<dyn TaskRepository>,
        runtime_factory: Arc<RuntimeFactory>,
        llm: Arc<dyn LlmClient>,
        tools: Arc<ToolService>,
        tool_governance: Arc<ToolGovernanceService>,
        candidate_generator: Arc<dyn CandidateGenerator>,
        gating: Arc<MemoryGatingService>,
        memory_pipeline: Arc<MemoryPipelineService>,
        reflexion: Option<Arc<ReflexionService>>,
    ) -> Self {
        Self {
            agent_definition_repo,
            agent_instance_repo,
            task_repo,
            runtime_factory,
            llm,
            tools,
            tool_governance,
            policy_evaluator: PolicyEvaluator::new(),
            candidate_generator,
            gating,
            memory_pipeline,
            reflexion,
        }
    }

    pub async fn execute(
        &self,
        instance_id: Uuid,
        request: RunRequest,
        event_sink: mpsc::Sender<StreamEvent>,
    ) -> anyhow::Result<()> {
        self.execute_inner(
            instance_id,
            request,
            event_sink,
            vec![RuntimeMessage::user("Run request execution")],
        )
        .await
    }

    pub async fn execute_with_history(
        &self,
        instance_id: Uuid,
        request: RunRequest,
        event_sink: mpsc::Sender<StreamEvent>,
        initial_messages: Vec<RuntimeMessage>,
    ) -> anyhow::Result<()> {
        self.execute_inner(instance_id, request, event_sink, initial_messages)
            .await
    }

    async fn execute_inner(
        &self,
        instance_id: Uuid,
        request: RunRequest,
        event_sink: mpsc::Sender<StreamEvent>,
        initial_messages: Vec<RuntimeMessage>,
    ) -> anyhow::Result<()> {
        self.execute_inner_with_depth(instance_id, request, event_sink, initial_messages, 0)
            .await
    }

    /// Recursive execution with followUp depth guard (max 3 levels).
    async fn execute_inner_with_depth(
        &self,
        instance_id: Uuid,
        request: RunRequest,
        event_sink: mpsc::Sender<StreamEvent>,
        initial_messages: Vec<RuntimeMessage>,
        depth: usize,
    ) -> anyhow::Result<()> {
        const MAX_FOLLOWUP_DEPTH: usize = 3;
        if depth >= MAX_FOLLOWUP_DEPTH {
            warn!("FollowUp chain reached max depth {}; discarding remaining", MAX_FOLLOWUP_DEPTH);
            return Ok(());
        }
        // 1. Fetch instance and definition
        let instance = self
            .agent_instance_repo
            .get(instance_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Agent instance not found: {}", instance_id))?;

        let definition = self
            .agent_definition_repo
            .get(instance.agent_definition_id)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Agent definition not found: {}",
                    instance.agent_definition_id
                )
            })?;

        // 2. Update instance status to Running
        self.agent_instance_repo
            .update_status(instance_id, AgentInstanceStatus::Running)
            .await?;

        // 3. Create task
        let task = self
            .task_repo
            .create(
                TaskType::AgentTask,
                &request.goal,
                request.instructions.as_deref(),
                Some(instance_id),
                serde_json::to_value(&request.input_artifacts)?,
            )
            .await?;

        // 4. Link task to instance
        self.agent_instance_repo
            .update_current_task(instance_id, Some(task.id))
            .await?;
        self.task_repo
            .update_status(task.id, TaskStatus::Running)
            .await?;

        // 5. Record execution start event
        if let Err(e) = event_sink
            .send(StreamEvent::Start {
                session_id: instance_id,
            })
            .await
        {
            warn!("Failed to send start event: {}", e);
        }

        // 6. Build kernel agent definition and execution request
        let kernel_def = v1_agent_definition_to_kernel(&definition);
        let execution_request =
            run_request_to_execution_request(&kernel_def, &request, Some(instance_id));

        // 7. Execute via kernel bridge (no policy inside kernel)
        let execution_result = self
            .run_execution(
                instance_id,
                kernel_def,
                execution_request,
                event_sink.clone(),
                initial_messages,
            )
            .await;


        let (summary, mut queue) = match execution_result {
            Ok((summary, queue)) => (summary, queue),
            Err(e) => {
                // 8. Update task status for failure
                self.task_repo.update_status(task.id, TaskStatus::Failed).await?;
                self.agent_instance_repo
                    .update_current_task(instance_id, None)
                    .await?;
                self.agent_instance_repo
                    .update_status(instance_id, AgentInstanceStatus::Failed)
                    .await?;

                if let Err(send_err) = event_sink
                    .send(StreamEvent::Error {
                        code: "EXECUTION_ERROR".into(),
                        message: e.to_string(),
                    })
                    .await
                {
                    warn!("Failed to send error event: {}", send_err);
                }

                // Create failure checkpoint
                let snapshot = serde_json::json!({
                    "status": "FAILED",
                    "task_id": task.id,
                    "goal": request.goal,
                });
                if let Err(ce) = self
                    .create_checkpoint(instance_id, Some(task.id), snapshot, None)
                    .await
                {
                    warn!("Failed to create checkpoint after failure: {}", ce);
                }

                return Err(e);
            }
        };

        // 8. Update task status to completed
        self.task_repo.update_status(task.id, TaskStatus::Completed).await?;
        // 9. Update instance status to Ready
        self.agent_instance_repo
            .update_current_task(instance_id, None)
            .await?;
        self.agent_instance_repo
            .update_status(instance_id, AgentInstanceStatus::Ready)
            .await?;

        // 10. Send terminal event
        if let Err(e) = event_sink
            .send(StreamEvent::Done {
                message_id: task.id,
                artifacts: None,
            })
            .await
        {
            warn!("Failed to send done event: {}", e);
        }

        // 11. Create checkpoint after execution
        let snapshot = serde_json::json!({
            "status": "READY",
            "task_id": task.id,
            "goal": request.goal,
        });

        if let Err(e) = self
            .create_checkpoint(instance_id, Some(task.id), snapshot, None)
            .await
        {
            warn!("Failed to create checkpoint after execution: {}", e);
        }

        // 12. Process candidate generation with the summary
        let exec_summary = ExecutionSummary {
            task_id: task.id,
            agent_instance_id: instance_id,
            goal: request.goal.clone(),
            output_summary: summary,
            tool_calls: vec![],
            duration_ms: None,
        };
        let candidate_config = config::candidate_generation_config();
        match self
            .candidate_generator
            .generate_candidates(&exec_summary, &candidate_config)
            .await
        {
            Ok(candidates) => {
                for candidate in candidates {
                    match self.memory_pipeline.gate_and_notify(&candidate).await {
                        Ok(decision) => {
                            tracing::debug!(
                                "Memory candidate {} gated: {:?}",
                                candidate.id,
                                decision.decision
                            );
                        }
                        Err(e) => {
                            tracing::warn!("Failed to gate candidate {}: {}", candidate.id, e);
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Failed to generate memory candidates: {}", e);
            }
        }

        // 13. Check for followUp messages — chain recursive execution
        let followups = queue.drain_followups();
        if !followups.is_empty() {
            tracing::debug!(
                "FollowUp chain: {} message(s) at depth {}",
                followups.len(),
                depth
            );

            let followup_messages: Vec<RuntimeMessage> = followups
                .into_iter()
                .map(|sm| sm.into())
                .collect();

            // Create a follow-up request using the same goal
            let followup_request = RunRequest {
                goal: format!("[followUp] {}", request.goal),
                instructions: request.instructions.clone(),
                input_artifacts: request.input_artifacts.clone(),
                external_context_refs: request.external_context_refs.clone(),
                constraints: request.constraints.clone(),
                execution_mode: request.execution_mode.clone(),
                expected_outputs: request.expected_outputs.clone(),
                idempotency_key: None,
                webhook_url: None,
                async_execution: false,
                agent_instance_id: Some(instance_id),
            };

            return Box::pin(self.execute_inner_with_depth(
                instance_id,
                followup_request,
                event_sink,
                followup_messages,
                depth + 1,
            ))
            .await;
        }

        Ok(())
    }

    /// Creates a post-execution checkpoint with the runtime state.
    /// Accepts optional messages from the agent loop for persistence.
    pub async fn create_checkpoint(
        &self,
        instance_id: Uuid,
        task_id: Option<Uuid>,
        snapshot: serde_json::Value,
        messages: Option<&[serde_json::Value]>,
    ) -> anyhow::Result<()> {
        let state = serde_json::json!({
            "messages": messages.unwrap_or(&[]),
            "tool_call_count": 0,
            "intermediate_results": [],
            "custom_state": {
                "instance_state": snapshot.get("status").and_then(|s| s.as_str()).unwrap_or("Ready"),
                "checkpoint_reason": "run_service",
                "active_task_state": null,
                "pending_approval_ids": Vec::<Uuid>::new(),
                "child_delegation_ids": Vec::<Uuid>::new(),
                "event_sequence": 0,
            },
        });

        let checkpoint = RuntimeCheckpointPayload {
            instance_id: torque_kernel::AgentInstanceId::new(),
            node_id: task_id.unwrap_or(instance_id),
            reason: "run_service".to_string(),
            state,
        };

        self.runtime_factory
            .checkpointer()
            .save(checkpoint)
            .await?;
        Ok(())
    }

    /// Evaluate tool policy before execution using multi-source dimensional merge.
    /// This is called by the orchestration layer, not the kernel.
    pub fn evaluate_tool_policy(
        &self,
        tool_name: &str,
        agent_definition_id: Uuid,
        tool_policy: serde_json::Value,
        team_policy: Option<serde_json::Value>,
    ) -> crate::policy::PolicyDecision {
        use crate::policy::PolicySources;

        let input = PolicyInput {
            action_type: "tool_call".to_string(),
            tool_name: Some(tool_name.to_string()),
            agent_definition_id: Some(agent_definition_id),
            ..Default::default()
        };

        let sources = PolicySources::new().with_agent(tool_policy);

        // Add team policy if available
        let sources = if let Some(team) = team_policy {
            sources.with_team(team)
        } else {
            sources
        };

        self.policy_evaluator.evaluate(&input, &sources)
    }

    async fn run_execution(
        &self,
        _instance_id: Uuid,
        kernel_def: torque_kernel::AgentDefinition,
        request: torque_kernel::ExecutionRequest,
        event_sink: mpsc::Sender<StreamEvent>,
        initial_messages: Vec<RuntimeMessage>,
    ) -> anyhow::Result<(String, InMemoryMessageQueue)> {
        let mut kernel = self.runtime_factory.create_handle(vec![kernel_def]);
        let model_driver = self.runtime_factory.create_model_driver(self.llm.clone());
        let governed_tool_registry = Arc::new(GovernedToolRegistry::new(
            self.tools.registry(),
            self.tool_governance.clone(),
        ));
        let tool_executor = self.runtime_factory.create_tool_executor(governed_tool_registry);
        let output_sink = self.runtime_factory.create_output_sink(event_sink.clone());

        // Build a queue from initial messages for lifetime-safety and followUp chaining.
        let mut queue = InMemoryMessageQueue::new(
            initial_messages
                .iter()
                .map(StructuredMessage::from_runtime)
                .collect(),
        );

        let result = kernel
            .execute_v1_with_queue(
                request,
                &model_driver,
                &tool_executor,
                Some(&output_sink),
                &mut queue,
            )
            .await;

        result
            .map(|r| (r.summary.unwrap_or_default(), queue))
            .map_err(|e| anyhow::anyhow!("Kernel execution failed: {}", e))
    }
}
