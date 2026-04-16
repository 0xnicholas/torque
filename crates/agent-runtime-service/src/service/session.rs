use crate::agent::stream::StreamEvent;
use crate::infra::llm::OpenAiClient;
use crate::repository::{MessageRepository, SessionRepository};
use crate::service::{MemoryService, ToolService};
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum SessionServiceError {
    #[error("not found")]
    NotFound,
    #[error("forbidden")]
    Forbidden,
    #[error("conflict")]
    Conflict,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub struct SessionService {
    session_repo: Arc<dyn SessionRepository>,
    message_repo: Arc<dyn MessageRepository>,
    _llm: Arc<OpenAiClient>,
    _tools: Arc<ToolService>,
    _memory: Arc<MemoryService>,
}

impl SessionService {
    pub fn new(
        session_repo: Arc<dyn SessionRepository>,
        message_repo: Arc<dyn MessageRepository>,
        llm: Arc<OpenAiClient>,
        tools: Arc<ToolService>,
        memory: Arc<MemoryService>,
    ) -> Self {
        Self { session_repo, message_repo, _llm: llm, _tools: tools, _memory: memory }
    }

    pub async fn create(
        &self,
        api_key: &str,
        project_scope: &str,
    ) -> Result<crate::models::Session, SessionServiceError> {
        self.session_repo.create(api_key, project_scope).await
            .map_err(SessionServiceError::Other)
    }

    pub async fn list(
        &self,
        api_key: &str,
        limit: i64,
    ) -> Result<Vec<crate::models::Session>, SessionServiceError> {
        self.session_repo.list(api_key, limit).await
            .map_err(SessionServiceError::Other)
    }

    pub async fn get_by_id(
        &self,
        session_id: Uuid,
        api_key: &str,
    ) -> Result<crate::models::Session, SessionServiceError> {
        let session = self.session_repo.get_by_id(session_id).await
            .map_err(SessionServiceError::Other)?
            .ok_or(SessionServiceError::NotFound)?;

        use subtle::ConstantTimeEq;
        if !bool::from(session.api_key.as_bytes().ct_eq(api_key.as_bytes())) {
            return Err(SessionServiceError::Forbidden);
        }

        Ok(session)
    }

    pub async fn chat(
        &self,
        _session_id: Uuid,
        _api_key: &str,
        _message: String,
        _event_sink: mpsc::Sender<StreamEvent>,
    ) -> Result<(), SessionServiceError> {
        Ok(())
    }

    pub async fn list_messages(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<crate::models::Message>, SessionServiceError> {
        self.message_repo.list_by_session(session_id, 100).await
            .map_err(SessionServiceError::Other)
    }
}
