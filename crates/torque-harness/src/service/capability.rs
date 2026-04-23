use crate::models::v1::capability::{
    CapabilityProfile, CapabilityProfileCreate, CapabilityRegistryBinding,
    CapabilityRegistryBindingCreate, CapabilityResolveRequest,
    CapabilityResolution, ResolvedCandidate,
};
use crate::repository::{CapabilityProfileRepository, CapabilityRegistryBindingRepository};
use std::sync::Arc;
use uuid::Uuid;

pub struct CapabilityService {
    profile_repo: Arc<dyn CapabilityProfileRepository>,
    binding_repo: Arc<dyn CapabilityRegistryBindingRepository>,
}

impl CapabilityService {
    pub fn new(
        profile_repo: Arc<dyn CapabilityProfileRepository>,
        binding_repo: Arc<dyn CapabilityRegistryBindingRepository>,
    ) -> Self {
        Self {
            profile_repo,
            binding_repo,
        }
    }

    pub async fn create_profile(
        &self,
        req: CapabilityProfileCreate,
    ) -> anyhow::Result<CapabilityProfile> {
        self.profile_repo.create(&req).await
    }

    pub async fn list_profiles(&self, limit: i64) -> anyhow::Result<Vec<CapabilityProfile>> {
        self.profile_repo.list(limit).await
    }

    pub async fn get_profile(&self, id: Uuid) -> anyhow::Result<Option<CapabilityProfile>> {
        self.profile_repo.get(id).await
    }

    pub async fn delete_profile(&self, id: Uuid) -> anyhow::Result<bool> {
        self.profile_repo.delete(id).await
    }

    pub async fn create_binding(
        &self,
        req: CapabilityRegistryBindingCreate,
    ) -> anyhow::Result<CapabilityRegistryBinding> {
        self.binding_repo.create(&req).await
    }

    pub async fn list_bindings(
        &self,
        limit: i64,
    ) -> anyhow::Result<Vec<CapabilityRegistryBinding>> {
        self.binding_repo.list(limit).await
    }

    pub async fn get_binding(&self, id: Uuid) -> anyhow::Result<Option<CapabilityRegistryBinding>> {
        self.binding_repo.get(id).await
    }

    pub async fn delete_binding(&self, id: Uuid) -> anyhow::Result<bool> {
        self.binding_repo.delete(id).await
    }

    pub async fn resolve_by_ref(
        &self,
        capability_ref: &str,
        _constraints: Option<serde_json::Value>,
    ) -> anyhow::Result<CapabilityResolution> {
        let profile = self.profile_repo.get_by_name(capability_ref).await?
            .ok_or_else(|| anyhow::anyhow!("Capability profile not found: {}", capability_ref))?;

        let bindings = self.binding_repo.list_by_profile(profile.id, 10).await?;

        let candidates: Vec<ResolvedCandidate> = bindings.into_iter().map(|b| {
            ResolvedCandidate {
                capability_profile_id: b.capability_profile_id,
                agent_definition_id: b.agent_definition_id,
                match_rationale: "Direct binding match".to_string(),
                policy_check_summary: None,
                risk_level: profile.risk_level.clone(),
                quality_tier: b.quality_tier,
                compatibility_score: b.compatibility_score,
                cost_or_latency_estimate: None,
            }
        }).collect();

        Ok(CapabilityResolution {
            capability_ref: capability_ref.to_string(),
            capability_profile_id: profile.id,
            candidates,
            resolved_at: chrono::Utc::now(),
        })
    }

    pub async fn resolve(
        &self,
        req: CapabilityResolveRequest,
    ) -> anyhow::Result<CapabilityResolution> {
        let capability_ref = req.constraints
            .as_ref()
            .and_then(|c| c.get("capability_ref"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("capability_ref required in constraints"))?
            .to_string();

        self.resolve_by_ref(&capability_ref, req.constraints).await
    }
}
