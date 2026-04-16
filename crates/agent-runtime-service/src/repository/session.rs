use async_trait::async_trait;
use crate::db::Database;
use crate::models::{Session, SessionStatus};
use uuid::Uuid;

pub struct SessionKernelState {
    pub status: String,
}

#[async_trait]
pub trait SessionRepository: Send + Sync {
    async fn create(&self, api_key: &str, project_scope: &str) -> anyhow::Result<Session>;
    async fn list(&self, api_key: &str, limit: i64) -> anyhow::Result<Vec<Session>>;
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
            INSERT INTO sessions (api_key, status, project_scope)
            VALUES ($1, 'idle', $2)
            RETURNING *
            "#,
        )
        .bind(api_key)
        .bind(project_scope)
        .fetch_one(self.db.pool())
        .await?;
        Ok(row)
    }

    async fn list(&self, api_key: &str, limit: i64) -> anyhow::Result<Vec<Session>> {
        let rows = sqlx::query_as::<_, Session>(
            "SELECT * FROM sessions WHERE api_key = $1 ORDER BY created_at DESC LIMIT $2"
        )
        .bind(api_key)
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
        .bind(status)
        .bind(error_msg)
        .bind(id)
        .execute(self.db.pool())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn try_mark_running(&self, id: Uuid) -> anyhow::Result<bool> {
        let result = sqlx::query(
            "UPDATE sessions SET status = 'running', error_message = NULL, updated_at = NOW() WHERE id = $1 AND status IN ('idle', 'completed')"
        )
        .bind(id)
        .execute(self.db.pool())
        .await?;
        Ok(result.rows_affected() == 1)
    }

    async fn get_kernel_state(&self, id: Uuid) -> anyhow::Result<Option<SessionKernelState>> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT status FROM sessions WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(self.db.pool())
        .await?;
        Ok(row.map(|(status,)| SessionKernelState { status }))
    }
}
