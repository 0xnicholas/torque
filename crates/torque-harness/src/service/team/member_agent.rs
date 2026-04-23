use crate::models::v1::PartialQuality;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[async_trait]
pub trait MemberAgent: Send + Sync {
    async fn start(&self) -> anyhow::Result<()>;
    async fn stop(&self) -> anyhow::Result<()>;
    async fn poll_tasks(&self) -> anyhow::Result<Vec<MemberTask>>;
    async fn accept_task(&self, delegation_id: Uuid) -> anyhow::Result<()>;
    async fn complete_task(&self, delegation_id: Uuid, artifact_id: Uuid) -> anyhow::Result<()>;
    async fn fail_task(&self, delegation_id: Uuid, error: &str) -> anyhow::Result<()>;
    async fn timeout_partial(
        &self,
        delegation_id: Uuid,
        partial_quality: PartialQuality,
    ) -> anyhow::Result<()>;
    async fn request_extension(
        &self,
        delegation_id: Uuid,
        seconds: u32,
        reason: &str,
    ) -> anyhow::Result<bool>;
    async fn health_check(&self) -> anyhow::Result<MemberHealth>;
}

#[derive(Debug, Clone)]
pub struct MemberTask {
    pub delegation_id: Uuid,
    pub task_id: Uuid,
    pub goal: String,
    pub instructions: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct MemberHealth {
    pub member_id: Uuid,
    pub is_healthy: bool,
    pub active_tasks: usize,
    pub completed_tasks: usize,
    pub failed_tasks: usize,
}
