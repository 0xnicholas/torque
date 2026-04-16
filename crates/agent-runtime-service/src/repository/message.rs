use async_trait::async_trait;
use crate::db::Database;
use crate::models::Message;
use uuid::Uuid;

#[async_trait]
pub trait MessageRepository: Send + Sync {
    async fn create(&self, msg: &Message,
    ) -> anyhow::Result<Message>;
    async fn list_by_session(
        &self,
        session_id: Uuid,
        limit: i64,
    ) -> anyhow::Result<Vec<Message>>;
    async fn get_recent_by_session(
        &self,
        session_id: Uuid,
        limit: i64,
    ) -> anyhow::Result<Vec<Message>>;
}

#[allow(dead_code)]
pub struct PostgresMessageRepository {
    db: Database,
}

impl PostgresMessageRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

#[async_trait]
impl MessageRepository for PostgresMessageRepository {
    async fn create(&self, message: &Message) -> anyhow::Result<Message> {
        let msg = sqlx::query_as::<_, Message>(
            r#"
            INSERT INTO session_messages (id, session_id, role, content, tool_calls, artifacts, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING *
            "#,
        )
        .bind(message.id)
        .bind(message.session_id)
        .bind(&message.role)
        .bind(&message.content)
        .bind(&message.tool_calls)
        .bind(&message.artifacts)
        .bind(message.created_at)
        .fetch_one(self.db.pool())
        .await?;

        Ok(msg)
    }

    async fn list_by_session(
        &self,
        session_id: Uuid,
        limit: i64,
    ) -> anyhow::Result<Vec<Message>> {
        let messages = sqlx::query_as::<_, Message>(
            r#"
            SELECT * FROM session_messages
            WHERE session_id = $1
            ORDER BY created_at ASC
            LIMIT $2
            "#,
        )
        .bind(session_id)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;

        Ok(messages)
    }

    async fn get_recent_by_session(
        &self,
        session_id: Uuid,
        count: i64,
    ) -> anyhow::Result<Vec<Message>> {
        let mut messages = sqlx::query_as::<_, Message>(
            r#"
            SELECT * FROM session_messages
            WHERE session_id = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(session_id)
        .bind(count)
        .fetch_all(self.db.pool())
        .await?;

        messages.reverse();
        Ok(messages)
    }
}
