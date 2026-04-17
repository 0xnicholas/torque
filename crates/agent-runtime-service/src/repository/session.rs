use async_trait::async_trait;
use crate::db::Database;
use crate::models::{Session, SessionStatus};
use uuid::Uuid;

pub struct SessionKernelState {
    pub agent_definition_id: Uuid,
    pub status: String,
    pub active_task_id: Option<Uuid>,
    pub checkpoint_id: Option<Uuid>,
}

#[async_trait]
pub trait SessionRepository: Send + Sync {
    async fn create(&self, api_key: &str, project_scope: &str) -> anyhow::Result<Session>;
    async fn list(&self, limit: i64) -> anyhow::Result<Vec<Session>>;
    async fn get_by_id(&self, id: Uuid) -> anyhow::Result<Option<Session>>;
    async fn update_status(
        &self,
        id: Uuid,
        status: SessionStatus,
        error_msg: Option<&str>,
    ) -> anyhow::Result<bool>;
    async fn try_mark_running(&self, id: Uuid) -> anyhow::Result<bool>;
    async fn get_kernel_state(&self, id: Uuid) -> anyhow::Result<Option<SessionKernelState>>;
}

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
    async fn create(&self, api_key: &str, project_scope: &str) -> anyhow::Result<Session> {
        let row = sqlx::query_as::<_, Session>(
            r#"
            INSERT INTO sessions (api_key, project_scope, status)
            VALUES ($1, $2, 'idle')
            RETURNING id, api_key, project_scope, status, error_message, created_at, updated_at
            "#
        )
        .bind(api_key)
        .bind(project_scope)
        .fetch_one(self.db.pool())
        .await?;
        Ok(row)
    }

    async fn list(&self, limit: i64) -> anyhow::Result<Vec<Session>> {
        let rows = sqlx::query_as::<_, Session>(
            "SELECT * FROM sessions ORDER BY created_at DESC LIMIT $1"
        )
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows)
    }

    async fn get_by_id(&self, id: Uuid) -> anyhow::Result<Option<Session>> {
        let row = sqlx::query_as::<_, Session>(
            "SELECT * FROM sessions WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(self.db.pool())
        .await?;
        Ok(row)
    }

    async fn update_status(
        &self,
        id: Uuid,
        status: SessionStatus,
        error_msg: Option<&str>,
    ) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "UPDATE sessions SET status = $1, error_message = $2, updated_at = NOW() WHERE id = $3"
        )
        .bind(format!("{:?}", status).to_lowercase())
        .bind(error_msg)
        .bind(id)
        .execute(self.db.pool())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn try_mark_running(&self, id: Uuid) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "UPDATE sessions SET status = 'running', updated_at = NOW() WHERE id = $1 AND status = 'idle'"
        )
        .bind(id)
        .execute(self.db.pool())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn get_kernel_state(
        &self,
        id: Uuid,
    ) -> anyhow::Result<Option<SessionKernelState>> {
        let row: Option<(Uuid, String, Option<Uuid>, Option<Uuid>)> = sqlx::query_as(
            "SELECT id, status, active_task_id, checkpoint_id FROM sessions WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(self.db.pool())
        .await?;
        Ok(row.map(|(agent_def_id, status, active_task_id, checkpoint_id)| SessionKernelState {
            agent_definition_id: agent_def_id,
            status,
            active_task_id,
            checkpoint_id,
        }))
    }
}
