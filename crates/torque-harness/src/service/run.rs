use crate::agent::stream::StreamEvent;
use crate::config;
use crate::infra::llm::LlmClient;
use crate::kernel_bridge::{
    run_request_to_execution_request, v1_agent_definition_to_kernel, KernelRuntimeHandle,
};
use crate::models::v1::agent_instance::AgentInstanceStatus;
use crate::models::v1::gating::ExecutionSummary;
use crate::models::v1::run::RunRequest;
use crate::models::v1::task::{TaskStatus, TaskType};
use crate::policy::{PolicyEvaluator, PolicyInput};
use crate::repository::{
    AgentDefinitionRepository, AgentInstanceRepository, CheckpointRepository, EventRepository,
    TaskRepository,
};
use crate::service::candidate_generator::CandidateGenerator;
use crate::service::gating::MemoryGatingService;
use crate::service::memory_pipeline::MemoryPipelineService;
use crate::service::reflexion::ReflexionService;
use crate::service::ToolService;
use checkpointer::CheckpointState;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::warn;
use uuid::Uuid;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ExecuteRequest {
    pub agent_definition_id: Uuid,
    pub agent_instance_id: Uuid,
    pub system_prompt: Option<String>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct ExecuteResponse {
    pub state: torque_kernel::ExecutionResult,
}

use crate::policy::ToolGovernanceService;

pub struct RunService {
    agent_definition_repo: Arc<dyn AgentDefinitionRepository>,
    agent_instance_repo: Arc<dyn AgentInstanceRepository>,
    task_repo: Arc<dyn TaskRepository>,
    event_repo: Arc<dyn EventRepository>,
    checkpoint_repo: Arc<dyn CheckpointRepository>,
    checkpointer: Arc<dyn checkpointer::Checkpointer>,
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
        event_repo: Arc<dyn EventRepository>,
        checkpoint_repo: Arc<dyn CheckpointRepository>,
        checkpointer: Arc<dyn checkpointer::Checkpointer>,
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
            event_repo,
            checkpoint_repo,
            checkpointer,
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
            )
            .await;

        // 7a. Memory candidate generation (on success)
        if let Ok(ref summary) = execution_result {
            let exec_summary = ExecutionSummary {
                task_id: task.id,
                agent_instance_id: instance_id,
                goal: request.goal.clone(),
                output_summary: summary.clone(),
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
        }

        let result = execution_result;

        // 8. Update task status based on result
        let final_status = match &result {
            Ok(_) => TaskStatus::Completed,
            Err(_) => TaskStatus::Failed,
        };
        self.task_repo.update_status(task.id, final_status).await?;

        // 9. Update instance status
        self.agent_instance_repo
            .update_current_task(instance_id, None)
            .await?;
        self.agent_instance_repo
            .update_status(
                instance_id,
                if result.is_ok() {
                    AgentInstanceStatus::Ready
                } else {
                    AgentInstanceStatus::Failed
                },
            )
            .await?;

        // 10. Send terminal event
        match result {
            Ok(_) => {
                if let Err(e) = event_sink
                    .send(StreamEvent::Done {
                        message_id: task.id,
                        artifacts: None,
                    })
                    .await
                {
                    warn!("Failed to send done event: {}", e);
                }
            }
            Err(ref e) => {
                if let Err(send_err) = event_sink
                    .send(StreamEvent::Error {
                        code: "EXECUTION_ERROR".into(),
                        message: e.to_string(),
                    })
                    .await
                {
                    warn!("Failed to send error event: {}", send_err);
                }
            }
        }

        // 11. Create checkpoint after execution
        let snapshot = serde_json::json!({
            "status": if result.is_ok() { "READY" } else { "FAILED" },
            "task_id": task.id,
            "goal": request.goal,
        });

        if let Err(e) = self
            .create_checkpoint(instance_id, Some(task.id), snapshot)
            .await
        {
            warn!("Failed to create checkpoint after execution: {}", e);
        }

        result.map(|_| ())
    }

    async fn create_checkpoint(
        &self,
        instance_id: Uuid,
        task_id: Option<Uuid>,
        snapshot: serde_json::Value,
    ) -> anyhow::Result<()> {
        let state = CheckpointState {
            messages: vec![],
            tool_call_count: 0,
            intermediate_results: vec![],
            custom_state: Some(serde_json::json!({
                "instance_state": snapshot.get("status").and_then(|s| s.as_str()).unwrap_or("Ready"),
                "checkpoint_reason": "run_service",
                "active_task_state": null,
                "pending_approval_ids": Vec::<Uuid>::new(),
                "child_delegation_ids": Vec::<Uuid>::new(),
                "event_sequence": 0,
            })),
        };

        self.checkpointer
            .save(instance_id, task_id.unwrap_or(instance_id), state)
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

    pub async fn execute_with_harness(
        &self,
        instance_id: Uuid,
        request: RunRequest,
        event_sink: mpsc::Sender<StreamEvent>,
    ) -> anyhow::Result<()> {
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

        self.agent_instance_repo
            .update_status(instance_id, AgentInstanceStatus::Running)
            .await?;

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

        self.agent_instance_repo
            .update_current_task(instance_id, Some(task.id))
            .await?;
        self.task_repo
            .update_status(task.id, TaskStatus::Running)
            .await?;

        if let Err(e) = event_sink
            .send(StreamEvent::Start {
                session_id: instance_id,
            })
            .await
        {
            warn!("Failed to send start event: {}", e);
        }

        let mut planning_executor = crate::harness::PlanningExecutor::new(
            self.llm.clone(),
            self.tools.registry(),
            self.tool_governance.clone(),
            self.reflexion.clone(),
        );

        let system_prompt = definition.system_prompt.as_deref();

        let result = planning_executor
            .plan_and_execute(&request.goal, system_prompt, event_sink.clone())
            .await;

        let final_status = match &result {
            Ok(ref r) if r.success => TaskStatus::Completed,
            _ => TaskStatus::Failed,
        };
        self.task_repo.update_status(task.id, final_status).await?;

        self.agent_instance_repo
            .update_current_task(instance_id, None)
            .await?;
        self.agent_instance_repo
            .update_status(
                instance_id,
                if result.as_ref().map(|r| r.success).unwrap_or(false) {
                    AgentInstanceStatus::Ready
                } else {
                    AgentInstanceStatus::Failed
                },
            )
            .await?;

        match &result {
            Ok(_) => {
                if let Err(e) = event_sink
                    .send(StreamEvent::Done {
                        message_id: task.id,
                        artifacts: None,
                    })
                    .await
                {
                    warn!("Failed to send done event: {}", e);
                }
            }
            Err(ref e) => {
                if let Err(send_err) = event_sink
                    .send(StreamEvent::Error {
                        code: "EXECUTION_ERROR".into(),
                        message: e.to_string(),
                    })
                    .await
                {
                    warn!("Failed to send error event: {}", send_err);
                }
            }
        }

        let snapshot = serde_json::json!({
            "status": if result.as_ref().map(|r| r.success).unwrap_or(false) { "READY" } else { "FAILED" },
            "task_id": task.id,
            "goal": request.goal,
        });

        if let Err(e) = self
            .create_checkpoint(instance_id, Some(task.id), snapshot)
            .await
        {
            warn!("Failed to create checkpoint: {}", e);
        }

        result.map(|_| ())
    }

    async fn run_execution(
        &self,
        _instance_id: Uuid,
        kernel_def: torque_kernel::AgentDefinition,
        request: torque_kernel::ExecutionRequest,
        event_sink: mpsc::Sender<StreamEvent>,
    ) -> anyhow::Result<String> {
        let mut kernel = KernelRuntimeHandle::new(
            vec![kernel_def],
            self.event_repo.clone(),
            self.checkpoint_repo.clone(),
            self.checkpointer.clone(),
        );

        // Execute without policy (policy is evaluated at orchestration layer)
        let result = kernel
            .execute_v1(
                request,
                self.llm.clone(),
                self.tools.registry(),
                event_sink,
                vec![], // Start with empty messages for v1
            )
            .await;

        result
            .map(|r| r.summary.unwrap_or_default())
            .map_err(|e| anyhow::anyhow!("Kernel execution failed: {}", e))
    }

// TODO: Re-implement when resume flow supports execution with message history.
// The resume endpoint should call execute_with_messages to resume execution
// with stored message history instead of starting fresh.
}
