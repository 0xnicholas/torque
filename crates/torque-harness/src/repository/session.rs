use anyhow::Result;
use async_trait::async_trait;
use uuid::Uuid;

use crate::db::Database;
use crate::models::v1::session::{Session, SessionStatus};

/// Repository for persisting and querying Session records.
#[async_trait]
pub trait SessionRepository: Send + Sync {
    /// Create a new session.
    async fn create(&self, session: &Session) -> Result<Session>;

    /// Retrieve a session by its ID and tenant.
    async fn get(&self, id: Uuid, tenant_id: Uuid) -> Result<Option<Session>>;

    /// List all sessions for a tenant, ordered by most recent first.
    async fn list(&self, tenant_id: Uuid, limit: i64, offset: i64) -> Result<Vec<Session>>;

    /// Update a session's status.
    async fn update_status(&self, id: Uuid, tenant_id: Uuid, status: &SessionStatus) -> Result<()>;

    /// Update the agent_instance_id after run creation.
    async fn update_agent_instance(
        &self,
        id: Uuid,
        tenant_id: Uuid,
        agent_instance_id: Option<Uuid>,
    ) -> Result<()>;

    /// Track the active compaction job on a session.
    async fn update_compaction_job(
        &self,
        id: Uuid,
        tenant_id: Uuid,
        compaction_job_id: Option<Uuid>,
    ) -> Result<()>;

    /// Delete a session and its associated data.
    async fn delete(&self, id: Uuid, tenant_id: Uuid) -> Result<bool>;

    /// Count active sessions for a tenant.
    async fn count_active(&self, tenant_id: Uuid) -> Result<i64>;
}

// ---------------------------------------------------------------------------
// Postgres implementation
// ---------------------------------------------------------------------------

pub struct PostgresSessionRepository {
    db: Database,
}

impl PostgresSessionRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

#[async_trait]
impl SessionRepository for PostgresSessionRepository {
    async fn create(&self, session: &Session) -> Result<Session> {
        let row = sqlx::query_as::<_, Session>(
            r#"
            INSERT INTO sessions (id, tenant_id, agent_definition_id, agent_instance_id,
                                  status, title, metadata, active_compaction_job_id,
                                  created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING *
            "#,
        )
        .bind(session.id)
        .bind(session.tenant_id)
        .bind(session.agent_definition_id)
        .bind(session.agent_instance_id)
        .bind(&session.status)
        .bind(&session.title)
        .bind(&session.metadata)
        .bind(session.active_compaction_job_id)
        .bind(session.created_at)
        .bind(session.updated_at)
        .fetch_one(self.db.pool())
        .await?;

        Ok(row)
    }

    async fn get(&self, id: Uuid, tenant_id: Uuid) -> Result<Option<Session>> {
        let row = sqlx::query_as::<_, Session>(
            r#"
            SELECT * FROM sessions WHERE id = $1 AND tenant_id = $2
            "#,
        )
        .bind(id)
        .bind(tenant_id)
        .fetch_optional(self.db.pool())
        .await?;

        Ok(row)
    }

    async fn list(&self, tenant_id: Uuid, limit: i64, offset: i64) -> Result<Vec<Session>> {
        let rows = sqlx::query_as::<_, Session>(
            r#"
            SELECT * FROM sessions
            WHERE tenant_id = $1
            ORDER BY updated_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(tenant_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(self.db.pool())
        .await?;

        Ok(rows)
    }

    async fn update_status(
        &self,
        id: Uuid,
        tenant_id: Uuid,
        status: &SessionStatus,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE sessions
            SET status = $3, updated_at = NOW()
            WHERE id = $1 AND tenant_id = $2
            "#,
        )
        .bind(id)
        .bind(tenant_id)
        .bind(status)
        .execute(self.db.pool())
        .await?;

        Ok(())
    }

    async fn update_agent_instance(
        &self,
        id: Uuid,
        tenant_id: Uuid,
        agent_instance_id: Option<Uuid>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE sessions
            SET agent_instance_id = $3, updated_at = NOW()
            WHERE id = $1 AND tenant_id = $2
            "#,
        )
        .bind(id)
        .bind(tenant_id)
        .bind(agent_instance_id)
        .execute(self.db.pool())
        .await?;

        Ok(())
    }

    async fn update_compaction_job(
        &self,
        id: Uuid,
        tenant_id: Uuid,
        compaction_job_id: Option<Uuid>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE sessions
            SET active_compaction_job_id = $3, updated_at = NOW()
            WHERE id = $1 AND tenant_id = $2
            "#,
        )
        .bind(id)
        .bind(tenant_id)
        .bind(compaction_job_id)
        .execute(self.db.pool())
        .await?;

        Ok(())
    }

    async fn delete(&self, id: Uuid, tenant_id: Uuid) -> Result<bool> {
        let result = sqlx::query(
            r#"
            DELETE FROM sessions WHERE id = $1 AND tenant_id = $2
            "#,
        )
        .bind(id)
        .bind(tenant_id)
        .execute(self.db.pool())
        .await?;

        Ok(result.rows_affected() > 0)
    }

    async fn count_active(&self, tenant_id: Uuid) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*) FROM sessions
            WHERE tenant_id = $1 AND status = 'active'
            "#,
        )
        .bind(tenant_id)
        .fetch_one(self.db.pool())
        .await?;

        Ok(row.0)
    }
}
