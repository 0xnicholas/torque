use async_trait::async_trait;
use checkpointer::Checkpointer;
use std::sync::Arc;
use torque_runtime::checkpoint::{RuntimeCheckpointPayload, RuntimeCheckpointRef};
use torque_runtime::environment::RuntimeCheckpointSink;

pub struct HarnessCheckpointSink {
    checkpointer: Arc<dyn Checkpointer>,
}

impl HarnessCheckpointSink {
    pub fn new(checkpointer: Arc<dyn Checkpointer>) -> Self {
        Self { checkpointer }
    }
}

#[async_trait]
impl RuntimeCheckpointSink for HarnessCheckpointSink {
    async fn save(
        &self,
        checkpoint: RuntimeCheckpointPayload,
    ) -> anyhow::Result<RuntimeCheckpointRef> {
        let checkpoint_id = self
            .checkpointer
            .save(
                checkpoint.instance_id.as_uuid(),
                checkpoint.node_id,
                checkpoint.state,
            )
            .await?;

        Ok(RuntimeCheckpointRef {
            checkpoint_id: checkpoint_id.0,
            instance_id: checkpoint.instance_id.as_uuid(),
        })
    }
}
