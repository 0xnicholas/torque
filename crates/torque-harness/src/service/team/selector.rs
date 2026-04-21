use crate::models::v1::team::{CandidateMember, MemberSelector, PolicyCheckSummary, SelectorType};
use crate::repository::{AgentInstanceRepository, TeamMemberRepository};
use std::sync::Arc;
use uuid::Uuid;

pub struct SelectorResolver {
    team_member_repo: Arc<dyn TeamMemberRepository>,
    agent_instance_repo: Arc<dyn AgentInstanceRepository>,
}

impl SelectorResolver {
    pub fn new(
        team_member_repo: Arc<dyn TeamMemberRepository>,
        agent_instance_repo: Arc<dyn AgentInstanceRepository>,
    ) -> Self {
        Self {
            team_member_repo,
            agent_instance_repo,
        }
    }

    pub async fn resolve(
        &self,
        selector: &MemberSelector,
        team_instance_id: Uuid,
    ) -> anyhow::Result<Vec<CandidateMember>> {
        let members = self.team_member_repo.list_by_team(team_instance_id, 100).await?;

        let mut candidates = Vec::new();
        for member in members.into_iter() {
            if !self.member_matches_selector(&member, selector) {
                continue;
            }

            let agent_definition_id = match self.agent_instance_repo.get(member.agent_instance_id).await {
                Ok(Some(inst)) => inst.agent_definition_id,
                _ => continue,
            };

            candidates.push(CandidateMember {
                team_member_id: member.id,
                agent_instance_id: member.agent_instance_id,
                agent_definition_id,
                role: member.role.clone(),
                capability_profiles: vec![],
                selection_rationale: format!("Matched {:?} selector with role: {}", selector.selector_type, member.role),
                policy_check_summary: PolicyCheckSummary {
                    resource_available: true,
                    approval_required: false,
                    risk_level: "low".to_string(),
                },
            });
        }

        Ok(candidates)
    }

    fn member_matches_selector(
        &self,
        member: &crate::models::v1::team::TeamMember,
        selector: &MemberSelector,
    ) -> bool {
        match selector.selector_type {
            SelectorType::Role => {
                selector.role.as_ref().map_or(true, |r| &member.role == r)
            }
            SelectorType::Any => true,
            SelectorType::Capability | SelectorType::Direct => true,
        }
    }
}