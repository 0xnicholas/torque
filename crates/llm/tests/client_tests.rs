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
