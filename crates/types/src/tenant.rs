use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tenant {
    pub id: Uuid,
    pub name: String,
    pub weight: i32,
    pub max_concurrency: i32,
    pub monthly_token_quota: Option<i64>,
    pub created_at: DateTime<Utc>,
}

impl Tenant {
    pub fn new(name: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            weight: 1,
            max_concurrency: 10,
            monthly_token_quota: None,
            created_at: Utc::now(),
        }
    }
}
