use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AgentInstanceStatus {
    Created,
    Hydrating,
    Ready,
    Running,
    WaitingTool,
    WaitingSubagent,
    WaitingApproval,
    Suspended,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for AgentInstanceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentInstanceStatus::Created => write!(f, "CREATED"),
            AgentInstanceStatus::Hydrating => write!(f, "HYDRATING"),
            AgentInstanceStatus::Ready => write!(f, "READY"),
            AgentInstanceStatus::Running => write!(f, "RUNNING"),
            AgentInstanceStatus::WaitingTool => write!(f, "WAITING_TOOL"),
            AgentInstanceStatus::WaitingSubagent => write!(f, "WAITING_SUBAGENT"),
            AgentInstanceStatus::WaitingApproval => write!(f, "WAITING_APPROVAL"),
            AgentInstanceStatus::Suspended => write!(f, "SUSPENDED"),
            AgentInstanceStatus::Completed => write!(f, "COMPLETED"),
            AgentInstanceStatus::Failed => write!(f, "FAILED"),
            AgentInstanceStatus::Cancelled => write!(f, "CANCELLED"),
        }
    }
}

impl TryFrom<&str> for AgentInstanceStatus {
    type Error = String;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "CREATED" => Ok(AgentInstanceStatus::Created),
            "HYDRATING" => Ok(AgentInstanceStatus::Hydrating),
            "READY" => Ok(AgentInstanceStatus::Ready),
            "RUNNING" => Ok(AgentInstanceStatus::Running),
            "WAITING_TOOL" => Ok(AgentInstanceStatus::WaitingTool),
            "WAITING_SUBAGENT" => Ok(AgentInstanceStatus::WaitingSubagent),
            "WAITING_APPROVAL" => Ok(AgentInstanceStatus::WaitingApproval),
            "SUSPENDED" => Ok(AgentInstanceStatus::Suspended),
            "COMPLETED" => Ok(AgentInstanceStatus::Completed),
            "FAILED" => Ok(AgentInstanceStatus::Failed),
            "CANCELLED" => Ok(AgentInstanceStatus::Cancelled),
            _ => Err(format!("Unknown status: {}", s)),
        }
    }
}

#[derive(Debug, Serialize, FromRow)]
pub struct AgentInstance {
    pub id: Uuid,
    pub agent_definition_id: Uuid,
    pub status: AgentInstanceStatus,
    pub external_context_refs: serde_json::Value,
    pub current_task_id: Option<Uuid>,
    pub checkpoint_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct AgentInstanceCreate {
    pub agent_definition_id: Uuid,
    #[serde(default)]
    pub external_context_refs: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct TimeTravelRequest {
    pub checkpoint_id: Uuid,
    pub branch_name: Option<String>,
}

mod sqlx_impls {
    use super::*;
    use sqlx::postgres::{PgArgumentBuffer, PgTypeInfo, PgValueRef};
    use sqlx::types::Type;
    use sqlx::encode::Encode;
    use sqlx::decode::Decode;

    impl Type<sqlx::Postgres> for AgentInstanceStatus {
        fn type_info() -> PgTypeInfo {
            PgTypeInfo::with_name("TEXT")
        }

        fn compatible(ty: &PgTypeInfo) -> bool {
            *ty == PgTypeInfo::with_name("TEXT")
                || *ty == PgTypeInfo::with_name("VARCHAR")
                || *ty == PgTypeInfo::with_name("UNKNOWN")
        }
    }

    impl Encode<'_, sqlx::Postgres> for AgentInstanceStatus {
        fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
            let s = self.to_string();
            Ok(<String as Encode<sqlx::Postgres>>::encode(s, buf)?)
        }
    }

    impl Decode<'_, sqlx::Postgres> for AgentInstanceStatus {
        fn decode(value: PgValueRef<'_>) -> Result<Self, Box<dyn std::error::Error + Send + Sync + 'static>> {
            let s: String = Decode::<sqlx::Postgres>::decode(value)?;
            s.as_str().try_into().map_err(|e: String| format!("{}", e).into())
        }
    }
}

