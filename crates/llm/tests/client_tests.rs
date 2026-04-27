use llm::{ChatRequest, LlmClient, Message, OpenAiClient};

#[tokio::test]
async fn test_openai_client_from_env_missing_key() {
    // Clear env vars to ensure test isolation
    std::env::remove_var("LLM_API_KEY");
    std::env::remove_var("LLM_BASE_URL");
    std::env::remove_var("LLM_AGENT_MODEL");

    let result = OpenAiClient::from_env();
    assert!(result.is_err());
}

#[tokio::test]
async fn test_openai_client_builder() {
    let client = OpenAiClient::new(
        "https://api.openai.com/v1".to_string(),
        "test-key".to_string(),
        "gpt-4".to_string(),
    );

    assert_eq!(client.model(), "gpt-4");
    assert_eq!(client.max_tokens(), 4096);
}

#[test]
fn test_chat_request_builder() {
    let request = ChatRequest::new("gpt-4", vec![Message::user("Hello")])
        .with_max_tokens(1000)
        .with_temperature(0.5);

    assert_eq!(request.model, "gpt-4");
    assert_eq!(request.messages.len(), 1);
    assert_eq!(request.max_tokens, Some(1000));
    assert_eq!(request.temperature, Some(0.5));
}

#[test]
fn test_message_helpers() {
    let sys = Message::system("You are a helpful assistant");
    assert_eq!(sys.role, "system");
    assert_eq!(sys.content, "You are a helpful assistant");

    let user = Message::user("What is rust?");
    assert_eq!(user.role, "user");

    let assistant = Message::assistant("Rust is...");
    assert_eq!(assistant.role, "assistant");
}

#[test]
fn test_token_estimation() {
    let client = OpenAiClient::new(
        "https://api.openai.com/v1".to_string(),
        "test-key".to_string(),
        "gpt-4".to_string(),
    );

    // Rough estimation: 1 token ≈ 4 characters
    let text = "Hello, world! This is a test.";
    let estimated = client.count_tokens(text);
    assert_eq!(estimated, text.len() / 4);
}

#[test]
fn test_request_serialization() {
    let request = ChatRequest::new("gpt-4", vec![Message::user("Hi")]).with_max_tokens(100);

    let json = serde_json::to_string(&request).unwrap();
    assert!(json.contains("\"model\":\"gpt-4\""));
    assert!(json.contains("\"max_tokens\":100"));
    assert!(!json.contains("\"temperature\":")); // Should be omitted when None
}

#[test]
fn test_message_with_tool_calls_serialization() {
    let tool_call = llm::ToolCall {
        id: "call_1".to_string(),
        name: "test_func".to_string(),
        arguments: serde_json::json!({"key": "value"}),
    };
    let msg = llm::Message {
        role: "assistant".to_string(),
        content: String::new(),
        tool_calls: Some(vec![tool_call]),
    };

    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains("\"tool_calls\""));
    assert!(json.contains("\"call_1\""));
    assert!(json.contains("\"test_func\""));
}

#[test]
fn test_message_without_tool_calls_omitted() {
    let msg = llm::Message {
        role: "user".to_string(),
        content: "Hello".to_string(),
        tool_calls: None,
    };

    let json = serde_json::to_string(&msg).unwrap();
    assert!(!json.contains("tool_calls"));
}

#[test]
fn test_message_tool_calls_deserialization() {
    let json = r#"{"role":"assistant","content":"","tool_calls":[{"id":"call_1","name":"test_func","arguments":{"key":"value"}}]}"#;
    let msg: llm::Message = serde_json::from_str(json).unwrap();
    assert_eq!(msg.role, "assistant");
    assert!(msg.tool_calls.is_some());
    let calls = msg.tool_calls.unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].id, "call_1");
}
