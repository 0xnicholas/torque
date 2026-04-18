use crate::models::v1::agent_instance::{AgentInstance, AgentInstanceStatus};
use crate::models::v1::checkpoint::Checkpoint;
use crate::models::v1::event::Event;
use crate::repository::{
    AgentInstanceRepository, CheckpointRepositoryExt, EventRepositoryExt,
};
use std::sync::Arc;
use uuid::Uuid;

pub struct RecoveryService {
    agent_instance_repo: Arc<dyn AgentInstanceRepository>,
    checkpoint_repo: Arc<dyn CheckpointRepositoryExt>,
    event_repo: Arc<dyn EventRepositoryExt>,
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
        }
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
        let checkpoint = self.checkpoint_repo.get(checkpoint_id).await?
            .ok_or_else(|| anyhow::anyhow!("Checkpoint not found: {}", checkpoint_id))?;

        let instance_id = checkpoint.agent_instance_id;

        // 2. Get instance
        let instance = self.agent_instance_repo.get(instance_id).await?
            .ok_or_else(|| anyhow::anyhow!("Agent instance not found: {}", instance_id))?;

        // 3. Pre-validate recovery plan (fetch all data before mutating)
        let events = self.event_repo.list_by_types(
            "agent_instance",
            instance_id,
            &[],
            1000,
        ).await?;

        let checkpoint_time = checkpoint.created_at;
        let tail_events: Vec<&Event> = events.iter()
            .filter(|e| e.timestamp > checkpoint_time)
            .collect();

        // 4. Apply recovery
        // Step 4a: Restore status from checkpoint snapshot
        if let Some(status) = checkpoint.snapshot.get("status").and_then(|s| s.as_str()) {
            let restored_status = match status {
                "READY" => AgentInstanceStatus::Ready,
                "RUNNING" => AgentInstanceStatus::Running,
                "SUSPENDED" => AgentInstanceStatus::Suspended,
                _ => AgentInstanceStatus::Created,
            };
            self.agent_instance_repo.update_status(instance_id, restored_status).await?;
        }

        // Step 4b: Replay tail events to reconcile state
        for event in tail_events {
            match event.event_type.as_str() {
                "task.completed" => {
                    self.agent_instance_repo.update_current_task(instance_id, None).await?;
                    self.agent_instance_repo.update_status(instance_id, AgentInstanceStatus::Ready).await?;
                }
                "task.failed" => {
                    self.agent_instance_repo.update_current_task(instance_id, None).await?;
                    // Keep failed status
                }
                "instance.suspended" => {
                    self.agent_instance_repo.update_status(instance_id, AgentInstanceStatus::Suspended).await?;
                }
                _ => {}
            }
        }

        // 5. Fetch updated instance
        let instance = self.agent_instance_repo.get(instance_id).await?
            .ok_or_else(|| anyhow::anyhow!("Agent instance disappeared during recovery"))?;

        Ok(instance)
    }

    /// Resume agent instance from latest checkpoint.
    pub async fn resume_instance(
        &self,
        instance_id: Uuid,
    ) -> anyhow::Result<AgentInstance> {
        // Find latest checkpoint for instance
        let checkpoints = self.checkpoint_repo.list_by_instance(instance_id, 1).await?;
        
        if let Some(checkpoint) = checkpoints.into_iter().next() {
            self.restore_from_checkpoint(checkpoint.id).await
        } else {
            // No checkpoint found, just return current instance state
            let instance = self.agent_instance_repo.get(instance_id).await?
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
        let checkpoint = self.checkpoint_repo.get(checkpoint_id).await?
            .ok_or_else(|| anyhow::anyhow!("Checkpoint not found: {}", checkpoint_id))?;
        
        if checkpoint.agent_instance_id != instance_id {
            return Err(anyhow::anyhow!(
                "Checkpoint {} does not belong to instance {}",
                checkpoint_id, instance_id
            ));
        }

        // 2. Get original instance
        let original = self.agent_instance_repo.get(instance_id).await?
            .ok_or_else(|| anyhow::anyhow!("Agent instance not found: {}", instance_id))?;

        // 3. Create new instance from checkpoint (branch)
        let new_instance = self.agent_instance_repo.create(
            &crate::models::v1::agent_instance::AgentInstanceCreate {
                agent_definition_id: original.agent_definition_id,
                external_context_refs: vec![],
            }
        ).await?;

        // 4. Restore status from checkpoint
        if let Some(status) = checkpoint.snapshot.get("status").and_then(|s| s.as_str()) {
            let restored_status = match status {
                "READY" => AgentInstanceStatus::Ready,
                "RUNNING" => AgentInstanceStatus::Running,
                "SUSPENDED" => AgentInstanceStatus::Suspended,
                _ => AgentInstanceStatus::Created,
            };
            self.agent_instance_repo.update_status(new_instance.id, restored_status).await?;
        }

        // 5. Replay events after checkpoint on the new instance
        let events = self.event_repo.list_by_types(
            "agent_instance",
            instance_id,
            &[],
            1000,
        ).await?;

        let checkpoint_time = checkpoint.created_at;
        let tail_events: Vec<&Event> = events.iter()
            .filter(|e| e.timestamp > checkpoint_time)
            .collect();

        for event in tail_events {
            match event.event_type.as_str() {
                "task.completed" => {
                    self.agent_instance_repo.update_current_task(new_instance.id, None).await?;
                    self.agent_instance_repo.update_status(new_instance.id, AgentInstanceStatus::Ready).await?;
                }
                "task.failed" => {
                    self.agent_instance_repo.update_current_task(new_instance.id, None).await?;
                }
                "instance.suspended" => {
                    self.agent_instance_repo.update_status(new_instance.id, AgentInstanceStatus::Suspended).await?;
                }
                _ => {}
            }
        }

        // TODO: Record branch metadata (parent instance, checkpoint, branch name)
        // This would require a branch_history table

        // 6. Return the branched instance
        let branched = self.agent_instance_repo.get(new_instance.id).await?
            .ok_or_else(|| anyhow::anyhow!("Branched instance disappeared"))?;

        Ok(branched)
    }
}
