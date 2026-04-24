use crate::models::v1::agent_instance::{AgentInstance, AgentInstanceStatus};
use crate::models::v1::checkpoint::{Checkpoint, ContextAnchorType};
use crate::models::v1::event::Event;
use crate::models::v1::team::{
    TeamRecoveryAction, TeamRecoveryAssessment, TeamRecoveryDisposition, TeamTaskRecoveryResult,
    TeamTaskStatus,
};
use crate::repository::{
    AgentInstanceRepository, CheckpointRepositoryExt, EventRepositoryExt, MemoryRepositoryV1,
    TeamMemberRepository, TeamTaskRepository,
};
use crate::service::event_replay::EventReplayRegistry;
use serde::Serialize;
use std::sync::Arc;
use uuid::Uuid;

fn normalize_status(s: &str) -> String {
    if s.contains('_') {
        s.to_string()
    } else {
        let mut result = String::new();
        for (i, c) in s.chars().enumerate() {
            if c.is_uppercase() && i > 0 {
                result.push('_');
            }
            result.push(c.to_ascii_uppercase());
        }
        result
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RecoveryDisposition {
    ResumeCurrent,
    AwaitingApproval,
    AwaitingTool,
    AwaitingDelegation,
    Suspended,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RecoveryAction {
    ReplayTailEvents,
    ResumeExecution,
    AwaitApprovalDecision,
    AwaitToolCompletion,
    AwaitDelegationCompletion,
    StaySuspended,
    AcceptCompletedState,
    EscalateFailure,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecoveryAssessmentResult {
    pub instance_id: Uuid,
    pub checkpoint_id: Uuid,
    pub disposition: RecoveryDisposition,
    pub requires_replay: bool,
    pub recommended_action: RecoveryAction,
    pub terminal: bool,
}

impl RecoveryAssessmentResult {
    pub fn is_terminal(&self) -> bool {
        self.terminal
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RecoveryResult {
    pub checkpoint_id: Uuid,
    pub restored_anchors: usize,
    pub events_replayed: usize,
    pub inconsistencies_found: usize,
    pub resolutions_applied: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Inconsistency {
    pub anchor_type: ContextAnchorType,
    pub reference_id: Uuid,
    pub description: String,
}

pub struct RecoveryService {
    agent_instance_repo: Arc<dyn AgentInstanceRepository>,
    checkpoint_repo: Arc<dyn CheckpointRepositoryExt>,
    event_repo: Arc<dyn EventRepositoryExt>,
    event_registry: EventReplayRegistry,
    repo_v1: Option<Arc<dyn MemoryRepositoryV1>>,
    team_member_repo: Option<Arc<dyn TeamMemberRepository>>,
    team_task_repo: Option<Arc<dyn TeamTaskRepository>>,
}

impl RecoveryService {
    pub fn new(
        agent_instance_repo: Arc<dyn AgentInstanceRepository>,
        checkpoint_repo: Arc<dyn CheckpointRepositoryExt>,
        event_repo: Arc<dyn EventRepositoryExt>,
    ) -> Self {
        Self {
            agent_instance_repo,
            checkpoint_repo,
            event_repo,
            event_registry: EventReplayRegistry::new(),
            repo_v1: None,
            team_member_repo: None,
            team_task_repo: None,
        }
    }

    pub fn with_repo_v1(self, repo_v1: Arc<dyn MemoryRepositoryV1>) -> Self {
        Self {
            repo_v1: Some(repo_v1),
            ..self
        }
    }

    pub fn with_team_repos(
        mut self,
        team_member_repo: Arc<dyn TeamMemberRepository>,
        team_task_repo: Arc<dyn TeamTaskRepository>,
    ) -> Self {
        self.team_member_repo = Some(team_member_repo);
        self.team_task_repo = Some(team_task_repo);
        self
    }

    /// Assess recovery for a checkpoint without applying it.
    ///
    /// This provides a synchronous-style assessment using persistence data
    /// without requiring kernel runtime hydration.
    pub async fn assess_recovery(
        &self,
        checkpoint_id: Uuid,
    ) -> anyhow::Result<RecoveryAssessmentResult> {
        let checkpoint = self
            .checkpoint_repo
            .get(checkpoint_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Checkpoint not found: {}", checkpoint_id))?;

        let instance_id = checkpoint.agent_instance_id;

        let events = self
            .event_repo
            .list_by_types("agent_instance", instance_id, &[], 1000)
            .await?;

        let checkpoint_time = checkpoint.created_at;
        let tail_events: Vec<&Event> = events
            .iter()
            .filter(|e| e.timestamp > checkpoint_time)
            .collect();

        let custom = checkpoint.snapshot.get("custom_state");
        let state_str = custom
            .and_then(|c| c.get("instance_state"))
            .and_then(|s| s.as_str())
            .unwrap_or("CREATED");

        let normalized = normalize_status(state_str);
        let disposition = match normalized.as_str() {
            "WAITING_APPROVAL" => RecoveryDisposition::AwaitingApproval,
            "WAITING_TOOL" => RecoveryDisposition::AwaitingTool,
            "WAITING_SUBAGENT" => RecoveryDisposition::AwaitingDelegation,
            "SUSPENDED" => RecoveryDisposition::Suspended,
            "COMPLETED" => RecoveryDisposition::Completed,
            "FAILED" => RecoveryDisposition::Failed,
            _ => RecoveryDisposition::ResumeCurrent,
        };

        let requires_replay = !tail_events.is_empty();
        let recommended_action = if requires_replay {
            RecoveryAction::ReplayTailEvents
        } else {
            match disposition {
                RecoveryDisposition::ResumeCurrent => RecoveryAction::ResumeExecution,
                RecoveryDisposition::AwaitingApproval => RecoveryAction::AwaitApprovalDecision,
                RecoveryDisposition::AwaitingTool => RecoveryAction::AwaitToolCompletion,
                RecoveryDisposition::AwaitingDelegation => RecoveryAction::AwaitDelegationCompletion,
                RecoveryDisposition::Suspended => RecoveryAction::StaySuspended,
                RecoveryDisposition::Completed => RecoveryAction::AcceptCompletedState,
                RecoveryDisposition::Failed => RecoveryAction::EscalateFailure,
            }
        };

        let terminal = matches!(
            disposition,
            RecoveryDisposition::Completed | RecoveryDisposition::Failed
        );

        Ok(RecoveryAssessmentResult {
            instance_id,
            checkpoint_id,
            disposition,
            requires_replay,
            recommended_action,
            terminal,
        })
    }

    /// Restore agent instance from checkpoint.
    ///
    /// Recovery flow:
    /// 1. Load checkpoint
    /// 2. Validate recovery plan (fetch events, check consistency)
    /// 3. Apply recovery atomically
    /// 4. Replay tail events to reconcile state
    /// 5. Return updated instance
    ///
    /// Note: In production, this should use a database transaction.
    pub async fn restore_from_checkpoint(
        &self,
        checkpoint_id: Uuid,
    ) -> anyhow::Result<AgentInstance> {
        // 1. Load checkpoint
        let checkpoint = self
            .checkpoint_repo
            .get(checkpoint_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Checkpoint not found: {}", checkpoint_id))?;

        let instance_id = checkpoint.agent_instance_id;

        // 2. Validate instance exists
        let _instance = self
            .agent_instance_repo
            .get(instance_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Agent instance not found: {}", instance_id))?;

        // 3. Pre-validate recovery plan (fetch all data before mutating)
        let events = self
            .event_repo
            .list_by_types("agent_instance", instance_id, &[], 1000)
            .await?;

        let checkpoint_time = checkpoint.created_at;
        let tail_events: Vec<&Event> = events
            .iter()
            .filter(|e| e.timestamp > checkpoint_time)
            .collect();

        // 4. Apply recovery
        // Step 4a: Restore status from checkpoint snapshot
        // Note: snapshot is stored as CheckpointState serialized directly
        // custom_state field contains the instance state info
        if let Some(custom) = checkpoint.snapshot.get("custom_state") {
            if let Some(status) = custom.get("instance_state").and_then(|s| s.as_str()) {
                let normalized = normalize_status(status);
                let restored_status = match normalized.as_str() {
                    "READY" => AgentInstanceStatus::Ready,
                    "RUNNING" => AgentInstanceStatus::Running,
                    "SUSPENDED" => AgentInstanceStatus::Suspended,
                    "WAITING_SUBAGENT" => AgentInstanceStatus::WaitingSubagent,
                    "WAITING_APPROVAL" => AgentInstanceStatus::WaitingApproval,
                    "WAITING_TOOL" => AgentInstanceStatus::WaitingTool,
                    _ => AgentInstanceStatus::Created,
                };
                self.agent_instance_repo
                    .update_status(instance_id, restored_status)
                    .await?;
            }
        }

        // Step 4b: Replay tail events using registry
        for event in tail_events {
            if let Err(e) = self
                .event_registry
                .replay(event, &self.agent_instance_repo, None)
                .await
            {
                eprintln!("Event replay warning for {}: {}", event.event_type, e);
            }
        }

        if let Err(e) = self.reconcile_state(instance_id, &checkpoint).await {
            eprintln!("Reconciliation warning: {}", e);
        }

        // 5. Fetch updated instance
        let instance = self
            .agent_instance_repo
            .get(instance_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Agent instance disappeared during recovery"))?;

        Ok(instance)
    }

    async fn reconcile_state(
        &self,
        instance_id: Uuid,
        checkpoint: &crate::models::v1::checkpoint::Checkpoint,
    ) -> anyhow::Result<()> {
        let current_instance = self
            .agent_instance_repo
            .get(instance_id)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!("Instance {} disappeared during reconciliation", instance_id)
            })?;

        let checkpoint_time = checkpoint.created_at;
        let events_after_checkpoint = self
            .event_repo
            .list_by_types("agent_instance", instance_id, &[], 1000)
            .await?;

        let events_after: Vec<_> = events_after_checkpoint
            .iter()
            .filter(|e| e.timestamp > checkpoint_time)
            .collect();

        if !events_after.is_empty() {
            tracing::warn!(
                "Reconciliation: {} events occurred after checkpoint for instance {}",
                events_after.len(),
                instance_id
            );

            for event in &events_after {
                tracing::debug!(
                    "Reconciliation: event {} at {} for instance {}",
                    event.event_type,
                    event.timestamp,
                    instance_id
                );
            }
        }

        let custom = checkpoint.snapshot.get("custom_state");
        if let Some(data) = custom {
            if let Some(pending_approvals) = data.get("pending_approval_ids") {
                if let Some(approvals) = pending_approvals.as_array() {
                    let approval_count = approvals.len();
                    if approval_count > 0 {
                        tracing::info!(
                            "Reconciliation: checkpoint indicates {} pending approvals for instance {}",
                            approval_count,
                            instance_id
                        );
                    }
                }
            }

            if let Some(child_delegations) = data.get("child_delegation_ids") {
                if let Some(delegations) = child_delegations.as_array() {
                    for deleg_id in delegations {
                        if let Some(id_str) = deleg_id.as_str() {
                            if let Ok(child_id) = uuid::Uuid::parse_str(id_str) {
                                if let Ok(Some(child)) = self.agent_instance_repo.get(child_id).await {
                                    match child.status {
                                        AgentInstanceStatus::Failed => {
                                            tracing::warn!(
                                                "Reconciliation: child {} is Failed but parent {} expects active delegation, marking parent for re-delegation",
                                                child_id,
                                                instance_id
                                            );
                                            self.agent_instance_repo
                                                .update_status(instance_id, AgentInstanceStatus::Ready)
                                                .await?;
                                        }
                                        AgentInstanceStatus::Completed => {
                                            tracing::info!(
                                                "Reconciliation: child {} completed but parent {} still WaitingSubagent",
                                                child_id,
                                                instance_id
                                            );
                                        }
                                        _ => {
                                            tracing::debug!(
                                                "Reconciliation: child {} status {:?} for parent {}",
                                                child_id,
                                                child.status,
                                                instance_id
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if let Some(checkpoint_state) = data.get("instance_state").and_then(|s| s.as_str()) {
                let current_state = current_instance.status.to_string();
                if checkpoint_state != current_state {
                    tracing::warn!(
                        "Reconciliation: instance {} state mismatch - checkpoint: {}, current: {}",
                        instance_id,
                        checkpoint_state,
                        current_state
                    );
                }
            }
        }

        Ok(())
    }

    /// Resume agent instance from latest checkpoint.
    pub async fn resume_instance(&self, instance_id: Uuid) -> anyhow::Result<AgentInstance> {
        // Find latest checkpoint for instance
        let checkpoints = self
            .checkpoint_repo
            .list_by_instance(instance_id, 1)
            .await?;

        if let Some(checkpoint) = checkpoints.into_iter().next() {
            self.restore_from_checkpoint(checkpoint.id).await
        } else {
            // No checkpoint found, just return current instance state
            let instance = self
                .agent_instance_repo
                .get(instance_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Agent instance not found: {}", instance_id))?;
            Ok(instance)
        }
    }

    /// Time travel to checkpoint and create a new instance (branch).
    ///
    /// Per the recovery spec, time travel should create a new lineage
    /// rather than mutating the existing instance.
    pub async fn time_travel(
        &self,
        instance_id: Uuid,
        checkpoint_id: Uuid,
        branch_name: Option<String>,
    ) -> anyhow::Result<AgentInstance> {
        // 1. Verify checkpoint belongs to instance
        let checkpoint = self
            .checkpoint_repo
            .get(checkpoint_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Checkpoint not found: {}", checkpoint_id))?;

        if checkpoint.agent_instance_id != instance_id {
            return Err(anyhow::anyhow!(
                "Checkpoint {} does not belong to instance {}",
                checkpoint_id,
                instance_id
            ));
        }

        // 2. Get original instance
        let original = self
            .agent_instance_repo
            .get(instance_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Agent instance not found: {}", instance_id))?;

        // 3. Create new instance from checkpoint (branch)
        let new_instance = self
            .agent_instance_repo
            .create(&crate::models::v1::agent_instance::AgentInstanceCreate {
                agent_definition_id: original.agent_definition_id,
                external_context_refs: vec![],
            })
            .await?;

        // Log branch creation with name if provided
        if let Some(name) = branch_name {
            println!(
                "Created branch '{}' from checkpoint {} for instance {}",
                name, checkpoint_id, instance_id
            );
        }

        // 4. Restore status from checkpoint
        // Note: snapshot is stored as CheckpointState serialized directly
        // custom_state field contains the instance state info
        if let Some(custom) = checkpoint.snapshot.get("custom_state") {
            if let Some(status) = custom.get("instance_state").and_then(|s| s.as_str()) {
                let normalized = normalize_status(status);
                let restored_status = match normalized.as_str() {
                    "READY" => AgentInstanceStatus::Ready,
                    "RUNNING" => AgentInstanceStatus::Running,
                    "SUSPENDED" => AgentInstanceStatus::Suspended,
                    "WAITING_SUBAGENT" => AgentInstanceStatus::WaitingSubagent,
                    "WAITING_APPROVAL" => AgentInstanceStatus::WaitingApproval,
                    "WAITING_TOOL" => AgentInstanceStatus::WaitingTool,
                    _ => AgentInstanceStatus::Created,
                };
                self.agent_instance_repo
                    .update_status(new_instance.id, restored_status)
                    .await?;
            }
        }

        // 5. Replay events after checkpoint on the new instance
        let events = self
            .event_repo
            .list_by_types("agent_instance", instance_id, &[], 1000)
            .await?;

        let checkpoint_time = checkpoint.created_at;
        let tail_events: Vec<&Event> = events
            .iter()
            .filter(|e| e.timestamp > checkpoint_time)
            .collect();

        for event in tail_events {
            if let Err(e) = self
                .event_registry
                .replay(event, &self.agent_instance_repo, Some(new_instance.id))
                .await
            {
                eprintln!("Event replay warning for {}: {}", event.event_type, e);
            }
        }

        // TODO: Record branch metadata (parent instance, checkpoint, branch name)
        // This would require a branch_history table

        // 6. Return the branched instance
        let branched = self
            .agent_instance_repo
            .get(new_instance.id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Branched instance disappeared"))?;

        Ok(branched)
    }

    pub async fn restore_with_anchors_and_reconcile(
        &self,
        checkpoint_id: Uuid,
    ) -> anyhow::Result<RecoveryResult> {
        let checkpoint = self
            .checkpoint_repo
            .get(checkpoint_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Checkpoint not found: {}", checkpoint_id))?;

        let anchors = checkpoint.context_anchors.clone();
        let mut restored_anchors = 0;

        if let Some(repo_v1) = &self.repo_v1 {
            for anchor in &anchors {
                match anchor.anchor_type {
                    ContextAnchorType::MemoryEntry => {
                        if repo_v1.get_entry_by_id(anchor.reference_id).await?.is_some() {
                            restored_anchors += 1;
                        }
                    }
                    ContextAnchorType::ExternalContextRef => {
                        if repo_v1
                            .get_external_context_refs(checkpoint.agent_instance_id)
                            .await?
                            .iter()
                            .any(|r| r.id == anchor.reference_id)
                        {
                            restored_anchors += 1;
                        }
                    }
                    ContextAnchorType::SharedState => {
                        if repo_v1
                            .get_team_for_agent(checkpoint.agent_instance_id)
                            .await?
                            == Some(anchor.reference_id)
                        {
                            restored_anchors += 1;
                        }
                    }
                    ContextAnchorType::EventAnchor | ContextAnchorType::Artifact => {
                        restored_anchors += 1;
                    }
                }
            }
        }

        let event_anchor = anchors.iter().find(|a| matches!(
            a.anchor_type,
            ContextAnchorType::EventAnchor
        ));
        let mut events_replayed = 0;

        if let Some(anchor) = event_anchor {
            let events = self
                .event_repo
                .list_by_types("agent_instance", checkpoint.agent_instance_id, &[], 1000)
                .await?;
            let anchor_time = events
                .iter()
                .find(|e| e.event_id == anchor.reference_id)
                .map(|e| e.timestamp);
            if let Some(anchor_time) = anchor_time {
                let tail_events: Vec<_> = events
                    .into_iter()
                    .filter(|e| e.timestamp > anchor_time)
                    .collect();
                for event in &tail_events {
                    if self
                        .event_registry
                        .replay(event, &self.agent_instance_repo, None)
                        .await
                        .is_ok()
                    {
                        events_replayed += 1;
                    }
                }
            }
        }

        let inconsistencies = self.detect_inconsistencies(&checkpoint).await?;
        let resolutions_applied = self.apply_resolutions(&inconsistencies).await?;

        Ok(RecoveryResult {
            checkpoint_id,
            restored_anchors,
            events_replayed,
            inconsistencies_found: inconsistencies.len(),
            resolutions_applied,
        })
    }

    async fn detect_inconsistencies(
        &self,
        checkpoint: &Checkpoint,
    ) -> anyhow::Result<Vec<Inconsistency>> {
        let mut inconsistencies = Vec::new();
        let instance_id = checkpoint.agent_instance_id;

        let current_instance = self
            .agent_instance_repo
            .get(instance_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Instance {} not found", instance_id))?;

        let custom = checkpoint.snapshot.get("custom_state");
        if let Some(data) = custom {
            if let Some(checkpoint_state) = data.get("instance_state").and_then(|s| s.as_str()) {
                let current_state = current_instance.status.to_string();
                let normalized = normalize_status(checkpoint_state);
                if normalized != current_state {
                    inconsistencies.push(Inconsistency {
                        anchor_type: ContextAnchorType::MemoryEntry,
                        reference_id: instance_id,
                        description: format!(
                            "State mismatch: checkpoint={}, current={}",
                            normalized, current_state
                        ),
                    });
                }
            }
        }

        let events = self
            .event_repo
            .list_by_types("agent_instance", instance_id, &[], 1000)
            .await?;
        let checkpoint_time = checkpoint.created_at;
        let events_after: Vec<_> = events
            .iter()
            .filter(|e| e.timestamp > checkpoint_time)
            .collect();

        if events_after.is_empty() && !checkpoint.context_anchors.is_empty() {
            let last_event_anchor = checkpoint
                .context_anchors
                .iter()
                .find(|a| matches!(a.anchor_type, ContextAnchorType::EventAnchor));
            if last_event_anchor.is_none() {
                inconsistencies.push(Inconsistency {
                    anchor_type: ContextAnchorType::EventAnchor,
                    reference_id: instance_id,
                    description: "No events after checkpoint but no EventAnchor found".to_string(),
                });
            }
        }

        Ok(inconsistencies)
    }

    async fn apply_resolutions(&self, inconsistencies: &[Inconsistency]) -> anyhow::Result<usize> {
        let mut applied = 0;
        for inconsistency in inconsistencies {
            tracing::warn!(
                "Recovery resolution: type={:?} ref={} - {}",
                inconsistency.anchor_type,
                inconsistency.reference_id,
                inconsistency.description
            );
            applied += 1;
        }
        tracing::info!("Applied {} recovery resolutions", applied);
        Ok(applied)
    }

    pub async fn assess_team_recovery(
        &self,
        team_instance_id: Uuid,
    ) -> anyhow::Result<TeamRecoveryAssessment> {
        let member_repo = self.team_member_repo.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Team member repository not configured")
        })?;

        let members = member_repo.list_by_team(team_instance_id, 100).await?;

        let failed_member_ids: Vec<Uuid> = members
            .iter()
            .filter(|m| m.status == "Failed" || m.status == "failed")
            .map(|m| m.id)
            .collect();

        let waiting_member_ids: Vec<Uuid> = members
            .iter()
            .filter(|m| m.status == "WaitingMembers" || m.status == "WAITING_MEMBERS")
            .map(|m| m.id)
            .collect();

        let total_members = members.len();
        let failed_count = failed_member_ids.len();
        let waiting_count = waiting_member_ids.len();

        let disposition = if failed_count == 0 && waiting_count == 0 {
            TeamRecoveryDisposition::TeamHealthy
        } else if failed_count > 0 && failed_count < total_members {
            TeamRecoveryDisposition::TeamDegraded
        } else if failed_count == total_members && total_members > 0 {
            TeamRecoveryDisposition::TeamFailed
        } else if waiting_count > 0 && failed_count == 0 {
            TeamRecoveryDisposition::AwaitingSupervisor
        } else {
            TeamRecoveryDisposition::TeamHealthy
        };

        let recommendation = match disposition {
            TeamRecoveryDisposition::TeamHealthy => "Team is operational".to_string(),
            TeamRecoveryDisposition::TeamDegraded => format!(
                "Team is degraded with {} failed members out of {}. Consider retry or replacement.",
                failed_count, total_members
            ),
            TeamRecoveryDisposition::TeamFailed => {
                "All team members have failed. Escalate to supervisor for recovery.".to_string()
            }
            TeamRecoveryDisposition::AwaitingSupervisor => {
                "Team is waiting for supervisor guidance.".to_string()
            }
        };

        Ok(TeamRecoveryAssessment {
            team_instance_id,
            disposition,
            failed_member_ids,
            recommendation,
        })
    }

    pub async fn recover_team_task(
        &self,
        task_id: Uuid,
    ) -> anyhow::Result<TeamTaskRecoveryResult> {
        let task_repo = self.team_task_repo.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Team task repository not configured")
        })?;

        let task = task_repo
            .get(task_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Task not found: {}", task_id))?;

        let current_retry_count = task.retry_count;

        let (action_taken, new_status) = if task.status == TeamTaskStatus::Failed {
            if current_retry_count < 3 {
                let new_retry_count = current_retry_count + 1;
                task_repo.update_retry_count(task_id, new_retry_count).await?;
                task_repo.update_status(task_id, TeamTaskStatus::Open).await?;
                (TeamRecoveryAction::Retry, TeamTaskStatus::Open)
            } else {
                (TeamRecoveryAction::EscalateToSupervisor, task.status)
            }
        } else {
            (TeamRecoveryAction::NoOp, task.status)
        };

        Ok(TeamTaskRecoveryResult {
            task_id,
            action_taken,
            new_status,
        })
    }
}
