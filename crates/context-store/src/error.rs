use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContextStoreError {
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),

    #[error("S3 error: {0}")]
    S3(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Not found: {0}")]
    NotFound(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextStoreErrorKind {
    Redis,
    S3,
    Serialization,
    NotFound,
}
