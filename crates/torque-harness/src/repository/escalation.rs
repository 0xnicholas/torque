use crate::db::Database;
use crate::models::v1::escalation::{Escalation, EscalationSeverity, EscalationStatus, EscalationType};
use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait EscalationRepository: Send + Sync {
    async fn create(
        &self,
        instance_id: Uuid,
        team_instance_id: Option<Uuid>,
        escalation_type: EscalationType,
        severity: EscalationSeverity,
        description: &str,
        context: serde_json::Value,
    ) -> anyhow::Result<Escalation>;
    async fn get(&self, id: Uuid) -> anyhow::Result<Option<Escalation>>;
    async fn list_by_instance(&self, instance_id: Uuid, limit: i64) -> anyhow::Result<Vec<Escalation>>;
    async fn list_by_team(&self, team_instance_id: Uuid, limit: i64) -> anyhow::Result<Vec<Escalation>>;
    async fn list_pending(&self, limit: i64) -> anyhow::Result<Vec<Escalation>>;
    async fn update_status(&self, id: Uuid, status: EscalationStatus) -> anyhow::Result<bool>;
    async fn resolve(
        &self,
        id: Uuid,
        resolved_by: Uuid,
        resolution: &str,
    ) -> anyhow::Result<bool>;
}

pub struct PostgresEscalationRepository {
    db: Database,
}

impl PostgresEscalationRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

#[async_trait]
impl EscalationRepository for PostgresEscalationRepository {
    async fn create(
        &self,
        instance_id: Uuid,
        team_instance_id: Option<Uuid>,
        escalation_type: EscalationType,
        severity: EscalationSeverity,
        description: &str,
        context: serde_json::Value,
    ) -> anyhow::Result<Escalation> {
        let row = sqlx::query_as::<_, Escalation>(
            "INSERT INTO v1_escalations (instance_id, team_instance_id, escalation_type, severity, status, description, context) VALUES ($1, $2, $3, $4, 'pending', $5, $6) RETURNING *"
        )
        .bind(instance_id)
        .bind(team_instance_id)
        .bind(escalation_type)
        .bind(severity)
        .bind(description)
        .bind(context)
        .fetch_one(self.db.pool())
        .await?;
        Ok(row)
    }

    async fn get(&self, id: Uuid) -> anyhow::Result<Option<Escalation>> {
        let row = sqlx::query_as::<_, Escalation>(
            "SELECT * FROM v1_escalations WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(self.db.pool())
        .await?;
        Ok(row)
    }

    async fn list_by_instance(&self, instance_id: Uuid, limit: i64) -> anyhow::Result<Vec<Escalation>> {
        let rows = sqlx::query_as::<_, Escalation>(
            "SELECT * FROM v1_escalations WHERE instance_id = $1 ORDER BY created_at DESC LIMIT $2"
        )
        .bind(instance_id)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows)
    }

    async fn list_by_team(&self, team_instance_id: Uuid, limit: i64) -> anyhow::Result<Vec<Escalation>> {
        let rows = sqlx::query_as::<_, Escalation>(
            "SELECT * FROM v1_escalations WHERE team_instance_id = $1 ORDER BY created_at DESC LIMIT $2"
        )
        .bind(team_instance_id)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows)
    }

    async fn list_pending(&self, limit: i64) -> anyhow::Result<Vec<Escalation>> {
        let rows = sqlx::query_as::<_, Escalation>(
            "SELECT * FROM v1_escalations WHERE status IN ('pending', 'acknowledged', 'in_progress') ORDER BY created_at DESC LIMIT $1"
        )
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows)
    }

    async fn update_status(&self, id: Uuid, status: EscalationStatus) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "UPDATE v1_escalations SET status = $1 WHERE id = $2"
        )
        .bind(status)
        .bind(id)
        .execute(self.db.pool())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn resolve(
        &self,
        id: Uuid,
        resolved_by: Uuid,
        resolution: &str,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "UPDATE v1_escalations SET status = 'resolved', resolved_at = NOW(), resolved_by = $1, resolution = $2 WHERE id = $3"
        )
        .bind(resolved_by)
        .bind(resolution)
        .bind(id)
        .execute(self.db.pool())
        .await?;
        Ok(result.rows_affected() > 0)
    }
}