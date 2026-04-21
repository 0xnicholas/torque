use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub id: Uuid,
    pub run_id: Uuid,
    pub source_node: Uuid,
    pub target_node: Uuid,
}

impl Edge {
    pub fn new(run_id: Uuid, source_node: Uuid, target_node: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            run_id,
            source_node,
            target_node,
        }
    }
}
