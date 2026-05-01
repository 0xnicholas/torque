use torque_kernel::{ExecutionRequest, ExecutionResult};

use crate::id::ExtensionId;

/// Input data delivered to a hook handler.
///
/// Types marked with `(serde_json::Value)` are placeholders for
/// kernel types that will be defined in future phases.
#[derive(Debug, Clone)]
pub enum HookInput {
    // ── Intercept Hooks ──────────────────────────────────────
    ToolCall {
        tool: serde_json::Value,
        args: serde_json::Value,
    },
    ToolResult {
        tool: serde_json::Value,
        result: serde_json::Value,
    },
    Context {
        content: serde_json::Value,
    },

    // ── Observational Hooks ──────────────────────────────────
    TurnStart {
        turn_number: u32,
    },
    TurnEnd {
        turn_number: u32,
        response: serde_json::Value,
    },
    AgentStart {
        agent_id: ExtensionId,
        request: ExecutionRequest,
    },
    AgentEnd {
        agent_id: ExtensionId,
        result: ExecutionResult,
    },
    ExecutionStart {
        request: ExecutionRequest,
    },
    ExecutionEnd {
        result: ExecutionResult,
    },
    Error {
        error: serde_json::Value,
    },
    Checkpoint {
        checkpoint: serde_json::Value,
    },
    DelegationStart {
        delegation_id: uuid::Uuid,
    },
    DelegationComplete {
        delegation_id: uuid::Uuid,
        result: serde_json::Value,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use torque_kernel::{AgentDefinitionId, ExecutionRequest};
    use crate::id::ExtensionId;

    #[test]
    fn test_hook_input_tool_call() {
        let input = HookInput::ToolCall {
            tool: serde_json::json!("search"),
            args: serde_json::json!({}),
        };
        match input {
            HookInput::ToolCall { tool, .. } => assert_eq!(tool, serde_json::json!("search")),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_hook_input_tool_result() {
        let input = HookInput::ToolResult {
            tool: serde_json::json!("search"),
            result: serde_json::json!(["result1", "result2"]),
        };
        match input {
            HookInput::ToolResult { result, .. } => {
                assert_eq!(result, serde_json::json!(["result1", "result2"]))
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_hook_input_context() {
        let input = HookInput::Context {
            content: serde_json::json!({ "key": "value" }),
        };
        match input {
            HookInput::Context { content } => {
                assert_eq!(content["key"], "value")
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_hook_input_turn() {
        let start = HookInput::TurnStart { turn_number: 1 };
        match start {
            HookInput::TurnStart { turn_number } => assert_eq!(turn_number, 1),
            _ => panic!("wrong variant"),
        }

        let end = HookInput::TurnEnd {
            turn_number: 1,
            response: serde_json::json!("ok"),
        };
        match end {
            HookInput::TurnEnd { turn_number, response } => {
                assert_eq!(turn_number, 1);
                assert_eq!(response, "ok");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_hook_input_agent() {
        let agent_id = ExtensionId::new();
        let request = ExecutionRequest::new(
            AgentDefinitionId::new(),
            "test goal",
            vec!["step 1".to_string()],
        );

        let start = HookInput::AgentStart {
            agent_id,
            request: request.clone(),
        };
        match start {
            HookInput::AgentStart {
                agent_id: id,
                request: req,
            } => {
                assert_eq!(id, agent_id);
                assert_eq!(req.goal(), "test goal");
            }
            _ => panic!("wrong variant"),
        }

        let result = torque_kernel::ExecutionResult {
            instance_id: torque_kernel::AgentInstanceId::new(),
            task_id: torque_kernel::TaskId::new(),
            sequence_number: 0,
            outcome: torque_kernel::ExecutionOutcome::Continue,
            instance_state: torque_kernel::AgentInstanceState::Completed,
            task_state: torque_kernel::TaskState::Done,
            artifact_ids: Vec::new(),
            approval_request_ids: Vec::new(),
            delegation_request_ids: Vec::new(),
            events: Vec::new(),
            summary: None,
        };

        let end = HookInput::AgentEnd {
            agent_id,
            result,
        };
        match end {
            HookInput::AgentEnd { agent_id: id, .. } => assert_eq!(id, agent_id),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_hook_input_execution() {
        let request = ExecutionRequest::new(
            AgentDefinitionId::new(),
            "exec goal",
            vec![],
        );

        let start = HookInput::ExecutionStart {
            request: request.clone(),
        };
        match start {
            HookInput::ExecutionStart { request: req } => assert_eq!(req.goal(), "exec goal"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_hook_input_error() {
        let input = HookInput::Error {
            error: serde_json::json!("something went wrong"),
        };
        match input {
            HookInput::Error { error } => assert_eq!(error, "something went wrong"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_hook_input_checkpoint() {
        let input = HookInput::Checkpoint {
            checkpoint: serde_json::json!({ "id": "cp1" }),
        };
        match input {
            HookInput::Checkpoint { checkpoint } => assert_eq!(checkpoint["id"], "cp1"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_hook_input_delegation() {
        let id = uuid::Uuid::new_v4();
        let start = HookInput::DelegationStart {
            delegation_id: id,
        };
        match start {
            HookInput::DelegationStart { delegation_id } => assert_eq!(delegation_id, id),
            _ => panic!("wrong variant"),
        }

        let complete = HookInput::DelegationComplete {
            delegation_id: id,
            result: serde_json::json!("done"),
        };
        match complete {
            HookInput::DelegationComplete {
                delegation_id,
                result,
            } => {
                assert_eq!(delegation_id, id);
                assert_eq!(result, "done");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_hook_input_clone() {
        let input = HookInput::ToolCall {
            tool: serde_json::json!("test"),
            args: serde_json::json!({}),
        };
        let cloned = input.clone();
        match cloned {
            HookInput::ToolCall { tool, .. } => assert_eq!(tool, "test"),
            _ => panic!("wrong variant"),
        }
    }
}
