use thiserror::Error;

#[derive(Error, Debug)]
pub enum CheckpointerError {
    #[error("Checkpoint not found: {0}")]
    NotFound(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

pub type Result<T> = std::result::Result<T, CheckpointerError>;
