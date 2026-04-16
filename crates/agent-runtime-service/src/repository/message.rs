use async_trait::async_trait;
use crate::db::Database;
use crate::models::Message;
use uuid::Uuid;

#[async_trait]
pub trait MessageRepository: Send + Sync {
    async fn create(&self, msg: &Message) -> anyhow::Result<Message>;
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
    async fn create(&self, _msg: &Message) -> anyhow::Result<Message> {
        todo!("migrate from db/messages.rs")
    }

    async fn list_by_session(
        &self,
        _session_id: Uuid,
        _limit: i64,
    ) -> anyhow::Result<Vec<Message>> {
        todo!("migrate from db/messages.rs")
    }

    async fn get_recent_by_session(
        &self,
        _session_id: Uuid,
        _limit: i64,
    ) -> anyhow::Result<Vec<Message>> {
        todo!("migrate from db/messages.rs")
    }
}
