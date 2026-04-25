use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeCheckpointEvent {
    pub checkpoint_id: Uuid,
    pub instance_id: Uuid,
    pub reason: String,
}
