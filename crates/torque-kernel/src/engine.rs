use crate::{
    agent_instance::{AgentInstance, AgentInstanceState},
    execution::{ExecutionEvent, ExecutionOutcome, ExecutionResult},
    ids::{ApprovalRequestId, ArtifactId, DelegationRequestId},
    task::{Task, TaskState},
    task_packet::TaskPacket,
    KernelError, ValidationError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepDecision {
    Continue,
    AwaitTool,
    AwaitApproval(ApprovalRequestId),
    AwaitDelegation(DelegationRequestId),
    ProduceArtifacts(Vec<ArtifactId>),
    CompleteTask(String),
    FailTask(String),
    SuspendInstance,
}

#[derive(Debug, Default)]
pub struct ExecutionEngine;

impl ExecutionEngine {
    pub fn step(
        &self,
        instance: &mut AgentInstance,
        task: &mut Task,
        packet: &TaskPacket,
        decision: StepDecision,
    ) -> Result<ExecutionResult, KernelError> {
        validate_task_packet(packet, task)?;
        ensure_running(instance.state(), &decision)?;

        let previous_instance_state = instance.state();
        let previous_task_state = task.state();
        let mut artifact_ids = Vec::new();
        let mut approval_request_ids = Vec::new();
        let mut delegation_request_ids = Vec::new();
        let outcome;
        let mut summary = None;

        // =============================================================================
        // CHECKPOINT CREATION POINT
        // =============================================================================
        // TODO (Phase 2): When transitioning to WaitingTool/WaitingApproval/WaitingSubagent/Suspended,
        // trigger checkpoint creation via callback to persistence layer.
        //
        // Per Recovery Core Design Section 5.2 - "Recommended checkpoint contents should
        // focus on the minimum useful running state needed for efficient recovery"
        //
        // Full implementation requires:
        // 1. Defining CheckpointCallback trait in kernel
        // 2. Implementing callback in harness to call checkpointer.save()
        // 3. Wiring callback into state transition logic below
        //
        // Key state transitions that should trigger checkpoint:
        // - Running -> WaitingTool (line 49)
        // - Running -> WaitingApproval (line 53)
        // - Running -> WaitingSubagent (line 58)
        // - Running -> Suspended (line 84)
        // =============================================================================

        match decision {
            StepDecision::Continue => {
                outcome = ExecutionOutcome::Continue;
            }
            StepDecision::AwaitTool => {
                instance.wait_for_tool()?;
                outcome = ExecutionOutcome::AwaitTool;
            }
            StepDecision::AwaitApproval(approval_id) => {
                instance.wait_for_approval(approval_id)?;
                approval_request_ids.push(approval_id);
                outcome = ExecutionOutcome::AwaitApproval;
            }
            StepDecision::AwaitDelegation(delegation_id) => {
                instance.wait_for_delegation(delegation_id)?;
                delegation_request_ids.push(delegation_id);
                outcome = ExecutionOutcome::AwaitDelegation;
            }
            StepDecision::ProduceArtifacts(new_artifact_ids) => {
                for artifact_id in new_artifact_ids.iter().copied() {
                    task.record_artifact(artifact_id);
                    artifact_ids.push(artifact_id);
                }
                outcome = ExecutionOutcome::ProducedArtifacts;
            }
            StepDecision::CompleteTask(task_summary) => {
                task.complete(task_summary.clone())?;
                instance.mark_ready()?;
                instance.clear_active_task();
                summary = Some(task_summary);
                outcome = ExecutionOutcome::CompletedTask;
            }
            StepDecision::FailTask(reason) => {
                task.fail(reason.clone())?;
                instance.mark_ready()?;
                instance.clear_active_task();
                summary = Some(reason);
                outcome = ExecutionOutcome::FailedTask;
            }
            StepDecision::SuspendInstance => {
                instance.suspend()?;
                outcome = ExecutionOutcome::SuspendedInstance;
            }
        }

        let events = build_events(
            previous_instance_state,
            instance.state(),
            previous_task_state,
            task.state(),
            &artifact_ids,
            &approval_request_ids,
            &delegation_request_ids,
        );

        Ok(ExecutionResult {
            instance_id: instance.id(),
            task_id: task.id(),
            sequence_number: 0,
            outcome,
            instance_state: instance.state(),
            task_state: task.state(),
            artifact_ids,
            approval_request_ids,
            delegation_request_ids,
            events,
            summary,
        })
    }
}

fn validate_task_packet(packet: &TaskPacket, task: &Task) -> Result<(), KernelError> {
    if packet.goal.trim().is_empty() {
        return Err(ValidationError::new("TaskPacket", "goal must not be empty").into());
    }

    if packet.goal != task.goal() {
        return Err(
            ValidationError::new("TaskPacket", "goal must match the active task goal").into(),
        );
    }

    Ok(())
}

fn ensure_running(
    instance_state: AgentInstanceState,
    decision: &StepDecision,
) -> Result<(), KernelError> {
    if instance_state == AgentInstanceState::Running {
        return Ok(());
    }

    Err(ValidationError::new(
        "ExecutionEngine",
        format!("instance must be running before applying {decision:?}, got {instance_state:?}"),
    )
    .into())
}

fn build_events(
    previous_instance_state: AgentInstanceState,
    current_instance_state: AgentInstanceState,
    previous_task_state: TaskState,
    current_task_state: TaskState,
    artifact_ids: &[ArtifactId],
    approval_request_ids: &[ApprovalRequestId],
    delegation_request_ids: &[DelegationRequestId],
) -> Vec<ExecutionEvent> {
    let mut events = Vec::new();

    if previous_instance_state != current_instance_state {
        events.push(ExecutionEvent::InstanceStateChanged {
            from: previous_instance_state,
            to: current_instance_state,
        });
    }

    if previous_task_state != current_task_state {
        events.push(ExecutionEvent::TaskStateChanged {
            from: previous_task_state,
            to: current_task_state,
        });
    }

    for approval_request_id in approval_request_ids {
        events.push(ExecutionEvent::ApprovalRequested {
            approval_request_id: *approval_request_id,
        });
    }

    for delegation_request_id in delegation_request_ids {
        events.push(ExecutionEvent::DelegationRequested {
            delegation_request_id: *delegation_request_id,
        });
    }

    for artifact_id in artifact_ids {
        events.push(ExecutionEvent::ArtifactProduced {
            artifact_id: *artifact_id,
        });
    }

    events
}
