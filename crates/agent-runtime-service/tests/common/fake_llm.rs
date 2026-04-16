use async_trait::async_trait;
use llm::{
    ChatRequest, ChatResponse, Chunk, FinishReason, LlmClient, Message, TokenUsage, ToolCall,
};
use serde_json::Value;
use std::collections::VecDeque;
use std::sync::Mutex;

#[derive(Clone)]
struct ScriptedResponse {
    chunks: Vec<Chunk>,
    finish_reason: FinishReason,
    message_content: String,
}

pub struct FakeLlm {
    model: String,
    scripted: Mutex<VecDeque<ScriptedResponse>>,
    requests: Mutex<Vec<ChatRequest>>,
}

impl FakeLlm {
    pub fn single_text(content: impl Into<String>) -> Self {
        let content = content.into();
        Self {
            model: "fake-model".to_string(),
            scripted: Mutex::new(VecDeque::from([ScriptedResponse {
                chunks: vec![Chunk::content(content.clone())],
                finish_reason: FinishReason::Stop,
                message_content: content,
            }])),
            requests: Mutex::new(Vec::new()),
        }
    }

    pub fn tool_call_then_text(
        tool_name: impl Into<String>,
        arguments: Value,
        final_text: impl Into<String>,
    ) -> Self {
        let tool_name = tool_name.into();
        let final_text = final_text.into();
        let tool_call = ToolCall {
            id: "tool-call-1".to_string(),
            name: tool_name,
            arguments,
        };

        Self {
            model: "fake-model".to_string(),
            scripted: Mutex::new(VecDeque::from([
                ScriptedResponse {
                    chunks: vec![Chunk::with_tool_call(tool_call)],
                    finish_reason: FinishReason::ToolCalls,
                    message_content: String::new(),
                },
                ScriptedResponse {
                    chunks: vec![Chunk::content(final_text.clone())],
                    finish_reason: FinishReason::Stop,
                    message_content: final_text,
                },
            ])),
            requests: Mutex::new(Vec::new()),
        }
    }

    pub fn recorded_requests(&self) -> Vec<ChatRequest> {
        self.requests
            .lock()
            .map(|requests| requests.clone())
            .unwrap_or_default()
    }

    fn pop_response(&self) -> Result<ScriptedResponse, llm::LlmError> {
        let mut scripted = self
            .scripted
            .lock()
            .map_err(|_| llm::LlmError::Streaming("fake llm mutex poisoned".to_string()))?;
        scripted.pop_front().ok_or_else(|| {
            llm::LlmError::InvalidResponse("no fake llm response scripted".to_string())
        })
    }
}

#[async_trait]
impl LlmClient for FakeLlm {
    async fn chat(&self, request: ChatRequest) -> llm::Result<ChatResponse> {
        if let Ok(mut requests) = self.requests.lock() {
            requests.push(request.clone());
        }
        let response = self.pop_response()?;
        Ok(ChatResponse {
            message: Message::assistant(response.message_content),
            usage: TokenUsage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            },
            finish_reason: response.finish_reason,
        })
    }

    async fn chat_streaming(
        &self,
        request: ChatRequest,
        callback: Box<dyn Fn(Chunk) + Send + 'static>,
    ) -> llm::Result<ChatResponse> {
        if let Ok(mut requests) = self.requests.lock() {
            requests.push(request.clone());
        }
        let response = self.pop_response()?;
        for chunk in response.chunks.clone() {
            callback(chunk);
        }

        Ok(ChatResponse {
            message: Message::assistant(response.message_content),
            usage: TokenUsage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            },
            finish_reason: response.finish_reason,
        })
    }

    fn max_tokens(&self) -> usize {
        4096
    }

    fn count_tokens(&self, text: &str) -> usize {
        text.len() / 4
    }

    fn model(&self) -> &str {
        &self.model
    }
}
