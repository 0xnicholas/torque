pub mod session;
pub mod tool;
pub mod memory;
pub mod agent_instance;
pub mod agent_definition;

pub use session::SessionService;
pub use memory::MemoryService;
pub use tool::ToolService;
pub use agent_definition::AgentDefinitionService;

pub struct ServiceContainer {
    pub session: std::sync::Arc<SessionService>,
    pub memory: std::sync::Arc<memory::MemoryService>,
    pub tool: std::sync::Arc<ToolService>,
    pub agent_instance: std::sync::Arc<agent_instance::AgentInstanceService>,
    pub agent_definition: std::sync::Arc<AgentDefinitionService>,
    pub idempotency: std::sync::Arc<crate::v1_guards::IdempotencyStore>,
    pub run_gate: std::sync::Arc<crate::v1_guards::RunGate>,
}

impl ServiceContainer {
    pub async fn new(
        repos: crate::repository::RepositoryContainer,
        checkpointer: std::sync::Arc<dyn checkpointer::Checkpointer>,
        llm: std::sync::Arc<dyn llm::LlmClient>,
        idempotency: std::sync::Arc<crate::v1_guards::IdempotencyStore>,
        run_gate: std::sync::Arc<crate::v1_guards::RunGate>,
    ) -> Self {
        let tool = std::sync::Arc::new(ToolService::new().await);
        let memory = std::sync::Arc::new(memory::MemoryService::new(repos.memory.clone()));
        let session = std::sync::Arc::new(SessionService::new(
            repos.session.clone(),
            repos.message.clone(),
            repos.event.clone(),
            repos.checkpoint.clone(),
            checkpointer,
            llm,
            tool.clone(),
            memory.clone(),
        ));
        let agent_instance = std::sync::Arc::new(agent_instance::AgentInstanceService::new(
            repos.session.clone(),
            repos.event.clone(),
            repos.checkpoint.clone(),
            repos.agent_definition.clone(),
        ));
        let agent_definition = std::sync::Arc::new(AgentDefinitionService::new(
            repos.agent_definition.clone(),
        ));

        Self { session, memory, tool, agent_instance, agent_definition, idempotency, run_gate }
    }
}
