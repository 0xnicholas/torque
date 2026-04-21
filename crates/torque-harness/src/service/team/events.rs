use crate::models::v1::team::{TeamEvent, TeamEventType};
use crate::repository::TeamEventRepository;
use chrono::Utc;
use std::sync::Arc;
use uuid::Uuid;

pub struct TeamEventEmitter {
    repo: Arc<dyn TeamEventRepository>,
}

impl TeamEventEmitter {
    pub fn new(repo: Arc<dyn TeamEventRepository>) -> Self {
        Self { repo }
    }

    pub async fn emit(
        &self,
        team_instance_id: Uuid,
        event_type: TeamEventType,
        actor_ref: &str,
        team_task_ref: Option<Uuid>,
        related_instance_refs: Vec<Uuid>,
        related_artifact_refs: Vec<Uuid>,
        payload: serde_json::Value,
        causal_event_refs: Vec<Uuid>,
    ) -> anyhow::Result<TeamEvent> {
        let event = TeamEvent {
            id: Uuid::new_v4(),
            team_instance_id,
            event_type: event_type.to_string(),
            timestamp: Utc::now(),
            actor_ref: actor_ref.to_string(),
            team_task_ref,
            related_instance_refs,
            related_artifact_refs,
            payload,
            causal_event_refs,
        };
        self.repo.create(&event).await
    }

    pub async fn task_received(&self, team_instance_id: Uuid, task_id: Uuid) -> anyhow::Result<TeamEvent> {
        self.emit(
            team_instance_id,
            TeamEventType::TeamTaskReceived,
            "system",
            Some(task_id),
            vec![],
            vec![],
            serde_json::json!({}),
            vec![],
        ).await
    }

    pub async fn triage_completed(&self, team_instance_id: Uuid, task_id: Uuid, triage_result: &crate::models::v1::team::TriageResult) -> anyhow::Result<TeamEvent> {
        self.emit(
            team_instance_id,
            TeamEventType::TriageCompleted,
            "supervisor",
            Some(task_id),
            vec![],
            vec![],
            serde_json::json!({"triage_result": triage_result}),
            vec![],
        ).await
    }

    pub async fn mode_selected(&self, team_instance_id: Uuid, task_id: Uuid, mode: &crate::models::v1::team::TeamMode) -> anyhow::Result<TeamEvent> {
        self.emit(
            team_instance_id,
            TeamEventType::ModeSelected,
            "supervisor",
            Some(task_id),
            vec![],
            vec![],
            serde_json::json!({"mode": mode}),
            vec![],
        ).await
    }

    pub async fn lead_assigned(&self, team_instance_id: Uuid, task_id: Uuid, member_id: Uuid, causal_event_refs: Vec<Uuid>) -> anyhow::Result<TeamEvent> {
        self.emit(
            team_instance_id,
            TeamEventType::LeadAssigned,
            "supervisor",
            Some(task_id),
            vec![member_id],
            vec![],
            serde_json::json!({}),
            causal_event_refs,
        ).await
    }

    pub async fn member_activated(&self, team_instance_id: Uuid, task_id: Uuid, member_id: Uuid, role: &str, causal_event_refs: Vec<Uuid>) -> anyhow::Result<TeamEvent> {
        self.emit(
            team_instance_id,
            TeamEventType::MemberActivated,
            "supervisor",
            Some(task_id),
            vec![member_id],
            vec![],
            serde_json::json!({"role": role}),
            causal_event_refs,
        ).await
    }

    pub async fn delegation_created(&self, team_instance_id: Uuid, task_id: Uuid, delegation_id: Uuid, member_id: Uuid, causal_event_refs: Vec<Uuid>) -> anyhow::Result<TeamEvent> {
        self.emit(
            team_instance_id,
            TeamEventType::DelegationCreated,
            "supervisor",
            Some(task_id),
            vec![member_id],
            vec![],
            serde_json::json!({"delegation_id": delegation_id}),
            causal_event_refs,
        ).await
    }

    pub async fn delegation_accepted(&self, team_instance_id: Uuid, task_id: Uuid, delegation_id: Uuid, member_id: Uuid, causal_event_refs: Vec<Uuid>) -> anyhow::Result<TeamEvent> {
        self.emit(
            team_instance_id,
            TeamEventType::DelegationAccepted,
            "supervisor",
            Some(task_id),
            vec![member_id],
            vec![],
            serde_json::json!({"delegation_id": delegation_id}),
            causal_event_refs,
        ).await
    }

    pub async fn delegation_rejected(&self, team_instance_id: Uuid, task_id: Uuid, delegation_id: Uuid, member_id: Uuid, reason: &str, causal_event_refs: Vec<Uuid>) -> anyhow::Result<TeamEvent> {
        self.emit(
            team_instance_id,
            TeamEventType::DelegationRejected,
            "supervisor",
            Some(task_id),
            vec![member_id],
            vec![],
            serde_json::json!({"delegation_id": delegation_id, "reason": reason}),
            causal_event_refs,
        ).await
    }

    pub async fn member_result_received(&self, team_instance_id: Uuid, task_id: Uuid, delegation_id: Uuid, member_id: Uuid, causal_event_refs: Vec<Uuid>) -> anyhow::Result<TeamEvent> {
        self.emit(
            team_instance_id,
            TeamEventType::MemberResultReceived,
            "supervisor",
            Some(task_id),
            vec![member_id],
            vec![],
            serde_json::json!({"delegation_id": delegation_id}),
            causal_event_refs,
        ).await
    }

    pub async fn member_result_accepted(&self, team_instance_id: Uuid, task_id: Uuid, delegation_id: Uuid, member_id: Uuid, causal_event_refs: Vec<Uuid>) -> anyhow::Result<TeamEvent> {
        self.emit(
            team_instance_id,
            TeamEventType::MemberResultAccepted,
            "supervisor",
            Some(task_id),
            vec![member_id],
            vec![],
            serde_json::json!({"delegation_id": delegation_id}),
            causal_event_refs,
        ).await
    }

    pub async fn member_result_rejected(&self, team_instance_id: Uuid, task_id: Uuid, delegation_id: Uuid, member_id: Uuid, reason: &str, causal_event_refs: Vec<Uuid>) -> anyhow::Result<TeamEvent> {
        self.emit(
            team_instance_id,
            TeamEventType::MemberResultRejected,
            "supervisor",
            Some(task_id),
            vec![member_id],
            vec![],
            serde_json::json!({"delegation_id": delegation_id, "reason": reason}),
            causal_event_refs,
        ).await
    }

    pub async fn artifact_published(&self, team_instance_id: Uuid, task_id: Uuid, artifact_id: Uuid, scope: &crate::models::v1::team::PublishScope, causal_event_refs: Vec<Uuid>) -> anyhow::Result<TeamEvent> {
        self.emit(
            team_instance_id,
            TeamEventType::ArtifactPublished,
            "supervisor",
            Some(task_id),
            vec![],
            vec![artifact_id],
            serde_json::json!({"scope": scope}),
            causal_event_refs,
        ).await
    }

    pub async fn fact_published(&self, team_instance_id: Uuid, task_id: Uuid, key: &str, value: &serde_json::Value, causal_event_refs: Vec<Uuid>) -> anyhow::Result<TeamEvent> {
        self.emit(
            team_instance_id,
            TeamEventType::FactPublished,
            "supervisor",
            Some(task_id),
            vec![],
            vec![],
            serde_json::json!({"key": key, "value": value}),
            causal_event_refs,
        ).await
    }

    pub async fn blocker_added(&self, team_instance_id: Uuid, task_id: Uuid, blocker_id: Uuid, description: &str, causal_event_refs: Vec<Uuid>) -> anyhow::Result<TeamEvent> {
        self.emit(
            team_instance_id,
            TeamEventType::BlockerAdded,
            "supervisor",
            Some(task_id),
            vec![],
            vec![],
            serde_json::json!({"blocker_id": blocker_id, "description": description}),
            causal_event_refs,
        ).await
    }

    pub async fn blocker_resolved(&self, team_instance_id: Uuid, task_id: Uuid, blocker_id: Uuid, causal_event_refs: Vec<Uuid>) -> anyhow::Result<TeamEvent> {
        self.emit(
            team_instance_id,
            TeamEventType::BlockerResolved,
            "supervisor",
            Some(task_id),
            vec![],
            vec![],
            serde_json::json!({"blocker_id": blocker_id}),
            causal_event_refs,
        ).await
    }

    pub async fn approval_requested(&self, team_instance_id: Uuid, task_id: Uuid, approval_id: Uuid, causal_event_refs: Vec<Uuid>) -> anyhow::Result<TeamEvent> {
        self.emit(
            team_instance_id,
            TeamEventType::ApprovalRequested,
            "supervisor",
            Some(task_id),
            vec![],
            vec![],
            serde_json::json!({"approval_id": approval_id}),
            causal_event_refs,
        ).await
    }

    pub async fn team_blocked(&self, team_instance_id: Uuid, task_id: Uuid, reason: &str, causal_event_refs: Vec<Uuid>) -> anyhow::Result<TeamEvent> {
        self.emit(
            team_instance_id,
            TeamEventType::TeamBlocked,
            "supervisor",
            Some(task_id),
            vec![],
            vec![],
            serde_json::json!({"reason": reason}),
            causal_event_refs,
        ).await
    }

    pub async fn team_unblocked(&self, team_instance_id: Uuid, task_id: Uuid, causal_event_refs: Vec<Uuid>) -> anyhow::Result<TeamEvent> {
        self.emit(
            team_instance_id,
            TeamEventType::TeamUnblocked,
            "supervisor",
            Some(task_id),
            vec![],
            vec![],
            serde_json::json!({}),
            causal_event_refs,
        ).await
    }

    pub async fn team_completed(&self, team_instance_id: Uuid, task_id: Uuid) -> anyhow::Result<TeamEvent> {
        self.emit(
            team_instance_id,
            TeamEventType::TeamCompleted,
            "supervisor",
            Some(task_id),
            vec![],
            vec![],
            serde_json::json!({}),
            vec![],
        ).await
    }

    pub async fn team_failed(&self, team_instance_id: Uuid, task_id: Uuid, reason: &str) -> anyhow::Result<TeamEvent> {
        self.emit(
            team_instance_id,
            TeamEventType::TeamFailed,
            "supervisor",
            Some(task_id),
            vec![],
            vec![],
            serde_json::json!({"reason": reason}),
            vec![],
        ).await
    }
}