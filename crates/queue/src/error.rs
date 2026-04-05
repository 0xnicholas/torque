use thiserror::Error;

#[derive(Error, Debug)]
pub enum QueueError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Entry not found: {0}")]
    NotFound(String),

    #[error("Already locked: {0}")]
    AlreadyLocked(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueErrorKind {
    Database,
    NotFound,
    AlreadyLocked,
}
