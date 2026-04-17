use crate::repository::{
    AgentDefinitionRepository, CheckpointRepository, EventRepository, SessionRepository,
};
use std::sync::Arc;

pub struct AgentInstanceService {
    _session_repo: Arc<dyn SessionRepository>,
    _event_repo: Arc<dyn EventRepository>,
    _checkpoint_repo: Arc<dyn CheckpointRepository>,
    _agent_definition_repo: Arc<dyn AgentDefinitionRepository>,
}

impl AgentInstanceService {
    pub fn new(
        session_repo: Arc<dyn SessionRepository>,
        event_repo: Arc<dyn EventRepository>,
        checkpoint_repo: Arc<dyn CheckpointRepository>,
        agent_definition_repo: Arc<dyn AgentDefinitionRepository>,
    ) -> Self {
        Self {
            _session_repo: session_repo,
            _event_repo: event_repo,
            _checkpoint_repo: checkpoint_repo,
            _agent_definition_repo: agent_definition_repo,
        }
    }
}
