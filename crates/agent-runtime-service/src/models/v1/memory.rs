use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, sqlx::Type, Serialize, Deserialize)]
#[sqlx(rename_all = "snake_case")]
pub enum MemoryCategory {
    AgentProfileMemory,
    UserPreferenceMemory,
    TaskOrDomainMemory,
    ExternalContextMemory,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MemoryContent {
    pub category: MemoryCategory,
    pub key: String,
    pub value: serde_json::Value,
}

#[derive(Debug, sqlx::Type, Serialize, Deserialize)]
#[sqlx(rename_all = "UPPERCASE")]
pub enum MemoryWriteCandidateStatus {
    Pending,
    Approved,
    Rejected,
}

#[derive(Debug, Serialize, FromRow)]
pub struct MemoryWriteCandidate {
    pub id: Uuid,
    pub agent_instance_id: Uuid,
    pub team_instance_id: Option<Uuid>,
    pub content: serde_json::Value,
    pub reasoning: Option<String>,
    pub status: MemoryWriteCandidateStatus,
    pub memory_entry_id: Option<Uuid>,
    pub reviewed_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub reviewed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct MemoryWriteCandidateCreate {
    pub agent_instance_id: Uuid,
    pub content: MemoryContent,
    pub reasoning: Option<String>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct MemoryEntry {
    pub id: Uuid,
    pub agent_instance_id: Option<Uuid>,
    pub team_instance_id: Option<Uuid>,
    pub category: MemoryCategory,
    pub key: String,
    pub value: serde_json::Value,
    pub source_candidate_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
