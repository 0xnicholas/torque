use thiserror::Error;

#[derive(Error, Debug)]
pub enum ExecutorError {
    #[error("Database error: {0}")]
    Database(String),

    #[error("Queue error: {0}")]
    Queue(String),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Runtime error: {0}")]
    Runtime(String),

    #[error("Other error: {0}")]
    Other(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutorErrorKind {
    Database,
    Queue,
    Config,
    Runtime,
    Other,
}
