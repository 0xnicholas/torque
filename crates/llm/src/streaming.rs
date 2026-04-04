use super::client::Chunk;
use super::error::Result;

pub struct StreamingProcessor {
    buffer: String,
}

impl StreamingProcessor {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }

    pub fn process_line(&mut self, line: &str) -> Result<Option<Chunk>> {
        if line.starts_with("data: ") {
            let data = line.strip_prefix("data: ").unwrap();

            if data == "[DONE]" {
                return Ok(Some(Chunk::final_marker()));
            }

            #[derive(serde::Deserialize)]
            struct SSEvent {
                choices: Vec<SSChoice>,
            }

            #[derive(serde::Deserialize)]
            struct SSChoice {
                delta: Delta,
                finish_reason: Option<String>,
            }

            #[derive(serde::Deserialize)]
            struct Delta {
                content: Option<String>,
                #[serde(rename = "tool_calls")]
                tool_calls: Option<Vec<SSToolCall>>,
            }

            #[derive(serde::Deserialize)]
            struct SSToolCall {
                id: Option<String>,
                #[serde(rename = "function")]
                function: SSFunction,
            }

            #[derive(serde::Deserialize)]
            struct SSFunction {
                name: Option<String>,
                arguments: Option<String>,
            }

            match serde_json::from_str::<SSEvent>(data) {
                Ok(event) => {
                    if let Some(choice) = event.choices.into_iter().next() {
                        if let Some(content) = choice.delta.content {
                            return Ok(Some(Chunk::content(content)));
                        }

                        if let Some(tool_calls) = choice.delta.tool_calls {
                            for tc in tool_calls {
                                if let (Some(id), Some(name), Some(arguments)) =
                                    (tc.id, tc.function.name, tc.function.arguments)
                                {
                                    let args: serde_json::Value = serde_json::from_str(&arguments)
                                        .unwrap_or(serde_json::Value::Object(Default::default()));
                                    return Ok(Some(Chunk::with_tool_call(
                                        super::tools::ToolCall {
                                            id,
                                            name,
                                            arguments: args,
                                        },
                                    )));
                                }
                            }
                        }

                        if choice.finish_reason.is_some() {
                            return Ok(Some(Chunk::final_marker()));
                        }
                    }
                }
                Err(e) => {
                    tracing::debug!("Failed to parse SSE: {}", e);
                }
            }
        }

        Ok(None)
    }
}

impl Default for StreamingProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_content_line() {
        let mut processor = StreamingProcessor::new();
        let line = r#"data: {"choices":[{"delta":{"content":"Hello"}}]}"#;
        let result = processor.process_line(line).unwrap().unwrap();
        assert_eq!(result.content, "Hello");
        assert!(!result.is_final);
    }

    #[test]
    fn test_process_done() {
        let mut processor = StreamingProcessor::new();
        let result = processor.process_line("data: [DONE]").unwrap().unwrap();
        assert!(result.is_final);
    }

    #[test]
    fn test_process_tool_call() {
        let mut processor = StreamingProcessor::new();
        let line = r#"data: {"choices":[{"delta":{"tool_calls":[{"id":"call_1","function":{"name":"test","arguments":"{}"}}]}}]}"#;
        let result = processor.process_line(line).unwrap().unwrap();
        assert!(result.tool_call.is_some());
        let tc = result.tool_call.unwrap();
        assert_eq!(tc.id, "call_1");
        assert_eq!(tc.name, "test");
    }
}
