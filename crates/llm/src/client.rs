use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<super::tools::ToolCall>>,
    /// Required for tool-result messages: the `id` of the tool call
    /// this result responds to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Optional name to distinguish participants with the same role
    /// (e.g. multiple agents using "assistant").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl Message {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".into(),
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".into(),
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    /// Create a tool-result message responding to a specific tool call.
    pub fn tool(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: "tool".into(),
            content: content.into(),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
            name: None,
        }
    }

    /// Set the optional name field (builder style).
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}

/// Controls structured output mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseFormat {
    /// Standard text response (default).
    Text,
    /// Guaranteed valid JSON (no schema enforcement).
    JsonObject,
    /// Structured outputs with a JSON schema.
    JsonSchema {
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        schema: serde_json::Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        strict: Option<bool>,
    },
}

impl Default for ResponseFormat {
    fn default() -> Self {
        Self::Text
    }
}

/// Controls how tools are selected during inference.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolChoice {
    /// Let the model decide (default).
    Auto,
    /// Force at least one tool call.
    Required,
    /// No tool calls allowed.
    None,
    /// Force a specific tool by name.
    Specific { r#type: String, function: ToolChoiceName },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolChoiceName {
    pub name: String,
}

impl ToolChoice {
    pub fn specific(name: impl Into<String>) -> Self {
        Self::Specific {
            r#type: "function".into(),
            function: ToolChoiceName {
                name: name.into(),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<super::tools::ToolDef>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    /// Structured output mode (JSON mode, JSON schema, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,
    /// Controls tool calling behavior.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
    /// Nucleus sampling (0.0–1.0). Lower = more focused.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    /// Deterministic sampling seed (OpenAI).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<i64>,
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
            response_format: None,
            tool_choice: None,
            top_p: None,
            seed: None,
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

    pub fn with_response_format(mut self, fmt: ResponseFormat) -> Self {
        self.response_format = Some(fmt);
        self
    }

    pub fn with_tool_choice(mut self, choice: ToolChoice) -> Self {
        self.tool_choice = Some(choice);
        self
    }

    pub fn with_top_p(mut self, top_p: f32) -> Self {
        self.top_p = Some(top_p);
        self
    }

    pub fn with_seed(mut self, seed: i64) -> Self {
        self.seed = Some(seed);
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
    /// Unique response identifier from the provider.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// The model that actually produced this response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
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

    /// List available models from this provider.
    /// Default implementation returns an empty list.
    async fn list_models(&self) -> super::Result<Vec<String>> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_tool_serialization() {
        let msg = Message::tool("call_abc123", "result content");
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("call_abc123"));
        assert!(json.contains("tool"));
    }

    #[test]
    fn message_name_field() {
        let msg = Message::system("hello").with_name("supervisor");
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("supervisor"));
    }

    #[test]
    fn chat_request_full() {
        let req = ChatRequest::new("gpt-4o", vec![Message::user("hi")])
            .with_tools(vec![])
            .with_max_tokens(1000)
            .with_temperature(0.7)
            .with_response_format(ResponseFormat::JsonObject)
            .with_top_p(0.9)
            .with_seed(42);
        assert!(req.response_format.is_some());
        assert!(req.top_p.is_some());
        assert!(req.seed.is_some());
    }

    #[test]
    fn tool_choice_specific() {
        let choice = ToolChoice::specific("my_function");
        let json = serde_json::to_string(&choice).unwrap();
        assert!(json.contains("my_function"));
    }
}
