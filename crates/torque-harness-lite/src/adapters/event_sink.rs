use async_trait::async_trait;
use std::sync::Mutex;
use torque_kernel::{AgentInstanceId, ExecutionResult};
use torque_runtime::environment::RuntimeEventSink;
use uuid::Uuid;

#[derive(Default)]
pub struct InMemoryEventSink {
    results: Mutex<Vec<ExecutionResult>>,
    checkpoint_count: Mutex<usize>,
}

impl InMemoryEventSink {
    pub fn execution_count(&self) -> usize {
        self.results.lock().unwrap().len()
    }
}

#[async_trait]
impl RuntimeEventSink for InMemoryEventSink {
    async fn record_execution_result(&self, result: &ExecutionResult) -> anyhow::Result<()> {
        self.results.lock().unwrap().push(result.clone());
        Ok(())
    }

    async fn record_checkpoint_created(
        &self,
        _checkpoint_id: Uuid,
        _instance_id: AgentInstanceId,
        _reason: &str,
    ) -> anyhow::Result<()> {
        *self.checkpoint_count.lock().unwrap() += 1;
        Ok(())
    }
}
