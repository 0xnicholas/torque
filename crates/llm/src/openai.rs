use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use std::collections::BTreeMap;

use super::client::{
    ChatRequest, ChatResponse, Chunk, FinishReason, LlmClient, Message, TokenUsage,
};
use super::error::{LlmError, Result};
use super::tools::ToolCall;

const DEFAULT_MAX_TOKENS: usize = 4096;

pub struct OpenAiClient {
    http_client: Client,
    base_url: String,
    api_key: String,
    default_model: String,
}

impl OpenAiClient {
    pub fn new(base_url: String, api_key: String, default_model: String) -> Self {
        Self {
            http_client: Client::new(),
            base_url,
            api_key,
            default_model,
        }
    }

    pub fn from_env() -> Result<Self> {
        let base_url = std::env::var("LLM_BASE_URL")
            .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
        let api_key = std::env::var("LLM_API_KEY")
            .map_err(|_| LlmError::Config("LLM_API_KEY not set".to_string()))?;
        let default_model =
            std::env::var("LLM_AGENT_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());

        Ok(Self::new(base_url, api_key, default_model))
    }

    fn build_request(&self, request: ChatRequest) -> serde_json::Value {
        let mut body = serde_json::json!({
            "model": request.model,
            "messages": request.messages,
        });

        if let Some(tools) = request.tools {
            body["tools"] = serde_json::json!(tools
                .into_iter()
                .map(|t| {
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.parameters,
                        }
                    })
                })
                .collect::<Vec<_>>());
        }

        if let Some(max_tokens) = request.max_tokens {
            body["max_tokens"] = max_tokens.into();
        }

        if let Some(temperature) = request.temperature {
            body["temperature"] = temperature.into();
        }

        body
    }

    fn parse_finish_reason(value: Option<&str>) -> FinishReason {
        match value {
            Some("stop") => FinishReason::Stop,
            Some("length") => FinishReason::Length,
            Some("content_filter") => FinishReason::ContentFilter,
            Some("tool_calls") => FinishReason::ToolCalls,
            _ => FinishReason::Stop,
        }
    }
}

#[async_trait]
impl LlmClient for OpenAiClient {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        let url = format!("{}/chat/completions", self.base_url);
        let body = self.build_request(request);

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = response.status();

        if status.as_u16() == 401 {
            return Err(LlmError::AuthenticationFailed);
        }

        if status.as_u16() == 429 {
            return Err(LlmError::RateLimitExceeded);
        }

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(LlmError::InvalidResponse(format!(
                "Status {}: {}",
                status.as_u16(),
                error_text
            )));
        }

        #[derive(Deserialize)]
        struct ResponseBody {
            choices: Vec<Choice>,
            usage: Usage,
        }

        #[derive(Deserialize)]
        struct Choice {
            message: ResponseMessage,
            finish_reason: String,
        }

        #[derive(Deserialize)]
        struct ResponseMessage {
            role: String,
            content: Option<String>,
            #[serde(rename = "tool_calls")]
            tool_calls: Option<Vec<ToolCallResponse>>,
        }

        #[derive(Deserialize)]
        struct ToolCallResponse {
            id: String,
            #[serde(rename = "function")]
            function: FunctionResponse,
        }

        #[derive(Deserialize)]
        struct FunctionResponse {
            name: String,
            arguments: String,
        }

        #[derive(Deserialize)]
        struct Usage {
            #[serde(rename = "prompt_tokens")]
            prompt_tokens: i64,
            #[serde(rename = "completion_tokens")]
            completion_tokens: i64,
            #[serde(rename = "total_tokens")]
            total_tokens: i64,
        }

        let body: ResponseBody = response.json().await?;

        let (message, finish_reason_str) = if let Some(choice) = body.choices.into_iter().next() {
            let content = choice.message.content.unwrap_or_default();
            let reason = choice.finish_reason;
            let tool_calls = choice.message.tool_calls.map(|calls| {
                calls
                    .into_iter()
                    .map(|tc| ToolCall {
                        id: tc.id,
                        name: tc.function.name,
                        arguments: serde_json::from_str(&tc.function.arguments)
                            .unwrap_or(serde_json::Value::Object(Default::default())),
                    })
                    .collect()
            });
            let msg = Message {
                role: choice.message.role,
                content,
                tool_calls,
            };
            (msg, reason)
        } else {
            return Err(LlmError::InvalidResponse(
                "No choices in response".to_string(),
            ));
        };

        let finish_reason = Self::parse_finish_reason(Some(finish_reason_str.as_str()));

        Ok(ChatResponse {
            message,
            usage: TokenUsage {
                prompt_tokens: body.usage.prompt_tokens,
                completion_tokens: body.usage.completion_tokens,
                total_tokens: body.usage.total_tokens,
            },
            finish_reason,
        })
    }

    async fn chat_streaming(
        &self,
        request: ChatRequest,
        callback: Box<dyn Fn(Chunk) + Send + 'static>,
    ) -> Result<ChatResponse> {
        let mut request = request;
        request.stream = Some(true);

        let url = format!("{}/chat/completions", self.base_url);
        let body = self.build_request(request);

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = response.status();

        if status.as_u16() == 401 {
            return Err(LlmError::AuthenticationFailed);
        }

        if status.as_u16() == 429 {
            return Err(LlmError::RateLimitExceeded);
        }

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(LlmError::InvalidResponse(format!(
                "Status {}: {}",
                status.as_u16(),
                error_text
            )));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| LlmError::Streaming(format!("Failed to read response: {}", e)))?;

        let text = String::from_utf8_lossy(&bytes);
        let lines: Vec<&str> = text.lines().collect();

        #[derive(Default, Clone)]
        struct ToolCallAccumulator {
            id: String,
            name: String,
            arguments: String,
        }

        let mut full_content = String::new();
        let mut finish_reason = FinishReason::Stop;
        let mut tool_calls_by_index: BTreeMap<usize, ToolCallAccumulator> = BTreeMap::new();

        for line in lines {
            if line.starts_with("data: ") {
                let data = line.strip_prefix("data: ").unwrap();
                if data == "[DONE]" {
                    callback(Chunk::final_marker());
                    break;
                }

                #[derive(Deserialize)]
                struct SSEChunk {
                    choices: Vec<SSEChoice>,
                }

                #[derive(Deserialize)]
                struct SSEChoice {
                    delta: Delta,
                    #[serde(rename = "finish_reason")]
                    finish_reason: Option<String>,
                }

                #[derive(Deserialize)]
                struct Delta {
                    content: Option<String>,
                    #[serde(rename = "tool_calls")]
                    tool_calls: Option<Vec<SSEtoolCall>>,
                }

                #[derive(Deserialize)]
                struct SSEtoolCall {
                    index: Option<usize>,
                    id: Option<String>,
                    #[serde(rename = "function")]
                    function: Option<SSEFunction>,
                }

                #[derive(Deserialize)]
                struct SSEFunction {
                    name: Option<String>,
                    arguments: Option<String>,
                }

                match serde_json::from_str::<SSEChunk>(data) {
                    Ok(chunk) => {
                        if let Some(choice) = chunk.choices.into_iter().next() {
                            if let Some(content) = choice.delta.content {
                                full_content.push_str(&content);
                                callback(Chunk::content(content));
                            }
                            if let Some(tool_calls) = choice.delta.tool_calls {
                                for (fallback_index, tc) in tool_calls.into_iter().enumerate() {
                                    let index = tc.index.unwrap_or(fallback_index);
                                    let acc = tool_calls_by_index.entry(index).or_default();

                                    if let Some(id) = tc.id {
                                        acc.id = id;
                                    }
                                    if let Some(function) = tc.function {
                                        if let Some(name) = function.name {
                                            acc.name.push_str(&name);
                                        }
                                        if let Some(arguments) = function.arguments {
                                            acc.arguments.push_str(&arguments);
                                        }
                                    }
                                }
                            }
                            if choice.finish_reason.is_some() {
                                finish_reason =
                                    Self::parse_finish_reason(choice.finish_reason.as_deref());
                            }
                        }
                    }
                    Err(_) => {}
                }
            }
        }

        for (_, acc) in tool_calls_by_index {
            if acc.name.is_empty() {
                continue;
            }
            let arguments = serde_json::from_str(&acc.arguments)
                .unwrap_or(serde_json::Value::Object(Default::default()));
            let id = if acc.id.is_empty() {
                format!("tool-call-{}", acc.name)
            } else {
                acc.id
            };
            callback(Chunk::with_tool_call(ToolCall {
                id,
                name: acc.name,
                arguments,
            }));
        }

        Ok(ChatResponse {
            message: Message::assistant(full_content),
            usage: TokenUsage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            },
            finish_reason,
        })
    }

    fn max_tokens(&self) -> usize {
        DEFAULT_MAX_TOKENS
    }

    fn count_tokens(&self, text: &str) -> usize {
        text.len() / 4
    }

    fn model(&self) -> &str {
        &self.default_model
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_helpers() {
        let sys = Message::system("You are helpful");
        assert_eq!(sys.role, "system");
        assert_eq!(sys.content, "You are helpful");

        let user = Message::user("Hello");
        assert_eq!(user.role, "user");

        let assistant = Message::assistant("Hi there");
        assert_eq!(assistant.role, "assistant");
    }

    #[test]
    fn test_chat_request_builder() {
        let request = ChatRequest::new("gpt-4", vec![Message::user("test")])
            .with_max_tokens(1000)
            .with_temperature(0.7);

        assert_eq!(request.model, "gpt-4");
        assert_eq!(request.max_tokens, Some(1000));
        assert_eq!(request.temperature, Some(0.7));
    }
}
