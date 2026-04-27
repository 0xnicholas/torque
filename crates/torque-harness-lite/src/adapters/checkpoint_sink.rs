use async_trait::async_trait;
use std::sync::Mutex;
use torque_runtime::checkpoint::{RuntimeCheckpointPayload, RuntimeCheckpointRef};
use torque_runtime::environment::RuntimeCheckpointSink;
use uuid::Uuid;

#[derive(Default)]
pub struct InMemoryCheckpointSink {
    payloads: Mutex<Vec<RuntimeCheckpointPayload>>,
}

#[async_trait]
impl RuntimeCheckpointSink for InMemoryCheckpointSink {
    async fn save(
        &self,
        payload: RuntimeCheckpointPayload,
    ) -> anyhow::Result<RuntimeCheckpointRef> {
        let checkpoint_id = Uuid::new_v4();
        let instance_id = payload.instance_id.as_uuid();
        self.payloads.lock().unwrap().push(payload);
        Ok(RuntimeCheckpointRef {
            checkpoint_id,
            instance_id,
        })
    }
}
