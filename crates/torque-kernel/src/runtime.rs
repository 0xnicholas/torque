//! Kernel-owned runtime contracts plus an in-memory reference implementation.
//!
//! Defines the [`KernelRuntime`] and [`RuntimeStore`] traits that form the
//! execution API boundary. Production runtime environments compose these
//! contracts above the kernel (see `torque-runtime`).
//!
//! [`InMemoryKernelRuntime`] is a reference implementation useful for tests
//! and local execution. It is not the full production runtime environment.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::recovery::{
    Checkpoint, CheckpointId, CheckpointStateView, RecoveryAction, RecoveryAssessment,
    RecoveryDisposition, RecoveryView,
};
use crate::{
    AgentDefinition, AgentDefinitionId, AgentInstance, AgentInstanceId, AgentInstanceState,
    ExecutionEngine, ExecutionRequest, ExecutionResult, KernelError, StepDecision, Task, TaskId,
    TaskInputRef, TaskPacket,
};
use chrono::Utc;

/// Primary kernel execution interface (commands only).
///
/// Processes execution requests and step decisions, returning execution
/// results with state transitions and events.
///
/// For state queries (instance, task, execution history, checkpoints),
/// use [`RuntimeStore`]. This split follows Command-Query Separation:
/// the kernel owns execution commands; the store provides read access to
/// persisted state. Consumers that need both should hold
/// `&mut dyn KernelRuntime` and `&dyn RuntimeStore`.
pub trait KernelRuntime {
    fn handle(
        &mut self,
        request: ExecutionRequest,
        decision: StepDecision,
    ) -> Result<ExecutionResult, KernelError>;

    fn handle_command(
        &mut self,
        request: ExecutionRequest,
        command: RuntimeCommand,
    ) -> Result<ExecutionResult, KernelError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResumeSignal {
    ApprovalGranted {
        approval_request_id: crate::ApprovalRequestId,
    },
    ToolCompleted,
    DelegationCompleted {
        delegation_request_id: crate::DelegationRequestId,
    },
    ManualResume,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCommand {
    pub decision: StepDecision,
    pub resume_signal: Option<ResumeSignal>,
}

impl RuntimeCommand {
    pub fn new(decision: StepDecision) -> Self {
        Self {
            decision,
            resume_signal: None,
        }
    }

    pub fn with_resume_signal(mut self, resume_signal: ResumeSignal) -> Self {
        self.resume_signal = Some(resume_signal);
        self
    }
}

/// Persistence contract for kernel runtime state (queries only).
///
/// Provides read access to agent definitions, instances, tasks, execution
/// results, and checkpoints.
///
/// For execution commands, use [`KernelRuntime`]. See its docs for the
/// Command-Query Separation design rationale.
pub trait RuntimeStore {
    fn agent_definition(&self, agent_definition_id: AgentDefinitionId) -> Option<&AgentDefinition>;
    fn put_agent_definition(&mut self, agent_definition: AgentDefinition);
    fn instance(&self, instance_id: AgentInstanceId) -> Option<&AgentInstance>;
    fn remove_instance(&mut self, instance_id: AgentInstanceId) -> Option<AgentInstance>;
    fn put_instance(&mut self, instance: AgentInstance);
    fn task(&self, task_id: TaskId) -> Option<&Task>;
    fn remove_task(&mut self, task_id: TaskId) -> Option<Task>;
    fn put_task(&mut self, task: Task);
    fn append_execution_result(&mut self, result: ExecutionResult);
    fn execution_history(&self, instance_id: AgentInstanceId) -> &[ExecutionResult];
    fn latest_execution_result(&self, instance_id: AgentInstanceId) -> Option<&ExecutionResult>;
    fn append_checkpoint(&mut self, checkpoint: Checkpoint);
    fn checkpoint(
        &self,
        instance_id: AgentInstanceId,
        checkpoint_id: CheckpointId,
    ) -> Option<&Checkpoint>;
    fn checkpoint_history(&self, instance_id: AgentInstanceId) -> &[Checkpoint];
    fn latest_checkpoint(&self, instance_id: AgentInstanceId) -> Option<&Checkpoint>;
}

/// In-memory reference store used by the kernel's local runtime implementation.
#[derive(Debug, Default)]
pub struct InMemoryRuntimeStore {
    agent_definitions: HashMap<AgentDefinitionId, AgentDefinition>,
    instances: HashMap<AgentInstanceId, AgentInstance>,
    tasks: HashMap<TaskId, Task>,
    execution_history: HashMap<AgentInstanceId, Vec<ExecutionResult>>,
    checkpoints: HashMap<AgentInstanceId, Vec<Checkpoint>>,
}

impl InMemoryRuntimeStore {
    pub fn new(agent_definitions: impl IntoIterator<Item = AgentDefinition>) -> Self {
        let mut runtime = Self::default();
        for agent_definition in agent_definitions {
            runtime.put_agent_definition(agent_definition);
        }
        runtime
    }

    pub fn agent_definition(
        &self,
        agent_definition_id: AgentDefinitionId,
    ) -> Option<&AgentDefinition> {
        self.agent_definitions.get(&agent_definition_id)
    }

    pub fn instance(&self, instance_id: AgentInstanceId) -> Option<&AgentInstance> {
        self.instances.get(&instance_id)
    }

    pub fn task(&self, task_id: TaskId) -> Option<&Task> {
        self.tasks.get(&task_id)
    }

    pub fn execution_history(&self, instance_id: AgentInstanceId) -> &[ExecutionResult] {
        self.execution_history
            .get(&instance_id)
            .map(|history| history.as_slice())
            .unwrap_or(&[])
    }

    pub fn latest_execution_result(
        &self,
        instance_id: AgentInstanceId,
    ) -> Option<&ExecutionResult> {
        self.execution_history
            .get(&instance_id)
            .and_then(|history| history.last())
    }

    pub fn checkpoint(
        &self,
        instance_id: AgentInstanceId,
        checkpoint_id: CheckpointId,
    ) -> Option<&Checkpoint> {
        self.checkpoints
            .get(&instance_id)
            .and_then(|checkpoints| checkpoints.iter().find(|cp| cp.id == checkpoint_id))
    }

    pub fn checkpoint_history(&self, instance_id: AgentInstanceId) -> &[Checkpoint] {
        self.checkpoints
            .get(&instance_id)
            .map(|checkpoints| checkpoints.as_slice())
            .unwrap_or(&[])
    }

    pub fn latest_checkpoint(&self, instance_id: AgentInstanceId) -> Option<&Checkpoint> {
        self.checkpoints
            .get(&instance_id)
            .and_then(|checkpoints| checkpoints.last())
    }
}

impl RuntimeStore for InMemoryRuntimeStore {
    fn agent_definition(&self, agent_definition_id: AgentDefinitionId) -> Option<&AgentDefinition> {
        self.agent_definitions.get(&agent_definition_id)
    }

    fn put_agent_definition(&mut self, agent_definition: AgentDefinition) {
        self.agent_definitions
            .insert(agent_definition.id, agent_definition);
    }

    fn instance(&self, instance_id: AgentInstanceId) -> Option<&AgentInstance> {
        self.instances.get(&instance_id)
    }

    fn remove_instance(&mut self, instance_id: AgentInstanceId) -> Option<AgentInstance> {
        self.instances.remove(&instance_id)
    }

    fn put_instance(&mut self, instance: AgentInstance) {
        self.instances.insert(instance.id(), instance);
    }

    fn task(&self, task_id: TaskId) -> Option<&Task> {
        self.tasks.get(&task_id)
    }

    fn remove_task(&mut self, task_id: TaskId) -> Option<Task> {
        self.tasks.remove(&task_id)
    }

    fn put_task(&mut self, task: Task) {
        self.tasks.insert(task.id(), task);
    }

    fn append_execution_result(&mut self, result: ExecutionResult) {
        self.execution_history
            .entry(result.instance_id)
            .or_default()
            .push(result);
    }

    fn execution_history(&self, instance_id: AgentInstanceId) -> &[ExecutionResult] {
        self.execution_history
            .get(&instance_id)
            .map(|history| history.as_slice())
            .unwrap_or(&[])
    }

    fn latest_execution_result(&self, instance_id: AgentInstanceId) -> Option<&ExecutionResult> {
        self.execution_history
            .get(&instance_id)
            .and_then(|history| history.last())
    }

    fn append_checkpoint(&mut self, checkpoint: Checkpoint) {
        self.checkpoints
            .entry(checkpoint.instance_id)
            .or_default()
            .push(checkpoint);
    }

    fn checkpoint(
        &self,
        instance_id: AgentInstanceId,
        checkpoint_id: CheckpointId,
    ) -> Option<&Checkpoint> {
        self.checkpoints
            .get(&instance_id)
            .and_then(|checkpoints| checkpoints.iter().find(|cp| cp.id == checkpoint_id))
    }

    fn checkpoint_history(&self, instance_id: AgentInstanceId) -> &[Checkpoint] {
        self.checkpoints
            .get(&instance_id)
            .map(|checkpoints| checkpoints.as_slice())
            .unwrap_or(&[])
    }

    fn latest_checkpoint(&self, instance_id: AgentInstanceId) -> Option<&Checkpoint> {
        self.checkpoints
            .get(&instance_id)
            .and_then(|checkpoints| checkpoints.last())
    }
}

/// Reference implementation of the kernel runtime contract.
///
/// This type is intentionally useful for tests and local execution. It is not
/// the full production runtime environment for Torque deployments.
#[derive(Debug, Default)]
pub struct InMemoryKernelRuntime {
    engine: ExecutionEngine,
    store: InMemoryRuntimeStore,
}

impl InMemoryKernelRuntime {
    pub fn new(agent_definitions: impl IntoIterator<Item = AgentDefinition>) -> Self {
        Self {
            engine: ExecutionEngine,
            store: InMemoryRuntimeStore::new(agent_definitions),
        }
    }

    pub fn with_store(store: InMemoryRuntimeStore) -> Self {
        Self {
            engine: ExecutionEngine,
            store,
        }
    }

    pub fn store(&self) -> &InMemoryRuntimeStore {
        &self.store
    }

    pub fn instance(&self, instance_id: AgentInstanceId) -> Option<&AgentInstance> {
        self.store.instance(instance_id)
    }

    pub fn task(&self, task_id: TaskId) -> Option<&Task> {
        self.store.task(task_id)
    }

    pub fn execution_history(&self, instance_id: AgentInstanceId) -> &[ExecutionResult] {
        self.store.execution_history(instance_id)
    }

    pub fn latest_execution_result(
        &self,
        instance_id: AgentInstanceId,
    ) -> Option<&ExecutionResult> {
        self.store.latest_execution_result(instance_id)
    }

    pub fn checkpoint_history(&self, instance_id: AgentInstanceId) -> &[Checkpoint] {
        self.store.checkpoint_history(instance_id)
    }

    pub fn latest_checkpoint(&self, instance_id: AgentInstanceId) -> Option<&Checkpoint> {
        self.store.latest_checkpoint(instance_id)
    }

    pub fn checkpoint(
        &self,
        instance_id: AgentInstanceId,
        checkpoint_id: CheckpointId,
    ) -> Option<&Checkpoint> {
        self.store.checkpoint(instance_id, checkpoint_id)
    }

    pub fn create_checkpoint(
        &mut self,
        instance_id: AgentInstanceId,
    ) -> Result<Checkpoint, KernelError> {
        let state_view = self.checkpoint_state_view(instance_id)?;
        let instance = self.store.instance(instance_id).ok_or_else(|| {
            crate::ValidationError::new(
                "Checkpoint",
                format!("unknown instance: {}", instance_id.as_uuid()),
            )
        })?;

        let checkpoint = Checkpoint {
            id: CheckpointId::new(),
            instance_id,
            active_task_id: state_view.active_task_id,
            active_task_state: state_view.active_task_state,
            instance_state: instance.state(),
            pending_approval_ids: state_view.pending_approval_ids,
            child_delegation_ids: state_view.child_delegation_ids,
            event_sequence: state_view.event_sequence,
            created_at: Utc::now(),
        };

        self.store.append_checkpoint(checkpoint.clone());
        Ok(checkpoint)
    }

    pub fn checkpoint_state_view(
        &self,
        instance_id: AgentInstanceId,
    ) -> Result<CheckpointStateView, KernelError> {
        let instance = self.store.instance(instance_id).ok_or_else(|| {
            crate::ValidationError::new(
                "CheckpointStateView",
                format!("unknown instance: {}", instance_id.as_uuid()),
            )
        })?;

        Ok(CheckpointStateView {
            instance_id,
            active_task_id: instance.active_task_id(),
            active_task_state: instance
                .active_task_id()
                .and_then(|task_id| self.store.task(task_id).map(|task| task.state())),
            instance_state: instance.state(),
            pending_approval_ids: instance.pending_approval_ids().to_vec(),
            child_delegation_ids: instance.child_delegation_ids().to_vec(),
            event_sequence: self
                .latest_execution_result(instance_id)
                .map(|result| result.sequence_number)
                .unwrap_or(0),
            latest_outcome: self
                .latest_execution_result(instance_id)
                .map(|result| result.outcome),
        })
    }

    pub fn recovery_view(
        &self,
        instance_id: AgentInstanceId,
        checkpoint_id: CheckpointId,
    ) -> Result<RecoveryView, KernelError> {
        let checkpoint = self
            .store
            .checkpoint(instance_id, checkpoint_id)
            .cloned()
            .ok_or_else(|| {
                crate::ValidationError::new(
                    "RecoveryView",
                    format!(
                        "unknown checkpoint {} for instance {}",
                        checkpoint_id.as_uuid(),
                        instance_id.as_uuid()
                    ),
                )
            })?;

        let tail_events = self
            .store
            .execution_history(instance_id)
            .iter()
            .filter(|result| result.sequence_number > checkpoint.event_sequence)
            .cloned()
            .collect();

        Ok(RecoveryView {
            checkpoint,
            tail_events,
        })
    }

    pub fn assess_recovery(
        &self,
        instance_id: AgentInstanceId,
        checkpoint_id: CheckpointId,
    ) -> Result<RecoveryAssessment, KernelError> {
        let view = self.recovery_view(instance_id, checkpoint_id)?;
        let instance = self.store.instance(instance_id).ok_or_else(|| {
            crate::ValidationError::new(
                "RecoveryAssessment",
                format!("unknown instance: {}", instance_id.as_uuid()),
            )
        })?;

        let latest_outcome = self
            .store
            .latest_execution_result(instance_id)
            .map(|result| result.outcome);

        let disposition = match instance.state() {
            AgentInstanceState::AwaitingApproval => RecoveryDisposition::AwaitingApproval,
            AgentInstanceState::AwaitingTool => RecoveryDisposition::AwaitingTool,
            AgentInstanceState::AwaitingDelegation => RecoveryDisposition::AwaitingDelegation,
            AgentInstanceState::Suspended => RecoveryDisposition::Suspended,
            _ if matches!(latest_outcome, Some(crate::ExecutionOutcome::FailedTask)) => {
                RecoveryDisposition::Failed
            }
            AgentInstanceState::Ready if instance.active_task_id().is_none() => {
                RecoveryDisposition::Completed
            }
            _ => RecoveryDisposition::ResumeCurrent,
        };

        let requires_replay = !view.tail_events.is_empty();
        let recommended_action = if requires_replay {
            RecoveryAction::ReplayTailEvents
        } else {
            match disposition {
                RecoveryDisposition::ResumeCurrent => RecoveryAction::ResumeExecution,
                RecoveryDisposition::AwaitingApproval => RecoveryAction::AwaitApprovalDecision,
                RecoveryDisposition::AwaitingTool => RecoveryAction::AwaitToolCompletion,
                RecoveryDisposition::AwaitingDelegation => {
                    RecoveryAction::AwaitDelegationCompletion
                }
                RecoveryDisposition::Suspended => RecoveryAction::StaySuspended,
                RecoveryDisposition::Completed => RecoveryAction::AcceptCompletedState,
                RecoveryDisposition::Failed => RecoveryAction::EscalateFailure,
            }
        };

        Ok(RecoveryAssessment {
            view,
            disposition,
            requires_replay,
            latest_outcome,
            recommended_action,
        })
    }

    pub fn recover_latest(
        &self,
        instance_id: AgentInstanceId,
    ) -> Result<RecoveryAssessment, KernelError> {
        let checkpoint_id = self
            .latest_checkpoint(instance_id)
            .ok_or_else(|| {
                crate::ValidationError::new(
                    "RecoveryAssessment",
                    format!("no checkpoint found for instance {}", instance_id.as_uuid()),
                )
            })?
            .id;

        self.assess_recovery(instance_id, checkpoint_id)
    }
}

impl KernelRuntime for InMemoryKernelRuntime {
    fn handle(
        &mut self,
        request: ExecutionRequest,
        decision: StepDecision,
    ) -> Result<ExecutionResult, KernelError> {
        self.handle_command(request, RuntimeCommand::new(decision))
    }

    fn handle_command(
        &mut self,
        request: ExecutionRequest,
        command: RuntimeCommand,
    ) -> Result<ExecutionResult, KernelError> {
        let agent_definition_id = request.agent_definition_id();
        let _agent_definition = self
            .store
            .agent_definition(agent_definition_id)
            .ok_or_else(|| {
                crate::ValidationError::new(
                    "ExecutionRequest",
                    format!(
                        "unknown agent definition: {}",
                        agent_definition_id.as_uuid()
                    ),
                )
            })?;

        let instance_id = if let Some(existing_instance_id) = request.instance_id() {
            existing_instance_id
        } else {
            let mut instance = AgentInstance::new(agent_definition_id);
            instance.begin_hydrating()?;
            instance.mark_ready()?;
            let new_instance_id = instance.id();
            self.store.put_instance(instance);
            new_instance_id
        };

        let task_id = if let Some(task_id) = self
            .store
            .instance(instance_id)
            .and_then(|instance| instance.active_task_id())
        {
            task_id
        } else {
            let mut task = Task::new(
                request.goal().to_string(),
                request.instructions().to_vec(),
                vec![],
            )
            .with_input_ref_iter(
                request
                    .input_artifact_ids()
                    .iter()
                    .copied()
                    .map(TaskInputRef::Artifact),
            )
            .with_input_ref_iter(request.external_context_refs().iter().map(|context_ref| {
                TaskInputRef::ExternalContext {
                    context_ref_id: context_ref.id,
                }
            }));
            task.validate()?;
            task.start()?;
            let new_task_id = task.id();

            {
                let mut instance = self.store.remove_instance(instance_id).ok_or_else(|| {
                    crate::ValidationError::new(
                        "ExecutionRequest",
                        format!("unknown instance: {}", instance_id.as_uuid()),
                    )
                })?;
                instance.bind_active_task(new_task_id)?;
                self.store.put_instance(instance);
            }

            self.store.put_task(task);
            new_task_id
        };

        let mut instance = self.store.remove_instance(instance_id).ok_or_else(|| {
            crate::ValidationError::new(
                "ExecutionRequest",
                format!("unknown instance: {}", instance_id.as_uuid()),
            )
        })?;

        let mut task = self.store.remove_task(task_id).ok_or_else(|| {
            crate::ValidationError::new(
                "ExecutionRequest",
                format!("unknown task: {}", task_id.as_uuid()),
            )
        })?;

        // Deferred restore: always put instance and task back, even if
        // the operation fails. The store is in-memory and put is infallible,
        // so partial state is at least retained for retry.
        // A transactional store would wrap this in a rollback-on-error.
        let result = (|| -> Result<(ExecutionResult, Option<ResumeSignal>), KernelError> {
            let mut applied_resume_signal = None;

            match instance.state() {
                AgentInstanceState::Ready => instance.begin_running()?,
                AgentInstanceState::AwaitingApproval => match command.resume_signal {
                    Some(ResumeSignal::ApprovalGranted {
                        approval_request_id,
                    }) => {
                        instance.resolve_approval(approval_request_id)?;
                        instance.resume_running()?;
                        applied_resume_signal = Some(ResumeSignal::ApprovalGranted {
                            approval_request_id,
                        });
                    }
                    None => {
                        return Err(crate::ValidationError::new(
                            "ExecutionRequest",
                            "missing approval resume signal",
                        )
                        .into());
                    }
                    _ => {
                        return Err(crate::ValidationError::new(
                            "ExecutionRequest",
                            "invalid resume signal for waiting approval state",
                        )
                        .into());
                    }
                },
                AgentInstanceState::AwaitingTool => match command.resume_signal {
                    Some(ResumeSignal::ToolCompleted) => {
                        instance.resume_running()?;
                        applied_resume_signal = Some(ResumeSignal::ToolCompleted);
                    }
                    None => {
                        return Err(crate::ValidationError::new(
                            "ExecutionRequest",
                            "missing tool resume signal",
                        )
                        .into());
                    }
                    _ => {
                        return Err(crate::ValidationError::new(
                            "ExecutionRequest",
                            "invalid resume signal for waiting tool state",
                        )
                        .into());
                    }
                },
                AgentInstanceState::AwaitingDelegation => match command.resume_signal {
                    Some(ResumeSignal::DelegationCompleted {
                        delegation_request_id,
                    }) => {
                        instance.resolve_delegation(delegation_request_id)?;
                        instance.resume_running()?;
                        applied_resume_signal = Some(ResumeSignal::DelegationCompleted {
                            delegation_request_id,
                        });
                    }
                    None => {
                        return Err(crate::ValidationError::new(
                            "ExecutionRequest",
                            "missing delegation resume signal",
                        )
                        .into());
                    }
                    _ => {
                        return Err(crate::ValidationError::new(
                            "ExecutionRequest",
                            "invalid resume signal for waiting delegation state",
                        )
                        .into());
                    }
                },
                AgentInstanceState::Suspended => match command.resume_signal {
                    Some(ResumeSignal::ManualResume) => {
                        instance.resume_running()?;
                        applied_resume_signal = Some(ResumeSignal::ManualResume);
                    }
                    None => {
                        return Err(crate::ValidationError::new(
                            "ExecutionRequest",
                            "missing manual resume signal",
                        )
                        .into());
                    }
                    _ => {
                        return Err(crate::ValidationError::new(
                            "ExecutionRequest",
                            "invalid resume signal for suspended state",
                        )
                        .into());
                    }
                },
                AgentInstanceState::Running => {}
                other => {
                    return Err(crate::ValidationError::new(
                        "ExecutionRequest",
                        format!("instance is not resumable from state {other:?}"),
                    )
                    .into());
                }
            }

            let packet = TaskPacket::from_request_and_task(&request, &task);

            let result = self
                .engine
                .step(&mut instance, &mut task, &packet, command.decision)?;

            Ok((result, applied_resume_signal))
        })();

        // Always restore instance and task to the store
        self.store.put_instance(instance);
        self.store.put_task(task);

        let (mut result, applied_resume_signal) = result?;
        result.sequence_number = self.store.execution_history(result.instance_id).len() as u64 + 1;
        if let Some(resume_signal) = applied_resume_signal {
            result
                .events
                .push(crate::ExecutionEvent::ResumeApplied { resume_signal });
        }
        self.store.append_execution_result(result.clone());

        Ok(result)
    }
}
