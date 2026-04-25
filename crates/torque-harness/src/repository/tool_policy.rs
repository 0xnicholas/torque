use crate::models::v1::tool_policy::{ToolPolicy, ToolRiskLevel};
use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait ToolPolicyRepository: Send + Sync {
    async fn upsert(&self, policy: &ToolPolicy) -> anyhow::Result<()>;
    async fn get(&self, tool_name: &str) -> anyhow::Result<Option<ToolPolicy>>;
    async fn list(&self) -> anyhow::Result<Vec<ToolPolicy>>;
    async fn delete(&self, tool_name: &str) -> anyhow::Result<()>;
}

pub struct PostgresToolPolicyRepository {
    db: crate::db::Database,
}

impl PostgresToolPolicyRepository {
    pub fn new(db: crate::db::Database) -> Self {
        Self { db }
    }
}

#[async_trait]
impl ToolPolicyRepository for PostgresToolPolicyRepository {
    async fn upsert(&self, policy: &ToolPolicy) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO v1_tool_policies (id, tool_name, risk_level, side_effects, requires_approval, blocked, blocked_reason)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (tool_name) DO UPDATE SET
                risk_level = EXCLUDED.risk_level,
                side_effects = EXCLUDED.side_effects,
                requires_approval = EXCLUDED.requires_approval,
                blocked = EXCLUDED.blocked,
                blocked_reason = EXCLUDED.blocked_reason,
                updated_at = NOW()
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(&policy.tool_name)
        .bind(format!("{:?}", policy.risk_level).to_lowercase())
        .bind(&policy.side_effects.iter().map(|s| format!("{:?}", s)).collect::<Vec<_>>())
        .bind(policy.requires_approval)
        .bind(policy.blocked)
        .bind(&policy.blocked_reason)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    async fn get(&self, tool_name: &str) -> anyhow::Result<Option<ToolPolicy>> {
        let row = sqlx::query_as::<_, ToolPolicyRow>(
            "SELECT * FROM v1_tool_policies WHERE tool_name = $1",
        )
        .bind(tool_name)
        .fetch_optional(self.db.pool())
        .await?;
        Ok(row.map(|r| r.into()))
    }

    async fn list(&self) -> anyhow::Result<Vec<ToolPolicy>> {
        let rows = sqlx::query_as::<_, ToolPolicyRow>(
            "SELECT * FROM v1_tool_policies ORDER BY tool_name",
        )
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows.into_iter().map(|r| r.into()).collect())
    }

    async fn delete(&self, tool_name: &str) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM v1_tool_policies WHERE tool_name = $1")
            .bind(tool_name)
            .execute(self.db.pool())
            .await?;
        Ok(())
    }
}

use sqlx::FromRow;
use chrono::{DateTime, Utc};

#[derive(FromRow)]
struct ToolPolicyRow {
    id: Uuid,
    tool_name: String,
    risk_level: String,
    side_effects: Vec<String>,
    requires_approval: bool,
    blocked: bool,
    blocked_reason: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<ToolPolicyRow> for ToolPolicy {
    fn from(row: ToolPolicyRow) -> Self {
        ToolPolicy {
            id: row.id,
            tool_name: row.tool_name,
            risk_level: match row.risk_level.as_str() {
                "low" => ToolRiskLevel::Low,
                "medium" => ToolRiskLevel::Medium,
                "high" => ToolRiskLevel::High,
                "critical" => ToolRiskLevel::Critical,
                _ => ToolRiskLevel::Medium,
            },
            side_effects: row.side_effects.iter().map(|s| serde_json::from_str(s).unwrap_or(crate::models::v1::tool_policy::ToolSideEffect::FileSystem)).collect(),
            requires_approval: row.requires_approval,
            blocked: row.blocked,
            blocked_reason: row.blocked_reason,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}