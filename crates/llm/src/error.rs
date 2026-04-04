use thiserror::Error;

#[derive(Error, Debug)]
pub enum LlmError {
    #[error("Request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),

    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    #[error("API error: {code} - {message}")]
    ApiError { code: i64, message: String },

    #[error("Authentication failed")]
    AuthenticationFailed,

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Context length exceeded")]
    ContextLengthExceeded,

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Streaming error: {0}")]
    Streaming(String),

    #[error("Configuration error: {0}")]
    Config(String),
}

pub type Result<T> = std::result::Result<T, LlmError>;

impl LlmError {
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            LlmError::RateLimitExceeded | LlmError::RequestFailed(_)
        )
    }
}
