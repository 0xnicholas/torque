use crate::id::ExtensionId;

/// Execution mode for a hook point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookMode {
    /// Intercept — handler can modify input or reject.
    /// Modified values propagate to subsequent handlers.
    Intercept,
    /// Observational — handler is read-only.
    /// Return values are logged but do not affect flow.
    Observational,
}

/// Phase within the Torque execution lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookPhase {
    Pre,
    Post,
}

/// Metadata describing a single hook point.
#[derive(Debug, Clone)]
pub struct HookPointDef {
    pub name: &'static str,
    pub mode: HookMode,
    pub phase: HookPhase,
    pub description: &'static str,
}

/// Entry stored in the registry for a registered hook handler.
#[derive(Clone)]
pub struct HookHandlerEntry {
    pub extension_id: ExtensionId,
    pub handler: std::sync::Arc<dyn super::handler::HookHandler>,
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
    pub timeout: Option<std::time::Duration>,
}

impl std::fmt::Debug for HookHandlerEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HookHandlerEntry")
            .field("extension_id", &self.extension_id)
            .field("handler", &format_args!("HookHandler(...)"))
            .field("metadata", &self.metadata)
            .field("timeout", &self.timeout)
            .finish()
    }
}

// ── Predefined Hook Points ──────────────────────────────────

/// # Intercept Hooks
///
/// Handler modifications directly affect subsequent execution.

/// Tool call before invocation — can modify args or reject.
pub const TOOL_CALL: HookPointDef = HookPointDef {
    name: "tool_call",
    mode: HookMode::Intercept,
    phase: HookPhase::Pre,
    description: "Intercept tool call, can modify args or reject",
};

/// Tool result before returning to the agent — can modify result.
pub const TOOL_RESULT: HookPointDef = HookPointDef {
    name: "tool_result",
    mode: HookMode::Intercept,
    phase: HookPhase::Post,
    description: "Intercept tool result, can modify result",
};

/// Context before processing — can modify context content.
pub const CONTEXT: HookPointDef = HookPointDef {
    name: "context",
    mode: HookMode::Intercept,
    phase: HookPhase::Pre,
    description: "Intercept context, can modify context content",
};

/// # Observational Hooks
///
/// Handlers are read-only. Return values are logged but do not affect flow.

/// Turn start notification.
pub const TURN_START: HookPointDef = HookPointDef {
    name: "turn_start",
    mode: HookMode::Observational,
    phase: HookPhase::Pre,
    description: "Turn start notification",
};

/// Turn end notification.
pub const TURN_END: HookPointDef = HookPointDef {
    name: "turn_end",
    mode: HookMode::Observational,
    phase: HookPhase::Post,
    description: "Turn end notification",
};

/// Agent start notification.
pub const AGENT_START: HookPointDef = HookPointDef {
    name: "agent_start",
    mode: HookMode::Observational,
    phase: HookPhase::Pre,
    description: "Agent start notification",
};

/// Agent end notification.
pub const AGENT_END: HookPointDef = HookPointDef {
    name: "agent_end",
    mode: HookMode::Observational,
    phase: HookPhase::Post,
    description: "Agent end notification",
};

/// Execution start notification.
pub const EXECUTION_START: HookPointDef = HookPointDef {
    name: "execution_start",
    mode: HookMode::Observational,
    phase: HookPhase::Pre,
    description: "Execution start notification",
};

/// Execution end notification.
pub const EXECUTION_END: HookPointDef = HookPointDef {
    name: "execution_end",
    mode: HookMode::Observational,
    phase: HookPhase::Post,
    description: "Execution end notification",
};

/// Error notification.
pub const ERROR: HookPointDef = HookPointDef {
    name: "error",
    mode: HookMode::Observational,
    phase: HookPhase::Post,
    description: "Error notification",
};

/// Checkpoint notification.
pub const CHECKPOINT: HookPointDef = HookPointDef {
    name: "checkpoint",
    mode: HookMode::Observational,
    phase: HookPhase::Post,
    description: "Checkpoint notification",
};

/// Pre-compaction hook (observational).
///
/// Fired before a Session.compact() is applied, carrying optional
/// custom instructions and the current message count.
pub const PRE_COMPACTION: HookPointDef = HookPointDef {
    name: "pre_compaction",
    mode: HookMode::Observational,
    phase: HookPhase::Pre,
    description: "Pre-compaction notification",
};

/// Post-compaction hook (observational).
///
/// Fired after a Session.compact() completes, indicating whether
/// the compaction succeeded or was aborted.
pub const POST_COMPACTION: HookPointDef = HookPointDef {
    name: "post_compaction",
    mode: HookMode::Observational,
    phase: HookPhase::Post,
    description: "Post-compaction notification",
};

/// Delegation start notification.
pub const DELEGATION_START: HookPointDef = HookPointDef {
    name: "delegation_start",
    mode: HookMode::Observational,
    phase: HookPhase::Pre,
    description: "Delegation start notification",
};

/// Delegation complete notification.
pub const DELEGATION_COMPLETE: HookPointDef = HookPointDef {
    name: "delegation_complete",
    mode: HookMode::Observational,
    phase: HookPhase::Post,
    description: "Delegation complete notification",
};

/// All predefined hook point names.
pub const ALL_HOOK_NAMES: &[&str] = &[
    "tool_call", "tool_result", "context",
    "turn_start", "turn_end",
    "agent_start", "agent_end",
    "execution_start", "execution_end",
    "error", "checkpoint",
    "pre_compaction", "post_compaction",
    "delegation_start", "delegation_complete",
];

/// Look up a predefined hook by name.
pub fn get_hook_def(name: &str) -> Option<&'static HookPointDef> {
    match name {
        "tool_call" => Some(&TOOL_CALL),
        "tool_result" => Some(&TOOL_RESULT),
        "context" => Some(&CONTEXT),
        "turn_start" => Some(&TURN_START),
        "turn_end" => Some(&TURN_END),
        "agent_start" => Some(&AGENT_START),
        "agent_end" => Some(&AGENT_END),
        "execution_start" => Some(&EXECUTION_START),
        "execution_end" => Some(&EXECUTION_END),
        "error" => Some(&ERROR),
        "checkpoint" => Some(&CHECKPOINT),
        "pre_compaction" => Some(&PRE_COMPACTION),
        "post_compaction" => Some(&POST_COMPACTION),
        "delegation_start" => Some(&DELEGATION_START),
        "delegation_complete" => Some(&DELEGATION_COMPLETE),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_mode_debug_clone() {
        let mode = HookMode::Intercept;
        assert_eq!(format!("{:?}", mode), "Intercept");
        assert_eq!(mode, HookMode::Intercept);
        assert_ne!(mode, HookMode::Observational);
    }

    #[test]
    fn test_hook_phase() {
        assert_eq!(HookPhase::Pre as u8, 0u8);
        assert_eq!(HookPhase::Post as u8, 1u8);
    }

    #[test]
    fn test_predefined_hooks_have_correct_names() {
        assert_eq!(TOOL_CALL.name, "tool_call");
        assert_eq!(TOOL_CALL.mode, HookMode::Intercept);
        assert_eq!(TOOL_CALL.phase, HookPhase::Pre);

        assert_eq!(TOOL_RESULT.name, "tool_result");
        assert_eq!(TOOL_RESULT.mode, HookMode::Intercept);
        assert_eq!(TOOL_RESULT.phase, HookPhase::Post);

        assert_eq!(CONTEXT.name, "context");
        assert_eq!(CONTEXT.mode, HookMode::Intercept);

        assert_eq!(TURN_START.name, "turn_start");
        assert_eq!(TURN_START.mode, HookMode::Observational);

        assert_eq!(TURN_END.name, "turn_end");
        assert_eq!(TURN_END.mode, HookMode::Observational);

        assert_eq!(AGENT_START.name, "agent_start");
        assert_eq!(AGENT_END.name, "agent_end");
        assert_eq!(EXECUTION_START.name, "execution_start");
        assert_eq!(EXECUTION_END.name, "execution_end");
        assert_eq!(ERROR.name, "error");
        assert_eq!(CHECKPOINT.name, "checkpoint");
        assert_eq!(PRE_COMPACTION.name, "pre_compaction");
        assert_eq!(POST_COMPACTION.name, "post_compaction");
        assert_eq!(DELEGATION_START.name, "delegation_start");
        assert_eq!(DELEGATION_COMPLETE.name, "delegation_complete");
    }

    #[test]
    fn test_get_hook_def_found() {
        for name in ALL_HOOK_NAMES {
            let def = get_hook_def(name);
            assert!(def.is_some(), "hook '{}' should be found", name);
            assert_eq!(def.unwrap().name, *name);
        }
    }

    #[test]
    fn test_get_hook_def_not_found() {
        assert!(get_hook_def("nonexistent_hook").is_none());
    }

    #[test]
    fn test_all_hook_names_count() {
        assert_eq!(ALL_HOOK_NAMES.len(), 15);
    }

    #[test]
    fn test_hook_handler_entry_debug() {
        let entry = HookHandlerEntry {
            extension_id: ExtensionId::from_uuid(uuid::Uuid::nil()),
            handler: std::sync::Arc::new(crate::hook::handler::TestHandler::default()),
            metadata: std::collections::HashMap::new(),
            timeout: None,
        };
        let debug = format!("{:?}", entry);
        assert!(debug.contains("HookHandler(...)"));
        assert!(debug.contains("extension_id"));
    }
}
