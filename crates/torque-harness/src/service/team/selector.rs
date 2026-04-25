use crate::models::v1::team::{CandidateMember, MemberSelector, PolicyCheckSummary, SelectorType};
use crate::repository::{
    AgentInstanceRepository, CapabilityProfileRepository, CapabilityRegistryBindingRepository,
    TeamMemberRepository,
};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

pub struct SelectorResolver {
    team_member_repo: Arc<dyn TeamMemberRepository>,
    agent_instance_repo: Arc<dyn AgentInstanceRepository>,
    capability_profile_repo: Arc<dyn CapabilityProfileRepository>,
    capability_binding_repo: Arc<dyn CapabilityRegistryBindingRepository>,
}

impl SelectorResolver {
    pub fn new(
        team_member_repo: Arc<dyn TeamMemberRepository>,
        agent_instance_repo: Arc<dyn AgentInstanceRepository>,
        capability_profile_repo: Arc<dyn CapabilityProfileRepository>,
        capability_binding_repo: Arc<dyn CapabilityRegistryBindingRepository>,
    ) -> Self {
        Self {
            team_member_repo,
            agent_instance_repo,
            capability_profile_repo,
            capability_binding_repo,
        }
    }

    pub fn team_member_repo(&self) -> Arc<dyn TeamMemberRepository> {
        self.team_member_repo.clone()
    }

    pub async fn resolve(
        &self,
        selector: &MemberSelector,
        team_instance_id: Uuid,
    ) -> anyhow::Result<Vec<CandidateMember>> {
        let members = self
            .team_member_repo
            .list_by_team(team_instance_id, 100)
            .await?;

        let capable_agent_ids = self
            .resolve_capable_agents(&selector.capability_profiles)
            .await?;

        let agent_instance_ids: Vec<Uuid> = members.iter().map(|m| m.agent_instance_id).collect();

        let agent_instances = self
            .agent_instance_repo
            .get_many(&agent_instance_ids)
            .await?;

        let agent_instance_map: HashMap<Uuid, _> = agent_instances
            .into_iter()
            .map(|inst| (inst.id, inst))
            .collect();

        let mut candidates = Vec::new();
        for member in members.into_iter() {
            if !self.member_matches_selector(&member, selector) {
                continue;
            }

            let agent_instance = match agent_instance_map.get(&member.agent_instance_id) {
                Some(inst) => inst,
                None => continue,
            };

            if selector.selector_type == SelectorType::Capability
                && !capable_agent_ids.contains(&agent_instance.agent_definition_id)
            {
                continue;
            }

            if let Some(direct_id) = &selector.agent_definition_id {
                if &agent_instance.agent_definition_id != direct_id {
                    continue;
                }
            }

            let member_capabilities = self
                .get_member_capabilities(agent_instance.agent_definition_id)
                .await?;

            candidates.push(CandidateMember {
                team_member_id: member.id,
                agent_instance_id: member.agent_instance_id,
                agent_definition_id: agent_instance.agent_definition_id,
                role: member.role.clone(),
                capability_profiles: member_capabilities,
                selection_rationale: format!(
                    "Matched {:?} selector with role: {}",
                    selector.selector_type, member.role
                ),
                policy_check_summary: PolicyCheckSummary {
                    resource_available: true,
                    approval_required: false,
                    risk_level: "low".to_string(),
                },
            });
        }

        Ok(candidates)
    }

    async fn resolve_capable_agents(
        &self,
        profile_names: &[String],
    ) -> anyhow::Result<std::collections::HashSet<Uuid>> {
        if profile_names.is_empty() {
            return Ok(std::collections::HashSet::new());
        }

        let profiles = self.capability_profile_repo.list(100).await?;
        let matching_profile_ids: Vec<Uuid> = profiles
            .into_iter()
            .filter(|p| {
                profile_names
                    .iter()
                    .any(|name| p.name.to_lowercase().contains(&name.to_lowercase()))
            })
            .map(|p| p.id)
            .collect();

        if matching_profile_ids.is_empty() {
            return Ok(std::collections::HashSet::new());
        }

        let bindings = self.capability_binding_repo.list(500).await?;
        let capable_agents: std::collections::HashSet<Uuid> = bindings
            .into_iter()
            .filter(|b| matching_profile_ids.contains(&b.capability_profile_id))
            .map(|b| b.agent_definition_id)
            .collect();

        Ok(capable_agents)
    }

    async fn get_member_capabilities(
        &self,
        agent_definition_id: Uuid,
    ) -> anyhow::Result<Vec<String>> {
        let bindings = self.capability_binding_repo.list(500).await?;
        let profile_ids: Vec<Uuid> = bindings
            .into_iter()
            .filter(|b| b.agent_definition_id == agent_definition_id)
            .map(|b| b.capability_profile_id)
            .collect();

        if profile_ids.is_empty() {
            return Ok(vec![]);
        }

        let profiles = self.capability_profile_repo.list(100).await?;
        let capability_names: Vec<String> = profiles
            .into_iter()
            .filter(|p| profile_ids.contains(&p.id))
            .map(|p| p.name)
            .collect();

        Ok(capability_names)
    }

    fn member_matches_selector(
        &self,
        member: &crate::models::v1::team::TeamMember,
        selector: &MemberSelector,
    ) -> bool {
        match selector.selector_type {
            SelectorType::Role => selector.role.as_ref().map_or(true, |r| &member.role == r),
            SelectorType::Any => true,
            SelectorType::Capability | SelectorType::Direct => true,
        }
    }
}
