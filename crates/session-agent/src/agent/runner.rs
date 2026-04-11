use crate::agent::{
    context::{ContextManager, DEFAULT_MEMORY_RECALL_LIMIT},
    stream::StreamEvent,
};
use crate::db::Database;
use crate::models::{Message, Session};
use crate::tools::ToolRegistry;
use llm::{Chunk, FinishReason, LlmClient, Message as LlmMessage};
use serde_json::Value;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

pub const MAX_TOOL_CALLS: usize = 20;
pub const MAX_CONSECUTIVE_TOOL_FAILURES: usize = 3;

pub struct AgentRunner<C: LlmClient> {
    llm: Arc<C>,
    db: Database,
    tools: Arc<ToolRegistry>,
    context_manager: ContextManager,
}

impl<C: LlmClient> AgentRunner<C> {
    pub fn new(llm: Arc<C>, db: Database, tools: Arc<ToolRegistry>) -> Self {
        Self {
            llm,
            db,
            tools,
            context_manager: ContextManager::new(),
        }
    }

    pub async fn run(
        &self,
        session: &Session,
        user_message: &Message,
        tx: mpsc::Sender<StreamEvent>,
    ) -> anyhow::Result<Message> {
        let _ = tx
            .send(StreamEvent::Start {
                session_id: session.id,
            })
            .await;

        let _user_msg = crate::db::messages::create(self.db.pool(), user_message).await?;

        let history =
            crate::db::messages::get_recent_by_session(self.db.pool(), session.id, 50).await?;

        let context = self.context_manager.build_context(history);
        let recalled_memory = crate::db::memory_entries::recall_for_prompt(
            self.db.pool(),
            &session.project_scope,
            &user_message.content,
            DEFAULT_MEMORY_RECALL_LIMIT,
        )
        .await?;
        let tool_defs = self.tools.to_llm_tools().await;

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
        if let Some(memory_slice) = self
            .context_manager
            .build_memory_slice_message(&recalled_memory)
        {
            llm_messages.push(memory_slice);
        }
        llm_messages.extend(context.to_llm_messages());

        let (final_content, tool_calls_log) = self
            .execute_with_tools(llm_messages, tool_defs, tx.clone())
            .await?;

        let artifacts = self.extract_artifacts(&final_content);

        let assistant_msg = Message::assistant(
            session.id,
            final_content,
            Some(serde_json::to_value(&tool_calls_log)?),
            artifacts.clone(),
        );

        let saved_msg = crate::db::messages::create(self.db.pool(), &assistant_msg).await?;

        let _ = tx
            .send(StreamEvent::Done {
                message_id: saved_msg.id,
                artifacts,
            })
            .await;

        Ok(saved_msg)
    }

    async fn execute_with_tools(
        &self,
        mut messages: Vec<LlmMessage>,
        tools: Vec<llm::ToolDef>,
        tx: mpsc::Sender<StreamEvent>,
    ) -> anyhow::Result<(String, Vec<Value>)> {
        let mut tool_call_count = 0;
        let mut consecutive_failures = 0;
        let mut tool_calls_log = Vec::new();

        loop {
            if tool_call_count >= MAX_TOOL_CALLS {
                return Err(anyhow::anyhow!("Maximum tool call limit reached"));
            }

            let request = llm::ChatRequest::new(self.llm.model().to_string(), messages.clone())
                .with_tools(tools.clone());

            let content_buffer = Arc::new(Mutex::new(String::new()));
            let tool_calls_buffer: Arc<Mutex<Vec<llm::ToolCall>>> =
                Arc::new(Mutex::new(Vec::new()));
            let tx_clone = tx.clone();
            let content_buffer_clone = content_buffer.clone();
            let tool_calls_buffer_clone = tool_calls_buffer.clone();

            let response = self
                .llm
                .chat_streaming(request, move |chunk: Chunk| {
                    let content = chunk.content.clone();
                    if let Some(tool_call) = &chunk.tool_call {
                        let mut calls = tool_calls_buffer_clone.lock().unwrap();
                        if !calls.iter().any(|t| t.id == tool_call.id) {
                            calls.push(tool_call.clone());
                        }
                    }
                    if !content.is_empty() {
                        content_buffer_clone.lock().unwrap().push_str(&content);
                        let _ = tx_clone.try_send(StreamEvent::Chunk { content });
                    }
                })
                .await?;

            let content = Arc::try_unwrap(content_buffer)
                .map(|m| m.into_inner().unwrap_or_default())
                .unwrap_or_default();
            let tool_calls = Arc::try_unwrap(tool_calls_buffer)
                .map(|m| m.into_inner().unwrap_or_default())
                .unwrap_or_default();

            match response.finish_reason {
                FinishReason::ToolCalls => {
                    tool_call_count += 1;

                    for tool_call in &tool_calls {
                        let _ = tx
                            .send(StreamEvent::ToolCall {
                                name: tool_call.name.clone(),
                                arguments: tool_call.arguments.clone(),
                            })
                            .await;

                        tool_calls_log.push(serde_json::json!({
                            "name": tool_call.name,
                            "arguments": tool_call.arguments,
                        }));

                        let result = self
                            .tools
                            .execute(&tool_call.name, tool_call.arguments.clone())
                            .await?;
                        let _ = tx
                            .send(StreamEvent::ToolResult {
                                name: tool_call.name.clone(),
                                success: result.success,
                                content: result.content.clone(),
                                error: result.error.clone(),
                            })
                            .await;

                        if result.success {
                            consecutive_failures = 0;
                        } else {
                            consecutive_failures += 1;
                            if consecutive_failures >= MAX_CONSECUTIVE_TOOL_FAILURES {
                                return Err(anyhow::anyhow!(
                                    "Tool execution failed {} times consecutively",
                                    consecutive_failures
                                ));
                            }
                        }

                        messages.push(LlmMessage::user(&format!(
                            "Tool '{}' result: {}",
                            tool_call.name, result.content
                        )));
                    }
                }
                _ => {
                    return Ok((content, tool_calls_log));
                }
            }
        }
    }

    fn extract_artifacts(&self, content: &str) -> Option<Value> {
        if let Ok(json) = serde_json::from_str::<Value>(content) {
            if let Some(artifact) = json.get("__artifact") {
                return Some(artifact.clone());
            }
        }
        None
    }
}
