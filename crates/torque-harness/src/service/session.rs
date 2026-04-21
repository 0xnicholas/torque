use crate::agent::stream::StreamEvent;
use crate::infra::llm::{LlmClient, LlmMessage};
use crate::kernel_bridge::{session_to_execution_request, KernelRuntimeHandle};
use crate::repository::{
    CheckpointRepository, EventRepository, MessageRepository, SessionRepository,
};
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
    event_repo: Arc<dyn EventRepository>,
    checkpoint_repo: Arc<dyn CheckpointRepository>,
    checkpointer: Arc<dyn checkpointer::Checkpointer>,
    llm: Arc<dyn LlmClient>,
    tools: Arc<ToolService>,
    memory: Arc<MemoryService>,
}

impl SessionService {
    pub fn new(
        session_repo: Arc<dyn SessionRepository>,
        message_repo: Arc<dyn MessageRepository>,
        event_repo: Arc<dyn EventRepository>,
        checkpoint_repo: Arc<dyn CheckpointRepository>,
        checkpointer: Arc<dyn checkpointer::Checkpointer>,
        llm: Arc<dyn LlmClient>,
        tools: Arc<ToolService>,
        memory: Arc<MemoryService>,
    ) -> Self {
        Self {
            session_repo,
            message_repo,
            event_repo,
            checkpoint_repo,
            checkpointer,
            llm,
            tools,
            memory,
        }
    }

    pub async fn create(
        &self,
        api_key: &str,
        project_scope: &str,
    ) -> Result<crate::models::Session, SessionServiceError> {
        self.session_repo
            .create(api_key, project_scope)
            .await
            .map_err(SessionServiceError::Other)
    }

    pub async fn list(
        &self,
        api_key: &str,
        limit: i64,
    ) -> Result<Vec<crate::models::Session>, SessionServiceError> {
        let sessions = self
            .session_repo
            .list(limit)
            .await
            .map_err(SessionServiceError::Other)?;
        Ok(sessions
            .into_iter()
            .filter(|s| s.api_key == api_key)
            .collect())
    }

    pub async fn get_by_id(
        &self,
        session_id: Uuid,
        api_key: &str,
    ) -> Result<crate::models::Session, SessionServiceError> {
        let session = self
            .session_repo
            .get_by_id(session_id)
            .await
            .map_err(SessionServiceError::Other)?
            .ok_or(SessionServiceError::NotFound)?;

        use subtle::ConstantTimeEq;
        if !bool::from(session.api_key.as_bytes().ct_eq(api_key.as_bytes())) {
            return Err(SessionServiceError::Forbidden);
        }

        Ok(session)
    }

    pub fn tools(&self) -> &Arc<ToolService> {
        &self.tools
    }

    pub async fn chat(
        &self,
        session_id: Uuid,
        api_key: &str,
        message: String,
        event_sink: mpsc::Sender<StreamEvent>,
    ) -> Result<(), SessionServiceError> {
        let session = self.get_by_id(session_id, api_key).await?;

        if !self
            .session_repo
            .try_mark_running(session_id)
            .await
            .map_err(SessionServiceError::Other)?
        {
            return Err(SessionServiceError::Conflict);
        }

        let user_msg = crate::models::Message::user(session_id, message.clone());
        self.message_repo
            .create(&user_msg)
            .await
            .map_err(SessionServiceError::Other)?;

        let request = session_to_execution_request(&session, &message).map_err(|e| {
            SessionServiceError::Other(anyhow::anyhow!("kernel mapping error: {e}"))
        })?;

        let mut kernel = KernelRuntimeHandle::new(
            vec![],
            self.event_repo.clone(),
            self.checkpoint_repo.clone(),
            self.checkpointer.clone(),
        );

        let history = self
            .message_repo
            .get_recent_by_session(session_id, 50)
            .await
            .map_err(SessionServiceError::Other)?;

        let recalled_memory = self
            .memory
            .repo()
            .search_entries(&session.project_scope, &message, 10)
            .await
            .unwrap_or_default();

        let tool_defs = self.tools.registry().to_llm_tools().await;

        let system_prompt = format!(
            "You are a helpful assistant. You have access to tools: {}. \
             Use tools when needed. When you're done, provide a final response.",
            tool_defs
                .iter()
                .map(|t| t.name.clone())
                .collect::<Vec<_>>()
                .join(", ")
        );

        let mut llm_messages = vec![LlmMessage::system(&system_prompt)];
        if !recalled_memory.is_empty() {
            let memory_text = recalled_memory
                .iter()
                .map(|e| format!("[{:?}] {}", e.layer, e.content))
                .collect::<Vec<_>>()
                .join("\n");
            llm_messages.push(LlmMessage::user(format!(
                "Project memory:\n{}",
                memory_text
            )));
        }
        for msg in history {
            let role = msg.role.clone();
            let content = msg.content.clone();
            llm_messages.push(match role {
                crate::models::MessageRole::System => LlmMessage::system(content),
                crate::models::MessageRole::Assistant => LlmMessage::assistant(content),
                _ => LlmMessage::user(content),
            });
        }

        let result = kernel
            .execute_chat(
                request,
                self.llm.clone(),
                self.tools.registry().clone(),
                event_sink.clone(),
                llm_messages,
            )
            .await;

        match result {
            Ok(exec) => {
                let content = exec.summary.unwrap_or_default();
                let assistant_msg =
                    crate::models::Message::assistant(session_id, content, None, None);
                let saved = self
                    .message_repo
                    .create(&assistant_msg)
                    .await
                    .map_err(SessionServiceError::Other)?;
                self.session_repo
                    .update_status(session_id, crate::models::SessionStatus::Completed, None)
                    .await
                    .map_err(SessionServiceError::Other)?;
                let _ = event_sink
                    .send(StreamEvent::Done {
                        message_id: saved.id,
                        artifacts: None,
                    })
                    .await;
            }
            Err(e) => {
                self.session_repo
                    .update_status(
                        session_id,
                        crate::models::SessionStatus::Error,
                        Some(&e.to_string()),
                    )
                    .await
                    .map_err(SessionServiceError::Other)?;
                let _ = event_sink
                    .send(StreamEvent::Error {
                        code: "AGENT_ERROR".to_string(),
                        message: e.to_string(),
                    })
                    .await;
            }
        }

        Ok(())
    }

    pub async fn list_messages(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<crate::models::Message>, SessionServiceError> {
        self.message_repo
            .list_by_session(session_id, 100)
            .await
            .map_err(SessionServiceError::Other)
    }
}
