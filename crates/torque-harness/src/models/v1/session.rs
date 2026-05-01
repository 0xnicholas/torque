use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::fmt::Display;
use uuid::Uuid;

/// Status of a long-running agent session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "session_status", rename_all = "snake_case")]
pub enum SessionStatus {
    /// Session is active and accepting messages.
    Active,
    /// Session exists but is not accepting new messages.
    Idle,
    /// Compaction is in progress on this session.
    Compacting,
    /// Session encountered a terminal error.
    Error,
    /// Session has been terminated.
    Terminated,
}

impl Display for SessionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionStatus::Active => write!(f, "active"),
            SessionStatus::Idle => write!(f, "idle"),
            SessionStatus::Compacting => write!(f, "compacting"),
            SessionStatus::Error => write!(f, "error"),
            SessionStatus::Terminated => write!(f, "terminated"),
        }
    }
}

impl SessionStatus {
    /// Returns `true` if this status allows chat operations.
    pub fn is_available(&self) -> bool {
        matches!(self, SessionStatus::Active)
    }

    /// Returns `true` if this status allows triggering compaction.
    /// Allows retry when previous compaction was orphaned (status stuck in Compacting).
    pub fn can_compact(&self) -> bool {
        matches!(self, SessionStatus::Active | SessionStatus::Compacting)
    }

    /// Returns `true` if this is a terminal status.
    pub fn is_terminal(&self) -> bool {
        matches!(self, SessionStatus::Terminated)
    }
}

/// A long-running agent session that wraps an AgentInstance and its Run history.
///
/// Sessions provide the primary user-facing API for interacting with agents:
/// - `chat()` sends a message and streams the response
/// - `compact()` explicitly triggers context compaction with optional custom instructions
/// - `abort_compaction()` cancels an in-flight compaction
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Session {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub agent_definition_id: Uuid,
    pub agent_instance_id: Option<Uuid>,
    pub status: SessionStatus,
    pub title: Option<String>,
    pub metadata: serde_json::Value,
    pub active_compaction_job_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Request payload for creating a new session.
#[derive(Debug, Deserialize)]
pub struct SessionCreateRequest {
    pub agent_definition_id: Uuid,
    pub title: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Request payload for sending a chat message to a session.
#[derive(Debug, Deserialize)]
pub struct SessionChatRequest {
    pub message: String,
    pub additional_instructions: Option<String>,
}

/// Request payload for triggering compaction on a session.
#[derive(Debug, Deserialize)]
pub struct SessionCompactRequest {
    pub custom_instructions: Option<String>,
}

/// Response from triggering or aborting compaction.
#[derive(Debug, Serialize)]
pub struct CompactJobResponse {
    pub job_id: Uuid,
    pub status: String,
    pub session_id: Uuid,
}

/// Response from aborting a compaction job.
/// `job_id` is `None` when no active compaction job was found.
#[derive(Debug, Serialize)]
pub struct CompactionAbortResponse {
    pub job_id: Option<Uuid>,
    pub status: String,
    pub session_id: Uuid,
}
