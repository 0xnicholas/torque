use llm::openai::OpenAiClient;

#[tokio::test]
async fn test_openai_client_initialization() {
    let client = OpenAiClient::new(
        "https://api.openai.com/v1".to_string(),
        "sk-test".to_string(),
        "gpt-4o-mini".to_string(),
    );

    assert_eq!(client.model(), "gpt-4o-mini");
}

#[test]
fn test_client_with_default_values() {
    let client = OpenAiClient::new(
        "https://api.openai.com/v1".to_string(),
        "test-key".to_string(),
        "gpt-4".to_string(),
    );

    // Verify defaults
    assert_eq!(client.max_tokens(), 4096);
}
