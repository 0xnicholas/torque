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

// Concrete handlers
struct TaskCompletedHandler;
struct TaskFailedHandler;
struct InstanceSuspendedHandler;
struct InstanceResumedHandler;
struct NoOpHandler;

#[async_trait]
impl EventReplayHandler for TaskCompletedHandler {
    async fn replay(
        &self,
        event: &Event,
        repo: &Arc<dyn AgentInstanceRepository>,
    ) -> Result<(), String> {
        let instance_id = event.resource_id;
        repo.update_current_task(instance_id, None).await
            .map_err(|e| format!("Failed to update current task: {}", e))?;
        repo.update_status(instance_id, AgentInstanceStatus::Ready).await
            .map_err(|e| format!("Failed to update status: {}", e))?;
        Ok(())
    }
}

#[async_trait]
impl EventReplayHandler for TaskFailedHandler {
    async fn replay(
        &self,
        event: &Event,
        repo: &Arc<dyn AgentInstanceRepository>,
    ) -> Result<(), String> {
        let instance_id = event.resource_id;
        repo.update_current_task(instance_id, None).await
            .map_err(|e| format!("Failed to update current task: {}", e))?;
        Ok(())
    }
}

#[async_trait]
impl EventReplayHandler for InstanceSuspendedHandler {
    async fn replay(
        &self,
        event: &Event,
        repo: &Arc<dyn AgentInstanceRepository>,
    ) -> Result<(), String> {
        let instance_id = event.resource_id;
        repo.update_status(instance_id, AgentInstanceStatus::Suspended).await
            .map_err(|e| format!("Failed to suspend instance: {}", e))?;
        Ok(())
    }
}

#[async_trait]
impl EventReplayHandler for InstanceResumedHandler {
    async fn replay(
        &self,
        event: &Event,
        repo: &Arc<dyn AgentInstanceRepository>,
    ) -> Result<(), String> {
        let instance_id = event.resource_id;
        repo.update_status(instance_id, AgentInstanceStatus::Ready).await
            .map_err(|e| format!("Failed to resume instance: {}", e))?;
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

    pub fn register(
        &mut self,
        event_type: &str,
        handler: Box<dyn EventReplayHandler>,
    ) {
        self.handlers.insert(event_type.to_string(), handler);
    }

    pub async fn replay(
        &self,
        event: &Event,
        repo: &Arc<dyn AgentInstanceRepository>,
        override_instance_id: Option<uuid::Uuid>,
    ) -> Result<(), String> {
        if let Some(handler) = self.handlers.get(&event.event_type) {
            // Create a modified event with overridden instance_id if provided
            let mut event_to_replay = event.clone();
            if let Some(new_id) = override_instance_id {
                event_to_replay.resource_id = new_id;
            }
            handler.replay(&event_to_replay, repo).await
        } else {
            // Unknown event type: skip (not an error)
            Ok(())
        }
    }

    fn register_default_handlers(&mut self) {
        self.register("task.completed", Box::new(TaskCompletedHandler));
        self.register("task.failed", Box::new(TaskFailedHandler));
        self.register("instance.suspended", Box::new(InstanceSuspendedHandler));
        self.register("instance.resumed", Box::new(InstanceResumedHandler));
        self.register("checkpoint.created", Box::new(NoOpHandler));
        self.register("task.created", Box::new(NoOpHandler));
    }
}

impl Default for EventReplayRegistry {
    fn default() -> Self {
        Self::new()
    }
}
