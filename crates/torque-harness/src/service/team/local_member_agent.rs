use super::*;
use crate::models::v1::delegation::DelegationStatus;
use crate::models::v1::PartialQuality;
use crate::repository::{AgentInstanceRepository, DelegationRepository};
use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

pub struct LocalMemberAgent {
    member_id: Uuid,
    delegation_repo: Arc<dyn DelegationRepository>,
    agent_instance_repo: Arc<dyn AgentInstanceRepository>,
}

impl LocalMemberAgent {
    pub fn new(
        member_id: Uuid,
        delegation_repo: Arc<dyn DelegationRepository>,
        agent_instance_repo: Arc<dyn AgentInstanceRepository>,
    ) -> Self {
        Self {
            member_id,
            delegation_repo,
            agent_instance_repo,
        }
    }
}

#[async_trait]
impl MemberAgent for LocalMemberAgent {
    async fn start(&self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn stop(&self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn poll_tasks(&self) -> anyhow::Result<Vec<MemberTask>> {
        let delegations = self
            .delegation_repo
            .list_by_instance(self.member_id, 100)
            .await?;

        let tasks: Vec<MemberTask> = delegations
            .into_iter()
            .filter(|d| {
                matches!(
                    d.status,
                    DelegationStatus::Pending | DelegationStatus::Accepted
                )
            })
            .filter_map(|d| {
                let selector = &d.child_agent_definition_selector;
                let goal = selector.get("goal")?.as_str()?.to_string();
                let instructions = selector
                    .get("instructions")
                    .and_then(|v| v.as_str())
                    .map(String::from);

                Some(MemberTask {
                    delegation_id: d.id,
                    task_id: d.task_id,
                    goal,
                    instructions,
                    created_at: d.created_at,
                })
            })
            .collect();

        Ok(tasks)
    }

    async fn accept_task(&self, delegation_id: Uuid) -> anyhow::Result<()> {
        self.delegation_repo
            .update_status(delegation_id, "ACCEPTED")
            .await?;
        Ok(())
    }

    async fn complete_task(&self, delegation_id: Uuid, artifact_id: Uuid) -> anyhow::Result<()> {
        self.delegation_repo
            .complete(delegation_id, artifact_id)
            .await?;
        Ok(())
    }

    async fn fail_task(&self, delegation_id: Uuid, error: &str) -> anyhow::Result<()> {
        self.delegation_repo
            .fail(delegation_id, error)
            .await?;
        Ok(())
    }

    async fn timeout_partial(
        &self,
        _delegation_id: Uuid,
        _partial_quality: PartialQuality,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn request_extension(
        &self,
        _delegation_id: Uuid,
        _seconds: u32,
        _reason: &str,
    ) -> anyhow::Result<bool> {
        Ok(false)
    }

    async fn health_check(&self) -> anyhow::Result<MemberHealth> {
        Ok(MemberHealth {
            member_id: self.member_id,
            is_healthy: true,
            active_tasks: 0,
            completed_tasks: 0,
            failed_tasks: 0,
        })
    }
}
