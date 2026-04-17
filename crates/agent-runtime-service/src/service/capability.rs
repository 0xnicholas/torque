use crate::models::v1::capability::{
    CapabilityProfile, CapabilityProfileCreate, CapabilityRegistryBinding,
    CapabilityRegistryBindingCreate, CapabilityResolveRequest,
};
use crate::repository::{
    CapabilityProfileRepository, CapabilityRegistryBindingRepository,
};
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
        Self { profile_repo, binding_repo }
    }

    pub async fn create_profile(
        &self,
        req: CapabilityProfileCreate,
    ) -> anyhow::Result<CapabilityProfile> {
        self.profile_repo.create(&req).await
    }

    pub async fn list_profiles(
        &self, limit: i64) -> anyhow::Result<Vec<CapabilityProfile>> {
        self.profile_repo.list(limit).await
    }

    pub async fn get_profile(
        &self, id: Uuid) -> anyhow::Result<Option<CapabilityProfile>> {
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
        &self, limit: i64) -> anyhow::Result<Vec<CapabilityRegistryBinding>> {
        self.binding_repo.list(limit).await
    }

    pub async fn get_binding(
        &self, id: Uuid) -> anyhow::Result<Option<CapabilityRegistryBinding>> {
        self.binding_repo.get(id).await
    }

    pub async fn delete_binding(&self, id: Uuid) -> anyhow::Result<bool> {
        self.binding_repo.delete(id).await
    }

    pub async fn resolve(
        &self, _req: CapabilityResolveRequest) -> anyhow::Result<serde_json::Value> {
        Ok(serde_json::json!({"candidates": []}))
    }
}
