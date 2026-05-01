use thiserror::Error;

use crate::id::ExtensionId;

/// Errors that can occur during Extension lifecycle or operation.
#[derive(Debug, Error)]
pub enum ExtensionError {
    #[error("extension not found: {0}")]
    NotFound(ExtensionId),

    #[error("extension already registered: {0}")]
    AlreadyRegistered(ExtensionId),

    #[error("extension {0} timed out")]
    Timeout(ExtensionId),

    #[error("hook '{hook}' rejected: {reason}")]
    HookRejected {
        hook: &'static str,
        reason: String,
    },

    #[error("hook '{hook}' handler panicked: {message}")]
    HookPanicked {
        hook: &'static str,
        message: String,
    },

    #[error("lifecycle error: {0}")]
    LifecycleError(String),

    #[error("runtime error: {0}")]
    RuntimeError(String),

    #[error("serialization error: {0}")]
    SerializationError(String),

    #[error("subscription not found: {0}")]
    SubscriptionNotFound(String),

    #[error("topic not found: {0}")]
    TopicNotFound(String),

    #[error("handler panicked: {0}")]
    Panicked(String),

    #[error("extension {0} is in an invalid state for this operation")]
    InvalidState(ExtensionId),

    #[error("message target not found: {0}")]
    TargetNotFound(ExtensionId),
}

/// Convenience alias.
pub type Result<T> = std::result::Result<T, ExtensionError>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::ExtensionId;
    use uuid::Uuid;

    fn make_id() -> ExtensionId {
        ExtensionId::from_uuid(Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap())
    }

    #[test]
    fn error_not_found() {
        let id = make_id();
        let err = ExtensionError::NotFound(id);
        assert_eq!(err.to_string(), format!("extension not found: {id}"));
    }

    #[test]
    fn error_already_registered() {
        let id = make_id();
        let err = ExtensionError::AlreadyRegistered(id);
        assert_eq!(err.to_string(), format!("extension already registered: {id}"));
    }

    #[test]
    fn error_timeout() {
        let id = make_id();
        let err = ExtensionError::Timeout(id);
        assert_eq!(err.to_string(), format!("extension {id} timed out"));
    }

    #[test]
    fn error_hook_rejected() {
        let err = ExtensionError::HookRejected {
            hook: "tool_call",
            reason: "not allowed".into(),
        };
        assert_eq!(
            err.to_string(),
            "hook 'tool_call' rejected: not allowed"
        );
    }

    #[test]
    fn error_hook_panicked() {
        let err = ExtensionError::HookPanicked {
            hook: "turn_start",
            message: "index out of bounds".into(),
        };
        assert_eq!(
            err.to_string(),
            "hook 'turn_start' handler panicked: index out of bounds"
        );
    }

    #[test]
    fn error_lifecycle_error() {
        let err = ExtensionError::LifecycleError("invalid transition".into());
        assert_eq!(err.to_string(), "lifecycle error: invalid transition");
    }

    #[test]
    fn error_runtime_error() {
        let err = ExtensionError::RuntimeError("channel closed".into());
        assert_eq!(err.to_string(), "runtime error: channel closed");
    }

    #[test]
    fn error_serialization() {
        let err = ExtensionError::SerializationError("invalid json".into());
        assert_eq!(err.to_string(), "serialization error: invalid json");
    }

    #[test]
    fn error_subscription_not_found() {
        let err = ExtensionError::SubscriptionNotFound("sub-123".into());
        assert_eq!(err.to_string(), "subscription not found: sub-123");
    }

    #[test]
    fn error_topic_not_found() {
        let err = ExtensionError::TopicNotFound("ext:unknown".into());
        assert_eq!(err.to_string(), "topic not found: ext:unknown");
    }

    #[test]
    fn error_panicked() {
        let err = ExtensionError::Panicked("handler crashed".into());
        assert_eq!(err.to_string(), "handler panicked: handler crashed");
    }

    #[test]
    fn error_invalid_state() {
        let id = make_id();
        let err = ExtensionError::InvalidState(id);
        assert_eq!(
            err.to_string(),
            format!("extension {id} is in an invalid state for this operation")
        );
    }

    #[test]
    fn error_target_not_found() {
        let id = make_id();
        let err = ExtensionError::TargetNotFound(id);
        assert_eq!(err.to_string(), format!("message target not found: {id}"));
    }

    #[test]
    fn result_type_alias() {
        let ok: Result<i32> = Ok(42);
        assert_eq!(ok.unwrap(), 42);
    }
}
