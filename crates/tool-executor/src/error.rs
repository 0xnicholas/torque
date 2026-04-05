use thiserror::Error;

#[derive(Error, Debug)]
pub enum ToolError {
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    #[error("Execution error: {0}")]
    Execution(String),

    #[error("Timeout: {0}")]
    Timeout(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolErrorKind {
    PermissionDenied,
    ToolNotFound,
    Execution,
    Timeout,
}
