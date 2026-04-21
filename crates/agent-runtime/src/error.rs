use thiserror::Error;

#[derive(Error, Debug)]
pub enum AgentError {
    #[error("LLM error: {0}")]
    Llm(String),

    #[error("Tool execution error: {0}")]
    Tool(String),

    #[error("Context error: {0}")]
    Context(String),

    #[error("Max iterations exceeded")]
    MaxIterations,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentErrorKind {
    Llm,
    Tool,
    Context,
    MaxIterations,
}
