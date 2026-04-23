use crate::models::v1::agent_instance::{AgentInstance, AgentInstanceStatus};
use crate::models::v1::event::Event;
use crate::repository::{AgentInstanceRepository, CheckpointRepositoryExt, EventRepositoryExt};
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

pub struct RecoveryService {
    agent_instance_repo: Arc<dyn AgentInstanceRepository>,
    checkpoint_repo: Arc<dyn CheckpointRepositoryExt>,
    event_repo: Arc<dyn EventRepositoryExt>,
    event_registry: EventReplayRegistry,
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
        }
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

        // 2. Get instance
        let instance = self
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
            .ok_or_else(|| anyhow::anyhow!("Instance {} disappeared during reconciliation", instance_id))?;

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
}
