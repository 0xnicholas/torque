use llm::{ChatRequest, FinishReason, LlmClient, Message, OpenAiClient};

#[tokio::test]
async fn test_chat_returns_tool_calls() {
    let mut server = mockito::Server::new_async().await;

    let mock = server
        .mock("POST", "/chat/completions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_abc123",
                        "type": "function",
                        "function": {
                            "name": "get_weather",
                            "arguments": "{\"location\":\"NYC\"}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
        }"#)
        .create_async()
        .await;

    let client = OpenAiClient::new(
        server.url(),
        "test-key".to_string(),
        "gpt-4".to_string(),
    );

    let request = ChatRequest::new("gpt-4", vec![Message::user("weather in NYC?")]);
    let response = client.chat(request).await.unwrap();

    assert_eq!(response.finish_reason, FinishReason::ToolCalls);
    assert!(response.message.tool_calls.is_some());
    let tool_calls = response.message.tool_calls.unwrap();
    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls[0].id, "call_abc123");
    assert_eq!(tool_calls[0].name, "get_weather");
    assert_eq!(tool_calls[0].arguments, serde_json::json!({"location": "NYC"}));

    mock.assert_async().await;
}

#[tokio::test]
async fn test_chat_streaming_returns_rate_limit_on_429() {
    let mut server = mockito::Server::new_async().await;

    let mock = server
        .mock("POST", "/chat/completions")
        .with_status(429)
        .with_body(r#"{"error": {"message": "Too many requests"}}"#)
        .create_async()
        .await;

    let client = OpenAiClient::new(
        server.url(),
        "test-key".to_string(),
        "gpt-4".to_string(),
    );

    let request = ChatRequest::new("gpt-4", vec![Message::user("Hello")]);
    let result = client.chat_streaming(request, Box::new(|_| {})).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        llm::LlmError::RateLimitExceeded => {}
        other => panic!("Expected RateLimitExceeded, got {:?}", other),
    }

    mock.assert_async().await;
}

#[tokio::test]
async fn test_chat_streaming_returns_authentication_failed_on_401() {
    let mut server = mockito::Server::new_async().await;

    let mock = server
        .mock("POST", "/chat/completions")
        .with_status(401)
        .with_body(r#"{"error": {"message": "Invalid API key"}}"#)
        .create_async()
        .await;

    let client = OpenAiClient::new(
        server.url(),
        "bad-key".to_string(),
        "gpt-4".to_string(),
    );

    let request = ChatRequest::new("gpt-4", vec![Message::user("Hello")]);
    let result = client.chat_streaming(request, Box::new(|_| {})).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        llm::LlmError::AuthenticationFailed => {}
        other => panic!("Expected AuthenticationFailed, got {:?}", other),
    }

    mock.assert_async().await;
}
