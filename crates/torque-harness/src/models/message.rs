use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Message {
    pub id: Uuid,
    pub session_id: Uuid,
    pub role: MessageRole,
    pub content: String,
    pub tool_calls: Option<Value>,
    pub artifacts: Option<Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "TEXT")]
#[sqlx(rename_all = "snake_case")]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Tool,
}

impl Message {
    pub fn user(session_id: Uuid, content: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            session_id,
            role: MessageRole::User,
            content,
            tool_calls: None,
            artifacts: None,
            created_at: Utc::now(),
        }
    }

    pub fn assistant(
        session_id: Uuid,
        content: String,
        tool_calls: Option<Value>,
        artifacts: Option<Value>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            session_id,
            role: MessageRole::Assistant,
            content,
            tool_calls,
            artifacts,
            created_at: Utc::now(),
        }
    }
}
