use crate::db::Database;
use crate::models::v1::event::Event;
use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait EventRepositoryExt: Send + Sync {
    async fn list(&self, limit: i64) -> anyhow::Result<Vec<Event>>;
    async fn list_by_types(
        &self,
        resource_type: &str,
        resource_id: Uuid,
        event_types: &[String],
        limit: i64,
    ) -> anyhow::Result<Vec<Event>>;
}

pub struct PostgresEventRepositoryExt {
    db: Database,
}

impl PostgresEventRepositoryExt {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

#[async_trait]
impl EventRepositoryExt for PostgresEventRepositoryExt {
    async fn list(&self, limit: i64) -> anyhow::Result<Vec<Event>> {
        let rows =
            sqlx::query_as::<_, Event>("SELECT * FROM v1_events ORDER BY timestamp DESC LIMIT $1")
                .bind(limit)
                .fetch_all(self.db.pool())
                .await?;
        Ok(rows)
    }

    async fn list_by_types(
        &self,
        resource_type: &str,
        resource_id: Uuid,
        event_types: &[String],
        limit: i64,
    ) -> anyhow::Result<Vec<Event>> {
        let rows = sqlx::query_as::<_, Event>(
            "SELECT * FROM v1_events WHERE resource_type = $1 AND resource_id = $2 AND event_type = ANY($3) ORDER BY timestamp DESC LIMIT $4"
        )
        .bind(resource_type)
        .bind(resource_id)
        .bind(event_types)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows)
    }
}
