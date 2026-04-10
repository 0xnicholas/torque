use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "TEXT")]
#[sqlx(rename_all = "snake_case")]
pub enum MemoryLayer {
    L0,
    L1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "TEXT")]
#[sqlx(rename_all = "snake_case")]
pub enum MemoryCandidateStatus {
    Pending,
    Accepted,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "TEXT")]
#[sqlx(rename_all = "snake_case")]
pub enum MemoryEntryStatus {
    Active,
    Invalidated,
    Replaced,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MemoryCandidate {
    pub id: Uuid,
    pub project_scope: String,
    pub layer: MemoryLayer,
    pub proposed_fact: String,
    pub source_type: Option<String>,
    pub source_ref: Option<String>,
    pub proposer: Option<String>,
    pub confidence: Option<f64>,
    pub status: MemoryCandidateStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub accepted_at: Option<DateTime<Utc>>,
    pub rejected_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MemoryEntry {
    pub id: Uuid,
    pub project_scope: String,
    pub layer: MemoryLayer,
    pub content: String,
    pub source_candidate_id: Option<Uuid>,
    pub source_type: Option<String>,
    pub source_ref: Option<String>,
    pub proposer: Option<String>,
    pub status: MemoryEntryStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub invalidated_at: Option<DateTime<Utc>>,
}

impl MemoryCandidate {
    pub fn new(project_scope: String, layer: MemoryLayer, proposed_fact: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            project_scope,
            layer,
            proposed_fact,
            source_type: None,
            source_ref: None,
            proposer: None,
            confidence: None,
            status: MemoryCandidateStatus::Pending,
            created_at: now,
            updated_at: now,
            accepted_at: None,
            rejected_at: None,
        }
    }
}

impl MemoryEntry {
    pub fn new(project_scope: String, layer: MemoryLayer, content: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            project_scope,
            layer,
            content,
            source_candidate_id: None,
            source_type: None,
            source_ref: None,
            proposer: None,
            status: MemoryEntryStatus::Active,
            created_at: now,
            updated_at: now,
            invalidated_at: None,
        }
    }
}
