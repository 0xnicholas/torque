use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<super::tools::ToolCall>>,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".into(),
            content: content.into(),
            tool_calls: None,
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
            tool_calls: None,
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".into(),
            content: content.into(),
            tool_calls: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub tools: Option<Vec<super::tools::ToolDef>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

impl ChatRequest {
    pub fn new(model: impl Into<String>, messages: Vec<Message>) -> Self {
        Self {
            model: model.into(),
            messages,
            tools: None,
            max_tokens: None,
            temperature: None,
            stream: None,
        }
    }

    pub fn with_tools(mut self, tools: Vec<super::tools::ToolDef>) -> Self {
        self.tools = Some(tools);
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: usize) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    Length,
    ContentFilter,
    ToolCalls,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub message: Message,
    pub usage: TokenUsage,
    pub finish_reason: FinishReason,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub content: String,
    pub tool_call: Option<super::tools::ToolCall>,
    pub is_final: bool,
}

impl Chunk {
    pub fn content(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            tool_call: None,
            is_final: false,
        }
    }

    pub fn with_tool_call(tool_call: super::tools::ToolCall) -> Self {
        Self {
            content: String::new(),
            tool_call: Some(tool_call),
            is_final: false,
        }
    }

    pub fn final_marker() -> Self {
        Self {
            content: String::new(),
            tool_call: None,
            is_final: true,
        }
    }
}

#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn chat(&self, request: ChatRequest) -> super::Result<ChatResponse>;

    async fn chat_streaming(
        &self,
        request: ChatRequest,
        callback: Box<dyn Fn(Chunk) + Send + 'static>,
    ) -> super::Result<ChatResponse>;

    fn max_tokens(&self) -> usize;

    fn count_tokens(&self, text: &str) -> usize;

    fn model(&self) -> &str;
}
