use crate::models::v1::event::Event;
use crate::repository::event_ext::EventRepositoryExt;
use std::sync::Arc;
use uuid::Uuid;

pub struct EventService {
    repo: Arc<dyn EventRepositoryExt>,
}

impl EventService {
    pub fn new(repo: Arc<dyn EventRepositoryExt>) -> Self {
        Self { repo }
    }

    pub async fn list_by_resource(
        &self,
        resource_type: &str,
        resource_id: Uuid,
        event_types: &[String],
        limit: i64,
    ) -> anyhow::Result<Vec<Event>> {
        self.repo.list_by_types(resource_type, resource_id, event_types, limit).await
    }
}
