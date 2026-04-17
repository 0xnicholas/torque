use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, sqlx::Type, Serialize, Deserialize)]
#[sqlx(rename_all = "snake_case")]
pub enum ArtifactScope {
    Private,
    TeamShared,
    ExternalPublished,
}

#[derive(Debug, Serialize, FromRow)]
pub struct Artifact {
    pub id: Uuid,
    pub kind: String,
    pub scope: ArtifactScope,
    pub source_instance_id: Option<Uuid>,
    pub published_to_team_instance_id: Option<Uuid>,
    pub mime_type: String,
    pub size_bytes: i64,
    pub summary: Option<String>,
    pub content: serde_json::Value,
    pub created_at: DateTime<Utc>,
}
