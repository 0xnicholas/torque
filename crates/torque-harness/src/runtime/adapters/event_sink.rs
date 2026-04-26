use crate::repository::EventRepository;
use crate::runtime::events::EventRecorder;
use async_trait::async_trait;
use std::sync::Arc;
use torque_kernel::{AgentInstanceId, ExecutionResult};
use torque_runtime::environment::RuntimeEventSink;
use uuid::Uuid;

pub struct HarnessEventSink {
    event_repo: Arc<dyn EventRepository>,
}

impl HarnessEventSink {
    pub fn new(event_repo: Arc<dyn EventRepository>) -> Self {
        Self { event_repo }
    }
}

#[async_trait]
impl RuntimeEventSink for HarnessEventSink {
    async fn record_execution_result(&self, result: &ExecutionResult) -> anyhow::Result<()> {
        let events = EventRecorder::to_db_events(result, result.sequence_number);
        for event in events {
            self.event_repo.create(event).await?;
        }
        Ok(())
    }

    async fn record_checkpoint_created(
        &self,
        checkpoint_id: Uuid,
        instance_id: AgentInstanceId,
        reason: &str,
    ) -> anyhow::Result<()> {
        let event =
            EventRecorder::checkpoint_created_event(checkpoint_id, instance_id.as_uuid(), reason);
        self.event_repo.create(event).await?;
        Ok(())
    }
}
