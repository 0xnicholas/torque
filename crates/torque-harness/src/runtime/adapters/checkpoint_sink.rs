use async_trait::async_trait;
use checkpointer::{CheckpointState, Checkpointer};
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

fn to_checkpoint_state(value: serde_json::Value) -> CheckpointState {
    match serde_json::from_value(value.clone()) {
        Ok(state) => state,
        Err(_) => CheckpointState {
            messages: vec![],
            tool_call_count: 0,
            intermediate_results: vec![],
            custom_state: Some(value),
        },
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
                to_checkpoint_state(checkpoint.state),
            )
            .await?;

        Ok(RuntimeCheckpointRef {
            checkpoint_id: checkpoint_id.0,
            instance_id: checkpoint.instance_id.as_uuid(),
        })
    }
}
