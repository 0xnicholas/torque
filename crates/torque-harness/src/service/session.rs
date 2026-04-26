use crate::agent::stream::StreamEvent;
use crate::infra::llm::{LlmClient, LlmMessage};
use crate::runtime::message::RuntimeMessage;
use crate::runtime::mapping::session_to_execution_request;
use crate::repository::{MessageRepository, SessionRepository};
use crate::service::{ContextCompactionService, MemoryService, RuntimeFactory, ToolService};
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
    runtime_factory: Arc<RuntimeFactory>,
    llm: Arc<dyn LlmClient>,
    tools: Arc<ToolService>,
    memory: Arc<MemoryService>,
}

impl SessionService {
    pub fn new(
        session_repo: Arc<dyn SessionRepository>,
        message_repo: Arc<dyn MessageRepository>,
        runtime_factory: Arc<RuntimeFactory>,
        llm: Arc<dyn LlmClient>,
        tools: Arc<ToolService>,
        memory: Arc<MemoryService>,
    ) -> Self {
        Self {
            session_repo,
            message_repo,
            runtime_factory,
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

        let _ = event_sink.send(StreamEvent::Start { session_id }).await;

        let hydration_source = self
            .runtime_factory
            .create_hydration_source(self.session_repo.clone());
        let mut kernel = self
            .runtime_factory
            .create_handle(vec![])
            .with_hydration_source(Arc::new(hydration_source));
        let model_driver = self.runtime_factory.create_model_driver(self.llm.clone());
        let tool_executor = self.runtime_factory.create_tool_executor(self.tools.clone());
        let output_sink = self.runtime_factory.create_output_sink(event_sink.clone());

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

        if llm_messages.len() > 1 {
            let mut compacted_messages = vec![llm_messages[0].clone()];
            let remaining_messages = llm_messages[1..].to_vec();
            if let Some(summary) = ContextCompactionService::default().compact(&remaining_messages) {
                compacted_messages.push(LlmMessage::system(format!(
                    "{}\nKey facts:\n- {}",
                    summary.compact_summary,
                    summary.key_facts.join("\n- ")
                )));
                compacted_messages.extend(summary.preserved_tail);
                llm_messages = compacted_messages;
            }
        }

        let runtime_messages = llm_messages
            .into_iter()
            .map(RuntimeMessage::from)
            .collect::<Vec<_>>();

        let result = kernel
            .execute_chat(
                request,
                &model_driver,
                &tool_executor,
                Some(&output_sink),
                runtime_messages,
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
