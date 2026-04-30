use crate::harness::{AgentLoop, AgentLoopError};
use crate::infra::llm::LlmClient;
use crate::models::v1::team::TriageResult;
use crate::policy::ToolGovernanceService;
use crate::service::governed_tool::GovernedToolRegistry;
use crate::service::team::supervisor_tools::{create_supervisor_tools, SupervisorToolsConfig};
use crate::tools::ToolRegistry;
use std::sync::Arc;
use tokio::sync::mpsc;
use torque_runtime::StepDecision;

pub struct SupervisorAgent {
    agent: AgentLoop,
    tools: Arc<ToolRegistry>,
}

impl SupervisorAgent {
    pub async fn new(
        llm: Arc<dyn LlmClient>,
        extra_tools: Vec<crate::tools::ToolArc>,
        supervisor_tools_config: Option<SupervisorToolsConfig>,
        tool_governance: Arc<ToolGovernanceService>,
    ) -> Self {
        let registry = Arc::new(ToolRegistry::new());

        if let Some(config) = supervisor_tools_config {
            let supervisor_tools = create_supervisor_tools(config);
            for tool in supervisor_tools {
                registry.register(tool).await;
            }
        }

        for tool in extra_tools {
            registry.register(tool).await;
        }

        let governed_registry =
            Arc::new(GovernedToolRegistry::new(registry.clone(), tool_governance));
        let agent = AgentLoop::new(
            llm,
            Arc::new(crate::harness::react::ToolExecution::Governed(
                governed_registry,
            )),
        );

        Self {
            agent,
            tools: registry,
        }
    }

    pub async fn list_tool_names(&self) -> Vec<String> {
        self.tools.list_tool_names().await
    }

    pub async fn run(
        &mut self,
        task: &str,
        event_sink: mpsc::Sender<crate::agent::stream::StreamEvent>,
    ) -> Result<StepDecision, AgentLoopError> {
        let system_prompt = r#"You are a Team Supervisor agent.

You lead a team of specialists to accomplish tasks. You must:
1. Understand the task goal
2. Select appropriate team members
3. Delegate tasks with clear instructions
4. Evaluate and accept/reject results
5. Publish successful results to the team
6. Complete the team task when done

Available tools let you delegate, accept/reject results, publish artifacts, and manage the team."#;

        self.agent.run(task, Some(system_prompt), event_sink).await
    }

    pub fn turn_count(&self) -> u32 {
        self.agent.turn_count()
    }

    pub async fn triage(&self, task: &str) -> Result<TriageResult, AgentLoopError> {
        self.agent.triage(task).await
    }
}
