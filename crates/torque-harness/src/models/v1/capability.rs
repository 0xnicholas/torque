use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub enum RiskLevel {
    #[default]
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskLevel::Low => write!(f, "low"),
            RiskLevel::Medium => write!(f, "medium"),
            RiskLevel::High => write!(f, "high"),
            RiskLevel::Critical => write!(f, "critical"),
        }
    }
}

impl std::str::FromStr for RiskLevel {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "low" => Ok(RiskLevel::Low),
            "medium" => Ok(RiskLevel::Medium),
            "high" => Ok(RiskLevel::High),
            "critical" => Ok(RiskLevel::Critical),
            _ => Err(format!("Unknown RiskLevel: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub enum QualityTier {
    #[default]
    Experimental,
    Beta,
    Production,
}

impl std::fmt::Display for QualityTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QualityTier::Experimental => write!(f, "experimental"),
            QualityTier::Beta => write!(f, "beta"),
            QualityTier::Production => write!(f, "production"),
        }
    }
}

impl std::str::FromStr for QualityTier {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "experimental" => Ok(QualityTier::Experimental),
            "beta" => Ok(QualityTier::Beta),
            "production" => Ok(QualityTier::Production),
            _ => Err(format!("Unknown QualityTier: {}", s)),
        }
    }
}

mod sqlx_impls {
    use super::*;
    use sqlx::decode::Decode;
    use sqlx::encode::Encode;
    use sqlx::postgres::{PgArgumentBuffer, PgTypeInfo, PgValueRef};
    use sqlx::types::Type;

    impl Type<sqlx::Postgres> for RiskLevel {
        fn type_info() -> PgTypeInfo {
            PgTypeInfo::with_name("TEXT")
        }

        fn compatible(ty: &PgTypeInfo) -> bool {
            *ty == PgTypeInfo::with_name("TEXT")
                || *ty == PgTypeInfo::with_name("VARCHAR")
                || *ty == PgTypeInfo::with_name("UNKNOWN")
        }
    }

    impl Encode<'_, sqlx::Postgres> for RiskLevel {
        fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> sqlx::encode::IsNull {
            let s = self.to_string();
            buf.extend_from_slice(s.as_bytes());
            sqlx::encode::IsNull::No
        }
    }

    impl Decode<'_, sqlx::Postgres> for RiskLevel {
        fn decode(
            value: PgValueRef<'_>,
        ) -> Result<Self, Box<dyn std::error::Error + Send + Sync + 'static>> {
            let s: String = Decode::<sqlx::Postgres>::decode(value)?;
            s.as_str().parse().map_err(|e: String| e.into())
        }
    }

    impl Type<sqlx::Postgres> for QualityTier {
        fn type_info() -> PgTypeInfo {
            PgTypeInfo::with_name("TEXT")
        }

        fn compatible(ty: &PgTypeInfo) -> bool {
            *ty == PgTypeInfo::with_name("TEXT")
                || *ty == PgTypeInfo::with_name("VARCHAR")
                || *ty == PgTypeInfo::with_name("UNKNOWN")
        }
    }

    impl Encode<'_, sqlx::Postgres> for QualityTier {
        fn encode_by_ref(&self, buf: &mut PgArgumentBuffer) -> sqlx::encode::IsNull {
            let s = self.to_string();
            buf.extend_from_slice(s.as_bytes());
            sqlx::encode::IsNull::No
        }
    }

    impl Decode<'_, sqlx::Postgres> for QualityTier {
        fn decode(
            value: PgValueRef<'_>,
        ) -> Result<Self, Box<dyn std::error::Error + Send + Sync + 'static>> {
            let s: String = Decode::<sqlx::Postgres>::decode(value)?;
            s.as_str().parse().map_err(|e: String| e.into())
        }
    }
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedCandidate {
    pub capability_profile_id: Uuid,
    pub agent_definition_id: Uuid,
    pub match_rationale: String,
    pub policy_check_summary: Option<serde_json::Value>,
    pub risk_level: RiskLevel,
    pub quality_tier: QualityTier,
    pub compatibility_score: Option<f64>,
    pub cost_or_latency_estimate: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityResolution {
    pub capability_ref: String,
    pub capability_profile_id: Uuid,
    pub candidates: Vec<ResolvedCandidate>,
    pub resolved_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CapabilityResolveRequest {
    pub team_instance_id: Option<Uuid>,
    pub team_task_id: Option<Uuid>,
    pub selector_id: Option<String>,
    pub constraints: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityRef(pub String);

impl CapabilityRef {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Deserialize)]
pub struct CapabilityResolveByRefRequest {
    pub capability_ref: String,
    pub constraints: Option<serde_json::Value>,
}
