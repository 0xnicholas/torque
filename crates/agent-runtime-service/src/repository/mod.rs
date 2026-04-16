use std::sync::Arc;

pub mod session;
pub mod message;
pub mod memory;
pub mod event;
pub mod checkpoint;
pub mod agent_definition;

pub use session::{PostgresSessionRepository, SessionRepository, SessionKernelState};
pub use message::{PostgresMessageRepository, MessageRepository};
pub use memory::{PostgresMemoryRepository, MemoryRepository};
pub use event::{PostgresEventRepository, EventRepository};
pub use checkpoint::{PostgresCheckpointRepository, CheckpointRepository};
pub use agent_definition::{PostgresAgentDefinitionRepository, AgentDefinitionRepository};

pub struct RepositoryContainer {
    pub session: Arc<dyn SessionRepository>,
    pub message: Arc<dyn MessageRepository>,
    pub memory: Arc<dyn MemoryRepository>,
    pub event: Arc<dyn EventRepository>,
    pub checkpoint: Arc<dyn CheckpointRepository>,
    pub agent_definition: Arc<dyn AgentDefinitionRepository>,
}
