use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Serialize, FromRow)]
pub struct Approval {
    pub id: Uuid,
    pub task_id: Uuid,
    pub approval_type: String,
    pub status: String,
    pub requested_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct ApprovalResolveRequest {
    pub resolution: String,
    pub comment: Option<String>,
}
