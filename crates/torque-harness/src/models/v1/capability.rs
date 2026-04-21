use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Default, sqlx::Type, Serialize, Deserialize)]
#[sqlx(rename_all = "snake_case")]
pub enum RiskLevel {
    #[default]
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Default, sqlx::Type, Serialize, Deserialize)]
#[sqlx(rename_all = "snake_case")]
pub enum QualityTier {
    #[default]
    Experimental,
    Beta,
    Production,
}

#[derive(Debug, Serialize, FromRow)]
pub struct CapabilityProfile {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub input_contract: Option<serde_json::Value>,
    pub output_contract: Option<serde_json::Value>,
    pub risk_level: RiskLevel,
    pub default_agent_definition_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CapabilityProfileCreate {
    pub name: String,
    pub description: Option<String>,
    pub input_contract: Option<serde_json::Value>,
    pub output_contract: Option<serde_json::Value>,
    #[serde(default)]
    pub risk_level: RiskLevel,
    pub default_agent_definition_id: Option<Uuid>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct CapabilityRegistryBinding {
    pub id: Uuid,
    pub capability_profile_id: Uuid,
    pub agent_definition_id: Uuid,
    pub compatibility_score: Option<f64>,
    pub quality_tier: QualityTier,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CapabilityRegistryBindingCreate {
    pub capability_profile_id: Uuid,
    pub agent_definition_id: Uuid,
    pub compatibility_score: Option<f64>,
    #[serde(default)]
    pub quality_tier: QualityTier,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct CapabilityResolveRequest {
    pub team_instance_id: Option<Uuid>,
    pub team_task_id: Option<Uuid>,
    pub selector_id: Option<String>,
    pub constraints: Option<serde_json::Value>,
}
