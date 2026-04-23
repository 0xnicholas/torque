use crate::harness::{ReActHarness, ReActHarnessError};
use crate::infra::llm::LlmClient;
use crate::tools::ToolRegistry;
use crate::service::team::supervisor_tools::create_supervisor_tools;
use std::sync::Arc;
use tokio::sync::mpsc;
use torque_kernel::StepDecision;

pub struct SupervisorAgent {
    react: ReActHarness,
    tools: Arc<ToolRegistry>,
}

impl SupervisorAgent {
    pub async fn new(llm: Arc<dyn LlmClient>, extra_tools: Vec<crate::tools::ToolArc>) -> Self {
        let registry = Arc::new(ToolRegistry::new());

        let supervisor_tools = create_supervisor_tools();
        for tool in supervisor_tools {
            registry.register(tool).await;
        }

        for tool in extra_tools {
            registry.register(tool).await;
        }

        let react = ReActHarness::new(llm, registry.clone());

        Self {
            react,
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
    ) -> Result<StepDecision, ReActHarnessError> {
        let system_prompt = r#"You are a Team Supervisor agent.

You lead a team of specialists to accomplish tasks. You must:
1. Understand the task goal
2. Select appropriate team members
3. Delegate tasks with clear instructions
4. Evaluate and accept/reject results
5. Publish successful results to the team
6. Complete the team task when done

Available tools let you delegate, accept/reject results, publish artifacts, and manage the team."#;

        self.react.run(task, Some(system_prompt), event_sink).await
    }

    pub fn step_history(&self) -> &[crate::harness::ReActStep] {
        self.react.step_history()
    }
}