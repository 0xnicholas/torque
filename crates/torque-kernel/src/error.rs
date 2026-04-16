use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("{entity} validation failed: {message}")]
pub struct ValidationError {
    entity: &'static str,
    message: String,
}

impl ValidationError {
    pub fn new(entity: &'static str, message: impl Into<String>) -> Self {
        Self {
            entity,
            message: message.into(),
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("invalid {entity} state transition from {from} to {to}")]
pub struct StateTransitionError {
    entity: &'static str,
    from: String,
    to: String,
}

impl StateTransitionError {
    pub fn new(entity: &'static str, from: impl Into<String>, to: impl Into<String>) -> Self {
        Self {
            entity,
            from: from.into(),
            to: to.into(),
        }
    }
}

#[derive(Debug, Error)]
pub enum KernelError {
    #[error(transparent)]
    Validation(#[from] ValidationError),
    #[error(transparent)]
    StateTransition(#[from] StateTransitionError),
}
