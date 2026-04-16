pub mod session;
pub mod tool;
pub mod memory;
pub mod agent_instance;

pub use session::SessionService;
pub use memory::MemoryService;
pub use tool::ToolService;

pub struct ServiceContainer {
    pub session: std::sync::Arc<SessionService>,
    pub memory: std::sync::Arc<memory::MemoryService>,
    pub tool: std::sync::Arc<ToolService>,
    pub agent_instance: std::sync::Arc<agent_instance::AgentInstanceService>,
}

impl ServiceContainer {
    pub async fn new(
        repos: crate::repository::RepositoryContainer,
        _db: crate::db::Database,
        llm: std::sync::Arc<llm::OpenAiClient>,
    ) -> Self {
        let tool = std::sync::Arc::new(ToolService::new().await);
        let memory = std::sync::Arc::new(memory::MemoryService::new(repos.memory.clone()));
        let session = std::sync::Arc::new(SessionService::new(
            repos.session.clone(),
            repos.message.clone(),
            llm,
            tool.clone(),
            memory.clone(),
        ));
        let agent_instance = std::sync::Arc::new(agent_instance::AgentInstanceService::new(
            repos.session.clone(),
        ));

        Self { session, memory, tool, agent_instance }
    }
}
