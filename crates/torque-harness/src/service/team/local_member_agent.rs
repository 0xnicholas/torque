use super::*;
use async_trait::async_trait;
use uuid::Uuid;
use crate::models::v1::PartialQuality;

pub struct LocalMemberAgent {
    member_id: Uuid,
}

impl LocalMemberAgent {
    pub fn new(member_id: Uuid) -> Self {
        Self { member_id }
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
        Ok(vec![])
    }

    async fn accept_task(&self, _delegation_id: Uuid) -> anyhow::Result<()> {
        Ok(())
    }

    async fn complete_task(&self, _delegation_id: Uuid, _artifact_id: Uuid) -> anyhow::Result<()> {
        Ok(())
    }

    async fn fail_task(&self, _delegation_id: Uuid, _error: &str) -> anyhow::Result<()> {
        Ok(())
    }

    async fn timeout_partial(&self, _delegation_id: Uuid, _partial_quality: PartialQuality) -> anyhow::Result<()> {
        Ok(())
    }

    async fn request_extension(&self, _delegation_id: Uuid, _seconds: u32, _reason: &str) -> anyhow::Result<bool> {
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