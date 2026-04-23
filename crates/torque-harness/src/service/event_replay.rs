use crate::models::v1::agent_instance::AgentInstanceStatus;
use crate::models::v1::event::Event;
use crate::repository::AgentInstanceRepository;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

#[async_trait]
pub trait EventReplayHandler: Send + Sync {
    async fn replay(
        &self,
        event: &Event,
        repo: &Arc<dyn AgentInstanceRepository>,
    ) -> Result<(), String>;
}

/// Registry for event replay handlers during recovery.
pub struct EventReplayRegistry {
    handlers: HashMap<String, Box<dyn EventReplayHandler>>,
}

struct InstanceStateChangedHandler;
struct TaskStateChangedHandler;
struct NoOpHandler;

#[async_trait]
impl EventReplayHandler for InstanceStateChangedHandler {
    async fn replay(
        &self,
        event: &Event,
        repo: &Arc<dyn AgentInstanceRepository>,
    ) -> Result<(), String> {
        let instance_id = event.resource_id;
        let payload = &event.payload;
        let to_state = payload
            .get("to")
            .and_then(|v| v.as_str())
            .unwrap_or("Created");

        let status = match to_state {
            "Ready" => AgentInstanceStatus::Ready,
            "Running" => AgentInstanceStatus::Running,
            "Suspended" => AgentInstanceStatus::Suspended,
            "Failed" => AgentInstanceStatus::Failed,
            "Completed" => AgentInstanceStatus::Completed,
            "Cancelled" => AgentInstanceStatus::Cancelled,
            _ => return Ok(()),
        };

        repo.update_status(instance_id, status)
            .await
            .map_err(|e| format!("Failed to update instance status: {}", e))?;
        Ok(())
    }
}

#[async_trait]
impl EventReplayHandler for TaskStateChangedHandler {
    async fn replay(
        &self,
        event: &Event,
        repo: &Arc<dyn AgentInstanceRepository>,
    ) -> Result<(), String> {
        let _task_id = event.resource_id;
        let payload = &event.payload;
        let to_state = payload
            .get("to")
            .and_then(|v| v.as_str())
            .unwrap_or("Created");

        match to_state {
            "Completed" | "Failed" => {
                repo.update_current_task(event.resource_id, None)
                    .await
                    .map_err(|e| format!("Failed to clear current task: {}", e))?;
            }
            _ => {}
        }
        Ok(())
    }
}

#[async_trait]
impl EventReplayHandler for NoOpHandler {
    async fn replay(
        &self,
        _event: &Event,
        _repo: &Arc<dyn AgentInstanceRepository>,
    ) -> Result<(), String> {
        Ok(())
    }
}

impl EventReplayRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            handlers: HashMap::new(),
        };
        registry.register_default_handlers();
        registry
    }

    pub fn register(&mut self, event_type: &str, handler: Box<dyn EventReplayHandler>) {
        self.handlers.insert(event_type.to_string(), handler);
    }

    pub async fn replay(
        &self,
        event: &Event,
        repo: &Arc<dyn AgentInstanceRepository>,
        override_instance_id: Option<uuid::Uuid>,
    ) -> Result<(), String> {
        if let Some(handler) = self.handlers.get(&event.event_type) {
            let mut event_to_replay = event.clone();
            if let Some(new_id) = override_instance_id {
                event_to_replay.resource_id = new_id;
            }
            handler.replay(&event_to_replay, repo).await
        } else {
            Ok(())
        }
    }

    fn register_default_handlers(&mut self) {
        self.register(
            "instance_state_changed",
            Box::new(InstanceStateChangedHandler),
        );
        self.register("task_state_changed", Box::new(TaskStateChangedHandler));
        self.register("checkpoint.created", Box::new(NoOpHandler));
        self.register("task.created", Box::new(NoOpHandler));
        self.register("artifact_produced", Box::new(NoOpHandler));
        self.register("approval_requested", Box::new(NoOpHandler));
        self.register("delegation_requested", Box::new(NoOpHandler));
        self.register("resume_applied", Box::new(NoOpHandler));
    }
}

impl Default for EventReplayRegistry {
    fn default() -> Self {
        Self::new()
    }
}
