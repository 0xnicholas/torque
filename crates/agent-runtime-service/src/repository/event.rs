use async_trait::async_trait;
use crate::db::Database;
use crate::models::v1::event::Event;
use uuid::Uuid;

#[async_trait]
pub trait EventRepository: Send + Sync {
    async fn create(&self, event: Event) -> anyhow::Result<()>;
    async fn create_batch(
        &self,
        _events: Vec<Event>,
    ) -> anyhow::Result<()>;
    async fn list_by_resource(
        &self,
        resource_type: &str,
        resource_id: Uuid,
        limit: i64,
    ) -> anyhow::Result<Vec<Event>>;
}

pub struct PostgresEventRepository {
    db: Database,
}

impl PostgresEventRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

#[async_trait]
impl EventRepository for PostgresEventRepository {
    async fn create(&self, event: Event) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO v1_events (event_id, event_type, timestamp, resource_type, resource_id, payload, sequence_number)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#
        )
        .bind(event.event_id)
        .bind(event.event_type)
        .bind(event.timestamp)
        .bind(event.resource_type)
        .bind(event.resource_id)
        .bind(event.payload)
        .bind(event.sequence_number)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    async fn create_batch(
        &self,
        events: Vec<Event>,
    ) -> anyhow::Result<()> {
        let mut tx = self.db.pool().begin().await?;
        for event in events {
            sqlx::query(
                r#"
                INSERT INTO v1_events (event_id, event_type, timestamp, resource_type, resource_id, payload, sequence_number)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                "#
            )
            .bind(event.event_id)
            .bind(event.event_type)
            .bind(event.timestamp)
            .bind(event.resource_type)
            .bind(event.resource_id)
            .bind(event.payload)
            .bind(event.sequence_number)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn list_by_resource(
        &self,
        resource_type: &str,
        resource_id: Uuid,
        limit: i64,
    ) -> anyhow::Result<Vec<Event>> {
        let rows = sqlx::query_as::<_, Event>(
            "SELECT * FROM v1_events WHERE resource_type = $1 AND resource_id = $2 ORDER BY timestamp DESC LIMIT $3"
        )
        .bind(resource_type)
        .bind(resource_id)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows)
    }
}
