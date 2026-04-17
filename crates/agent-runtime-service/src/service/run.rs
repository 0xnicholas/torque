use crate::agent::stream::StreamEvent;
use crate::infra::llm::LlmClient;
use crate::kernel_bridge::{run_request_to_execution_request, v1_agent_definition_to_kernel, KernelRuntimeHandle};
use crate::models::v1::agent_instance::AgentInstanceStatus;
use crate::models::v1::run::RunRequest;
use crate::models::v1::task::{TaskStatus, TaskType};
use crate::repository::{
    AgentDefinitionRepository, AgentInstanceRepository, TaskRepository,
    EventRepository, CheckpointRepository,
};
use crate::service::ToolService;
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

pub struct RunService {
    agent_definition_repo: Arc<dyn AgentDefinitionRepository>,
    agent_instance_repo: Arc<dyn AgentInstanceRepository>,
    task_repo: Arc<dyn TaskRepository>,
    event_repo: Arc<dyn EventRepository>,
    checkpoint_repo: Arc<dyn CheckpointRepository>,
    checkpointer: Arc<dyn checkpointer::Checkpointer>,
    llm: Arc<dyn LlmClient>,
    tools: Arc<ToolService>,
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
        }
    }

    pub async fn execute(
        &self,
        instance_id: Uuid,
        request: RunRequest,
        event_sink: mpsc::Sender<StreamEvent>,
    ) -> anyhow::Result<()> {
        // 1. Fetch instance and definition
        let instance = self.agent_instance_repo.get(instance_id).await?
            .ok_or_else(|| anyhow::anyhow!("Agent instance not found: {}", instance_id))?;

        let definition = self.agent_definition_repo.get(instance.agent_definition_id).await?
            .ok_or_else(|| anyhow::anyhow!("Agent definition not found: {}", instance.agent_definition_id))?;

        // 2. Update instance status to Running
        self.agent_instance_repo.update_status(instance_id, AgentInstanceStatus::Running).await?;

        // 3. Create task
        let task = self.task_repo.create(
            TaskType::AgentTask,
            &request.goal,
            request.instructions.as_deref(),
            Some(instance_id),
            serde_json::to_value(&request.input_artifacts)?,
        ).await?;

        // 4. Link task to instance
        self.agent_instance_repo.update_current_task(instance_id, Some(task.id)).await?;
        self.task_repo.update_status(task.id, TaskStatus::Running).await?;

        // 5. Build kernel agent definition and execution request
        let kernel_def = v1_agent_definition_to_kernel(&definition);
        let execution_request = run_request_to_execution_request(&kernel_def, &request);

        // 6. Execute via kernel bridge
        let result = self.run_execution(
            instance_id,
            kernel_def,
            execution_request,
            event_sink.clone(),
        ).await;

        // 7. Update task status based on result
        let final_status = match &result {
            Ok(_) => TaskStatus::Completed,
            Err(_) => TaskStatus::Failed,
        };
        self.task_repo.update_status(task.id, final_status).await?;

        // 8. Update instance status
        self.agent_instance_repo.update_current_task(instance_id, None).await?;
        self.agent_instance_repo.update_status(
            instance_id,
            if result.is_ok() { AgentInstanceStatus::Ready } else { AgentInstanceStatus::Failed }
        ).await?;

        // 9. Send terminal event
        match result {
            Ok(_) => {
                let _ = event_sink.send(StreamEvent::Done {
                    message_id: task.id,
                    artifacts: None,
                }).await;
            }
            Err(ref e) => {
                let _ = event_sink.send(StreamEvent::Error {
                    code: "EXECUTION_ERROR".into(),
                    message: e.to_string(),
                }).await;
            }
        }

        result.map(|_| ())
    }

    async fn run_execution(
        &self,
        instance_id: Uuid,
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

        // Use existing execute_chat logic adapted for v1
        // For now, this is a simplified version that streams LLM responses
        let result = kernel.execute_chat(
            request,
            self.llm.clone(),
            self.tools.registry(),
            event_sink,
            vec![], // Start with empty messages for v1
        ).await;

        result
            .map(|r| r.summary.unwrap_or_default())
            .map_err(|e| anyhow::anyhow!("Kernel execution failed: {}", e))
    }
}
