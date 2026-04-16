use crate::repository::SessionRepository;
use std::sync::Arc;

pub struct AgentInstanceService {
    _session_repo: Arc<dyn SessionRepository>,
}

impl AgentInstanceService {
    pub fn new(session_repo: Arc<dyn SessionRepository>) -> Self {
        Self {
            _session_repo: session_repo,
        }
    }
}
