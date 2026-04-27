#[derive(Debug, Clone)]
pub struct CheckpointConfig {
    pub max_events_per_checkpoint: usize,
    pub max_seconds_per_checkpoint: u64,
    pub checkpoint_on_await_states: bool,
    pub checkpoint_on_tool_call: bool,
}

impl Default for CheckpointConfig {
    fn default() -> Self {
        Self {
            max_events_per_checkpoint: 50,
            max_seconds_per_checkpoint: 300,
            checkpoint_on_await_states: true,
            checkpoint_on_tool_call: true,
        }
    }
}

impl CheckpointConfig {
    pub fn from_env() -> Self {
        let default = Self::default();
        Self {
            max_events_per_checkpoint: std::env::var("CHECKPOINT_MAX_EVENTS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(default.max_events_per_checkpoint),
            max_seconds_per_checkpoint: std::env::var("CHECKPOINT_MAX_SECONDS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(default.max_seconds_per_checkpoint),
            checkpoint_on_await_states: std::env::var("CHECKPOINT_ON_AWAIT")
                .map(|v| v != "false")
                .unwrap_or(default.checkpoint_on_await_states),
            checkpoint_on_tool_call: std::env::var("CHECKPOINT_ON_TOOL_CALL")
                .map(|v| v != "false")
                .unwrap_or(default.checkpoint_on_tool_call),
        }
    }

    pub fn should_checkpoint(&self, reason: &str) -> bool {
        match reason {
            "awaiting_llm" => self.checkpoint_on_tool_call,
            "awaiting_tool" | "awaiting_approval" | "awaiting_delegation" => {
                self.checkpoint_on_await_states
            }
            "task_complete" | "task_fail" => true,
            _ => false,
        }
    }
}

pub fn checkpoint_config() -> CheckpointConfig {
    CheckpointConfig::from_env()
}
