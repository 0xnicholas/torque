use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalContextRef {
    pub id: Uuid,
    pub agent_instance_id: Uuid,
    pub context_type: String,
    pub uri: String,
    pub access_mode: String,
    pub created_at: DateTime<Utc>,
}