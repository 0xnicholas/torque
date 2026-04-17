use async_trait::async_trait;
use crate::db::Database;
use crate::models::v1::capability::{
    CapabilityProfile, CapabilityProfileCreate, CapabilityRegistryBinding,
    CapabilityRegistryBindingCreate,
};
use uuid::Uuid;

#[async_trait]
pub trait CapabilityProfileRepository: Send + Sync {
    async fn create(&self, req: &CapabilityProfileCreate) -> anyhow::Result<CapabilityProfile>;
    async fn list(&self, limit: i64) -> anyhow::Result<Vec<CapabilityProfile>>;
    async fn get(&self, id: Uuid) -> anyhow::Result<Option<CapabilityProfile>>;
    async fn delete(&self, id: Uuid) -> anyhow::Result<bool>;
}

pub struct PostgresCapabilityProfileRepository {
    db: Database,
}

impl PostgresCapabilityProfileRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

#[async_trait]
impl CapabilityProfileRepository for PostgresCapabilityProfileRepository {
    async fn create(&self, req: &CapabilityProfileCreate) -> anyhow::Result<CapabilityProfile> {
        let row = sqlx::query_as::<_, CapabilityProfile>(
            "INSERT INTO v1_capability_profiles (name, description, input_contract, output_contract, risk_level, default_agent_definition_id) VALUES ($1, $2, $3, $4, $5, $6) RETURNING *"
        )
        .bind(&req.name)
        .bind(&req.description)
        .bind(&req.input_contract)
        .bind(&req.output_contract)
        .bind(req.risk_level.clone())
        .bind(req.default_agent_definition_id)
        .fetch_one(self.db.pool())
        .await?;
        Ok(row)
    }

    async fn list(&self, limit: i64) -> anyhow::Result<Vec<CapabilityProfile>> {
        let rows = sqlx::query_as::<_, CapabilityProfile>(
            "SELECT * FROM v1_capability_profiles ORDER BY created_at DESC LIMIT $1"
        )
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows)
    }

    async fn get(&self, id: Uuid) -> anyhow::Result<Option<CapabilityProfile>> {
        let row = sqlx::query_as::<_, CapabilityProfile>(
            "SELECT * FROM v1_capability_profiles WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(self.db.pool())
        .await?;
        Ok(row)
    }

    async fn delete(&self, id: Uuid) -> anyhow::Result<bool> {
        let result = sqlx::query("DELETE FROM v1_capability_profiles WHERE id = $1")
            .bind(id)
            .execute(self.db.pool())
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

#[async_trait]
pub trait CapabilityRegistryBindingRepository: Send + Sync {
    async fn create(
        &self,
        req: &CapabilityRegistryBindingCreate,
    ) -> anyhow::Result<CapabilityRegistryBinding>;
    async fn list(&self, limit: i64) -> anyhow::Result<Vec<CapabilityRegistryBinding>>;
    async fn get(&self, id: Uuid) -> anyhow::Result<Option<CapabilityRegistryBinding>>;
    async fn delete(&self, id: Uuid) -> anyhow::Result<bool>;
}

pub struct PostgresCapabilityRegistryBindingRepository {
    db: Database,
}

impl PostgresCapabilityRegistryBindingRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

#[async_trait]
impl CapabilityRegistryBindingRepository for PostgresCapabilityRegistryBindingRepository {
    async fn create(
        &self,
        req: &CapabilityRegistryBindingCreate,
    ) -> anyhow::Result<CapabilityRegistryBinding> {
        let row = sqlx::query_as::<_, CapabilityRegistryBinding>(
            "INSERT INTO v1_capability_registry_bindings (capability_profile_id, agent_definition_id, compatibility_score, quality_tier, metadata) VALUES ($1, $2, $3, $4, $5) RETURNING *"
        )
        .bind(req.capability_profile_id)
        .bind(req.agent_definition_id)
        .bind(req.compatibility_score)
        .bind(req.quality_tier.clone())
        .bind(&req.metadata)
        .fetch_one(self.db.pool())
        .await?;
        Ok(row)
    }

    async fn list(&self, limit: i64) -> anyhow::Result<Vec<CapabilityRegistryBinding>> {
        let rows = sqlx::query_as::<_, CapabilityRegistryBinding>(
            "SELECT * FROM v1_capability_registry_bindings ORDER BY created_at DESC LIMIT $1"
        )
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows)
    }

    async fn get(&self, id: Uuid) -> anyhow::Result<Option<CapabilityRegistryBinding>> {
        let row = sqlx::query_as::<_, CapabilityRegistryBinding>(
            "SELECT * FROM v1_capability_registry_bindings WHERE id = $1"
        )
        .bind(id)
        .fetch_optional(self.db.pool())
        .await?;
        Ok(row)
    }

    async fn delete(&self, id: Uuid) -> anyhow::Result<bool> {
        let result = sqlx::query("DELETE FROM v1_capability_registry_bindings WHERE id = $1")
            .bind(id)
            .execute(self.db.pool())
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
