use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::Result;
use crate::id::ExtensionId;

/// Message sent between Extensions via the Actor Channel.
#[derive(Debug, Clone)]
pub enum ExtensionMessage {
    /// Fire-and-forget command.
    Command {
        target: ExtensionId,
        action: ExtensionAction,
    },
    /// Request expecting a response.
    Request {
        request_id: Uuid,
        target: ExtensionId,
        action: ExtensionAction,
        timeout_ms: u64,
    },
    /// Response to a previous Request.
    Response {
        request_id: Uuid,
        status: ResponseStatus,
        result: ExtensionResponse,
    },
}

/// Actions that an Extension can request of another Extension.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExtensionAction {
    Execute {
        goal: String,
        instructions: Vec<String>,
    },
    Query {
        key: String,
    },
    SetState {
        key: String,
        value: serde_json::Value,
    },
    Custom {
        namespace: String,
        name: String,
        payload: serde_json::Value,
    },
}

/// Response from an Extension action invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionResponse {
    pub request_id: Uuid,
    pub status: ResponseStatus,
    pub result: Option<serde_json::Value>,
}

impl ExtensionResponse {
    pub fn ok(request_id: Uuid, result: Option<serde_json::Value>) -> Self {
        Self {
            request_id,
            status: ResponseStatus::Success,
            result,
        }
    }

    pub fn fail(request_id: Uuid, error: impl Into<String>) -> Self {
        Self {
            request_id,
            status: ResponseStatus::Failure(error.into()),
            result: None,
        }
    }
}

/// Status of a response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ResponseStatus {
    Success,
    Failure(String),
    Timeout,
    NotFound,
}

/// Type alias for a oneshot sender used by the request-reply pattern.
pub(crate) type ReplySender = tokio::sync::oneshot::Sender<Result<ExtensionResponse>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_action_execute() {
        let action = ExtensionAction::Execute {
            goal: "do something".into(),
            instructions: vec!["step 1".into(), "step 2".into()],
        };
        match action {
            ExtensionAction::Execute { goal, instructions } => {
                assert_eq!(goal, "do something");
                assert_eq!(instructions.len(), 2);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn extension_action_query() {
        let action = ExtensionAction::Query {
            key: "status".into(),
        };
        match action {
            ExtensionAction::Query { key } => assert_eq!(key, "status"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn extension_action_set_state() {
        let action = ExtensionAction::SetState {
            key: "count".into(),
            value: serde_json::json!(42),
        };
        match action {
            ExtensionAction::SetState { key, value } => {
                assert_eq!(key, "count");
                assert_eq!(value, 42);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn extension_action_custom() {
        let action = ExtensionAction::Custom {
            namespace: "my-ext".into(),
            name: "ping".into(),
            payload: serde_json::json!({ "data": "hello" }),
        };
        match action {
            ExtensionAction::Custom { namespace, name, payload } => {
                assert_eq!(namespace, "my-ext");
                assert_eq!(name, "ping");
                assert_eq!(payload["data"], "hello");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn extension_action_serde() {
        let action = ExtensionAction::Custom {
            namespace: "test".into(),
            name: "echo".into(),
            payload: serde_json::json!("hello"),
        };
        let json = serde_json::to_string(&action).unwrap();
        let deserialized: ExtensionAction = serde_json::from_str(&json).unwrap();
        match deserialized {
            ExtensionAction::Custom { name, .. } => assert_eq!(name, "echo"),
            _ => panic!("wrong variant after deserialization"),
        }
    }

    #[test]
    fn extension_message_command() {
        let target = ExtensionId::new();
        let msg = ExtensionMessage::Command {
            target,
            action: ExtensionAction::Query { key: "x".into() },
        };
        match msg {
            ExtensionMessage::Command { target: t, action } => {
                assert_eq!(t, target);
                match action {
                    ExtensionAction::Query { key } => assert_eq!(key, "x"),
                    _ => panic!("wrong action variant"),
                }
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn extension_message_request() {
        let request_id = Uuid::new_v4();
        let msg = ExtensionMessage::Request {
            request_id,
            target: ExtensionId::new(),
            action: ExtensionAction::Execute {
                goal: "test".into(),
                instructions: vec![],
            },
            timeout_ms: 1000,
        };
        match msg {
            ExtensionMessage::Request {
                request_id: rid,
                timeout_ms,
                ..
            } => {
                assert_eq!(rid, request_id);
                assert_eq!(timeout_ms, 1000);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn extension_message_response() {
        let request_id = Uuid::new_v4();
        let result = ExtensionResponse {
            request_id,
            status: ResponseStatus::Success,
            result: Some(serde_json::json!("done")),
        };
        let msg = ExtensionMessage::Response {
            request_id,
            status: ResponseStatus::Success,
            result: result.clone(),
        };
        match msg {
            ExtensionMessage::Response { status, .. } => {
                assert!(matches!(status, ResponseStatus::Success));
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn response_ok() {
        let rid = Uuid::new_v4();
        let resp = ExtensionResponse::ok(rid, Some(serde_json::json!("data")));
        assert_eq!(resp.request_id, rid);
        assert!(matches!(resp.status, ResponseStatus::Success));
        assert_eq!(resp.result, Some(serde_json::json!("data")));
    }

    #[test]
    fn response_fail() {
        let rid = Uuid::new_v4();
        let resp = ExtensionResponse::fail(rid, "something went wrong");
        assert_eq!(resp.request_id, rid);
        match resp.status {
            ResponseStatus::Failure(msg) => assert_eq!(msg, "something went wrong"),
            _ => panic!("expected failure"),
        }
        assert!(resp.result.is_none());
    }
}
