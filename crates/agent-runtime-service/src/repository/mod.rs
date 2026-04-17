use std::sync::Arc;

pub mod agent_definition;
pub mod agent_instance;
pub mod checkpoint;
pub mod event;
pub mod memory;
pub mod message;
pub mod session;

pub use agent_definition::{AgentDefinitionRepository, PostgresAgentDefinitionRepository};
pub use agent_instance::{AgentInstanceRepository, PostgresAgentInstanceRepository};
pub use checkpoint::{CheckpointRepository, PostgresCheckpointRepository};
pub use event::{EventRepository, PostgresEventRepository};
pub use memory::{MemoryRepository, PostgresMemoryRepository};
pub use message::{MessageRepository, PostgresMessageRepository};
pub use session::{PostgresSessionRepository, SessionKernelState, SessionRepository};

pub struct RepositoryContainer {
    pub session: Arc<dyn SessionRepository>,
    pub message: Arc<dyn MessageRepository>,
    pub memory: Arc<dyn MemoryRepository>,
    pub event: Arc<dyn EventRepository>,
    pub checkpoint: Arc<dyn CheckpointRepository>,
    pub agent_definition: Arc<dyn AgentDefinitionRepository>,
    pub agent_instance: Arc<dyn AgentInstanceRepository>,
}
