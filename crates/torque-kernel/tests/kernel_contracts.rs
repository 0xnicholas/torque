use torque_kernel::{
    AccessMode, AgentDefinition, AgentDefinitionId, AgentInstance, AgentInstanceState,
    ApprovalRequestId, ExecutionEngine, ExecutionEvent, ExecutionMode, ExecutionOutcome,
    ExecutionRequest, ExternalContextKind, ExternalContextRef, InMemoryKernelRuntime,
    InMemoryRuntimeStore, KernelRuntime, RecoveryAction, RecoveryDisposition, ResumeSignal,
    RuntimeCommand, RuntimeStore, StepDecision, SyncPolicy, Task, TaskConstraint, TaskInputRef,
    TaskPacket, TaskState,
};

#[test]
fn kernel_exports_reference_runtime_surface_without_implying_production_runtime() {
    let _ = std::any::type_name::<dyn KernelRuntime>();
    let _ = std::any::type_name::<dyn RuntimeStore>();
    let _ = std::any::type_name::<InMemoryKernelRuntime>();
    let _ = std::any::type_name::<InMemoryRuntimeStore>();
}

#[test]
fn task_transitions_from_open_to_in_progress_to_done() {
    let mut task = Task::new(
        "Build kernel".into(),
        vec!["Set up runtime contracts".into()],
        vec![TaskConstraint::new("keep boundaries explicit")],
    );

    assert_eq!(task.state(), TaskState::Open);

    task.start().expect("task should start");
    assert_eq!(task.state(), TaskState::InProgress);

    task.complete("core contracts defined")
        .expect("task should complete");
    assert_eq!(task.state(), TaskState::Done);
}

#[test]
fn agent_instance_binds_a_single_active_task() {
    let definition_id = AgentDefinitionId::new();
    let mut instance = AgentInstance::new(definition_id);
    let task = Task::new("Build kernel".into(), vec![], vec![]);

    assert_eq!(instance.state(), AgentInstanceState::Created);
    assert!(instance.active_task_id().is_none());

    instance.begin_hydrating().expect("instance should hydrate");
    instance.mark_ready().expect("instance should become ready");
    instance
        .bind_active_task(task.id())
        .expect("task should bind");
    instance.begin_running().expect("instance should start");

    assert_eq!(instance.state(), AgentInstanceState::Running);
    assert_eq!(instance.active_task_id(), Some(task.id()));
}

#[test]
fn execution_request_captures_current_task_intent() {
    let request = ExecutionRequest::new(
        AgentDefinitionId::new(),
        "Scaffold torque-kernel",
        vec!["create ids/task/instance/execution modules".into()],
    )
    .with_execution_mode(ExecutionMode::Sync);

    assert_eq!(request.goal(), "Scaffold torque-kernel");
    assert_eq!(request.instructions().len(), 1);
    assert_eq!(request.execution_mode(), ExecutionMode::Sync);
}

#[test]
fn execution_outcome_marks_terminal_task_result() {
    assert!(ExecutionOutcome::CompletedTask.is_terminal());
    assert!(ExecutionOutcome::FailedTask.is_terminal());
    assert!(!ExecutionOutcome::AwaitApproval.is_terminal());
}

#[test]
fn execution_engine_moves_running_work_into_waiting_approval() {
    let definition_id = AgentDefinitionId::new();
    let request = ExecutionRequest::new(definition_id, "Review approval", vec!["ask human".into()]);
    let mut instance = AgentInstance::new(definition_id);
    let mut task = Task::new("Review approval".into(), vec!["ask human".into()], vec![]);

    task.start().expect("task should start");
    instance.begin_hydrating().expect("instance should hydrate");
    instance.mark_ready().expect("instance should become ready");
    instance
        .bind_active_task(task.id())
        .expect("task should bind");
    instance.begin_running().expect("instance should start");
    let packet = TaskPacket::from_request_and_task(&request, &task);

    let approval_id = ApprovalRequestId::new();
    let result = ExecutionEngine::default()
        .step(
            &mut instance,
            &mut task,
            &packet,
            StepDecision::AwaitApproval(approval_id),
        )
        .expect("step should succeed");

    assert_eq!(result.outcome, ExecutionOutcome::AwaitApproval);
    assert_eq!(result.instance_state, AgentInstanceState::WaitingApproval);
    assert_eq!(result.task_state, TaskState::InProgress);
    assert_eq!(result.approval_request_ids, vec![approval_id]);
    assert!(
        result
            .events
            .iter()
            .any(|event| matches!(event, ExecutionEvent::InstanceStateChanged { to, .. } if *to == AgentInstanceState::WaitingApproval))
    );
}

#[test]
fn execution_engine_completes_task_and_returns_terminal_result() {
    let definition_id = AgentDefinitionId::new();
    let request = ExecutionRequest::new(definition_id, "Finish kernel", vec!["close task".into()]);
    let mut instance = AgentInstance::new(definition_id);
    let mut task = Task::new("Finish kernel".into(), vec!["close task".into()], vec![]);

    task.start().expect("task should start");
    instance.begin_hydrating().expect("instance should hydrate");
    instance.mark_ready().expect("instance should become ready");
    instance
        .bind_active_task(task.id())
        .expect("task should bind");
    instance.begin_running().expect("instance should start");
    let packet = TaskPacket::from_request_and_task(&request, &task);

    let result = ExecutionEngine::default()
        .step(
            &mut instance,
            &mut task,
            &packet,
            StepDecision::CompleteTask("kernel contracts stabilized".into()),
        )
        .expect("step should succeed");

    assert_eq!(result.outcome, ExecutionOutcome::CompletedTask);
    assert_eq!(result.instance_state, AgentInstanceState::Ready);
    assert_eq!(result.task_state, TaskState::Done);
    assert_eq!(result.sequence_number, 0);
    assert_eq!(instance.state(), AgentInstanceState::Ready);
    assert_eq!(task.state(), TaskState::Done);
    assert_eq!(
        result.summary.as_deref(),
        Some("kernel contracts stabilized")
    );
    assert!(result.outcome.is_terminal());
}

#[test]
fn runtime_creates_instance_and_task_for_new_request() {
    let agent_definition = AgentDefinition::new("kernel-default", "system");
    let mut runtime = InMemoryKernelRuntime::new([agent_definition.clone()]);
    let artifact_id = torque_kernel::ArtifactId::new();
    let context_ref = ExternalContextRef {
        id: torque_kernel::ExternalContextRefId::new(),
        kind: ExternalContextKind::Document,
        locator: "doc://runtime-prd".into(),
        access_mode: AccessMode::ReadOnly,
        sync_policy: SyncPolicy::Snapshot,
        metadata: vec![],
    };
    let request = ExecutionRequest::new(
        agent_definition.id,
        "Create runtime flow",
        vec!["open a task and complete it".into()],
    )
    .with_input_artifact(artifact_id)
    .with_external_context_ref(context_ref.clone())
    .with_execution_mode(ExecutionMode::Sync);

    let result = runtime
        .handle(
            request,
            StepDecision::CompleteTask("runtime closed the first task".into()),
        )
        .expect("runtime should handle request");

    assert_eq!(result.outcome, ExecutionOutcome::CompletedTask);
    assert_eq!(result.instance_state, AgentInstanceState::Ready);
    assert_eq!(result.task_state, TaskState::Done);
    assert_eq!(
        runtime
            .instance(result.instance_id)
            .expect("instance should persist")
            .state(),
        AgentInstanceState::Ready
    );
    assert_eq!(
        runtime
            .task(result.task_id)
            .expect("task should persist")
            .state(),
        TaskState::Done
    );
    let task = runtime.task(result.task_id).expect("task should persist");
    assert!(task
        .input_refs()
        .contains(&TaskInputRef::Artifact(artifact_id)));
    assert!(task.input_refs().contains(&TaskInputRef::ExternalContext {
        context_ref_id: context_ref.id
    }));
}

#[test]
fn runtime_reuses_existing_instance_and_active_task() {
    let agent_definition = AgentDefinition::new("kernel-default", "system");
    let mut runtime = InMemoryKernelRuntime::new([agent_definition.clone()]);

    let first = runtime
        .handle(
            ExecutionRequest::new(agent_definition.id, "Continue work", vec!["begin".into()]),
            StepDecision::Continue,
        )
        .expect("first turn should succeed");

    let second = runtime
        .handle(
            ExecutionRequest::new(agent_definition.id, "Continue work", vec!["finish".into()])
                .with_instance_id(first.instance_id),
            StepDecision::CompleteTask("finished in second turn".into()),
        )
        .expect("second turn should succeed");

    assert_eq!(first.instance_id, second.instance_id);
    assert_eq!(first.task_id, second.task_id);
    assert_eq!(first.task_state, TaskState::InProgress);
    assert_eq!(second.task_state, TaskState::Done);
    assert_eq!(second.summary.as_deref(), Some("finished in second turn"));
}

#[test]
fn task_packet_derives_narrow_execution_view_from_request_and_task() {
    let agent_definition_id = AgentDefinitionId::new();
    let artifact_id = torque_kernel::ArtifactId::new();
    let context_ref = ExternalContextRef {
        id: torque_kernel::ExternalContextRefId::new(),
        kind: ExternalContextKind::Repository,
        locator: "repo://torque".into(),
        access_mode: AccessMode::ReadOnly,
        sync_policy: SyncPolicy::LazyFetch,
        metadata: vec![("branch".into(), "main".into())],
    };
    let request = ExecutionRequest::new(
        agent_definition_id,
        "Ship runtime",
        vec!["keep the execution view narrow".into()],
    )
    .with_constraint("do not inherit full transcript")
    .with_expected_output("return a structured execution result")
    .with_input_artifact(artifact_id)
    .with_external_context_ref(context_ref.clone())
    .with_execution_mode(ExecutionMode::Sync);
    let mut task = Task::new(
        "Ship runtime".into(),
        vec!["close the current work item".into()],
        vec![TaskConstraint::new("preserve task boundaries")],
    );
    task.start().expect("task should start");

    let packet = TaskPacket::from_request_and_task(&request, &task);

    assert_eq!(packet.goal, "Ship runtime");
    assert_eq!(packet.instructions.len(), 1);
    assert_eq!(packet.instructions[0], "close the current work item");
    assert_eq!(packet.constraints.len(), 2);
    assert!(packet
        .constraints
        .contains(&"preserve task boundaries".to_string()));
    assert!(packet
        .constraints
        .contains(&"do not inherit full transcript".to_string()));
    assert_eq!(
        packet.expected_outputs,
        vec!["return a structured execution result".to_string()]
    );
    assert!(packet.compact_summary.is_none());
    assert!(packet.key_facts.is_empty());
    assert_eq!(
        packet.input_refs,
        vec![
            TaskInputRef::Artifact(artifact_id),
            TaskInputRef::ExternalContext {
                context_ref_id: context_ref.id
            }
        ]
    );
    assert_eq!(packet.input_artifact_ids, vec![artifact_id]);
    assert_eq!(packet.external_context_refs, vec![context_ref]);
}

#[test]
fn task_packet_can_carry_compact_summary_as_derived_context() {
    let request = ExecutionRequest::new(
        AgentDefinitionId::new(),
        "Continue work",
        vec!["keep the packet derived".into()],
    );
    let mut task = Task::new("Continue work".into(), vec!["close the loop".into()], vec![]);
    task.start().expect("task should start");

    let packet = TaskPacket::from_request_and_task(&request, &task).with_compact_summary(
        "Compacted 8 earlier messages into a derived execution summary.",
        vec![
            "User wants a concise answer".to_string(),
            "Scratch file contains the expanded output".to_string(),
        ],
    );

    assert_eq!(
        packet.compact_summary.as_deref(),
        Some("Compacted 8 earlier messages into a derived execution summary.")
    );
    assert_eq!(packet.key_facts.len(), 2);
    assert_eq!(packet.goal, "Continue work");
}

#[test]
fn runtime_uses_store_abstraction_to_persist_state() {
    let agent_definition = AgentDefinition::new("kernel-default", "system");
    let store = InMemoryRuntimeStore::new([agent_definition.clone()]);
    let mut runtime = InMemoryKernelRuntime::with_store(store);

    let result = runtime
        .handle(
            ExecutionRequest::new(
                agent_definition.id,
                "Persist via store",
                vec!["step".into()],
            ),
            StepDecision::Continue,
        )
        .expect("runtime should persist through store");

    assert!(runtime
        .store()
        .agent_definition(agent_definition.id)
        .is_some());
    assert!(runtime.store().instance(result.instance_id).is_some());
    assert!(runtime.store().task(result.task_id).is_some());
    assert_eq!(runtime.execution_history(result.instance_id).len(), 1);
    assert_eq!(
        runtime
            .latest_execution_result(result.instance_id)
            .expect("latest result should exist")
            .outcome,
        ExecutionOutcome::Continue
    );
    assert_eq!(result.sequence_number, 1);
}

#[test]
fn runtime_can_capture_instance_checkpoint() {
    let agent_definition = AgentDefinition::new("kernel-default", "system");
    let mut runtime = InMemoryKernelRuntime::new([agent_definition.clone()]);

    let result = runtime
        .handle(
            ExecutionRequest::new(agent_definition.id, "Checkpoint flow", vec!["step".into()]),
            StepDecision::Continue,
        )
        .expect("runtime should produce first event");

    let checkpoint = runtime
        .create_checkpoint(result.instance_id)
        .expect("checkpoint should be created");

    assert_eq!(checkpoint.instance_id, result.instance_id);
    assert_eq!(checkpoint.active_task_id, Some(result.task_id));
    assert_eq!(checkpoint.active_task_state, Some(TaskState::InProgress));
    assert_eq!(checkpoint.instance_state, AgentInstanceState::Running);
    assert_eq!(checkpoint.event_sequence, result.sequence_number);
    assert_eq!(
        runtime.latest_checkpoint(result.instance_id).unwrap().id,
        checkpoint.id
    );
    assert_eq!(runtime.checkpoint_history(result.instance_id).len(), 1);
    assert_eq!(
        runtime
            .checkpoint(result.instance_id, checkpoint.id)
            .expect("checkpoint should be readable")
            .id,
        checkpoint.id
    );
}

#[test]
fn runtime_exports_checkpoint_state_view() {
    let agent_definition = AgentDefinition::new("kernel-default", "system");
    let mut runtime = InMemoryKernelRuntime::new([agent_definition.clone()]);

    let result = runtime
        .handle(
            ExecutionRequest::new(agent_definition.id, "Checkpoint flow", vec!["step".into()]),
            StepDecision::AwaitApproval(ApprovalRequestId::new()),
        )
        .expect("runtime should produce waiting approval state");

    let state_view = runtime
        .checkpoint_state_view(result.instance_id)
        .expect("checkpoint state view should be available");

    assert_eq!(state_view.instance_id, result.instance_id);
    assert_eq!(
        state_view.instance_state,
        AgentInstanceState::WaitingApproval
    );
    assert_eq!(state_view.active_task_id, Some(result.task_id));
    assert_eq!(state_view.active_task_state, Some(TaskState::InProgress));
    assert_eq!(state_view.event_sequence, result.sequence_number);
    assert_eq!(
        state_view.latest_outcome,
        Some(ExecutionOutcome::AwaitApproval)
    );

    let json = state_view
        .to_json_value()
        .expect("state view should serialize");
    let round_trip = torque_kernel::CheckpointStateView::from_json_value(json)
        .expect("state view should deserialize");
    assert_eq!(round_trip, state_view);
}

#[test]
fn runtime_builds_recovery_view_from_checkpoint_and_tail_events() {
    let agent_definition = AgentDefinition::new("kernel-default", "system");
    let mut runtime = InMemoryKernelRuntime::new([agent_definition.clone()]);

    let first = runtime
        .handle(
            ExecutionRequest::new(agent_definition.id, "Recovery flow", vec!["wait".into()]),
            StepDecision::AwaitApproval(ApprovalRequestId::new()),
        )
        .expect("first event should succeed");

    let checkpoint = runtime
        .create_checkpoint(first.instance_id)
        .expect("checkpoint should be created");

    let second = runtime
        .handle(
            ExecutionRequest::new(
                agent_definition.id,
                "Recovery flow",
                vec!["continue".into()],
            )
            .with_instance_id(first.instance_id),
            StepDecision::Continue,
        )
        .expect_err("waiting approval should still require explicit resume signal");
    let _ = second;

    let third = runtime
        .handle_command(
            ExecutionRequest::new(agent_definition.id, "Recovery flow", vec!["resume".into()])
                .with_instance_id(first.instance_id),
            RuntimeCommand::new(StepDecision::CompleteTask("done".into())).with_resume_signal(
                ResumeSignal::ApprovalGranted {
                    approval_request_id: first.approval_request_ids[0],
                },
            ),
        )
        .expect("resume should succeed");

    let recovery = runtime
        .recovery_view(first.instance_id, checkpoint.id)
        .expect("recovery view should build");

    assert_eq!(recovery.checkpoint.id, checkpoint.id);
    assert_eq!(recovery.tail_events.len(), 1);
    assert_eq!(
        recovery.tail_events[0].sequence_number,
        third.sequence_number
    );
    assert_eq!(
        recovery.tail_events[0].outcome,
        ExecutionOutcome::CompletedTask
    );
}

#[test]
fn runtime_assesses_waiting_approval_recovery_state() {
    let agent_definition = AgentDefinition::new("kernel-default", "system");
    let mut runtime = InMemoryKernelRuntime::new([agent_definition.clone()]);

    let first = runtime
        .handle(
            ExecutionRequest::new(agent_definition.id, "Recovery assess", vec!["wait".into()]),
            StepDecision::AwaitApproval(ApprovalRequestId::new()),
        )
        .expect("first request should enter waiting approval");

    let checkpoint = runtime
        .create_checkpoint(first.instance_id)
        .expect("checkpoint should be created");

    let assessment = runtime
        .assess_recovery(first.instance_id, checkpoint.id)
        .expect("recovery assessment should succeed");

    assert_eq!(
        assessment.disposition,
        RecoveryDisposition::AwaitingApproval
    );
    assert_eq!(
        assessment.recommended_action,
        RecoveryAction::AwaitApprovalDecision
    );
    assert_eq!(
        assessment.latest_outcome,
        Some(ExecutionOutcome::AwaitApproval)
    );
    assert!(!assessment.requires_replay);
    assert!(!assessment.is_terminal());
    assert!(assessment.requires_operator_action());
    assert!(assessment.summary().contains("awaiting approval"));
    assert_eq!(assessment.view.checkpoint.id, checkpoint.id);
}

#[test]
fn runtime_assesses_completed_recovery_state() {
    let agent_definition = AgentDefinition::new("kernel-default", "system");
    let mut runtime = InMemoryKernelRuntime::new([agent_definition.clone()]);

    let result = runtime
        .handle(
            ExecutionRequest::new(agent_definition.id, "Recovery assess", vec!["done".into()]),
            StepDecision::CompleteTask("done".into()),
        )
        .expect("request should complete");

    let checkpoint = runtime
        .create_checkpoint(result.instance_id)
        .expect("checkpoint should be created");

    let assessment = runtime
        .assess_recovery(result.instance_id, checkpoint.id)
        .expect("recovery assessment should succeed");

    assert_eq!(assessment.disposition, RecoveryDisposition::Completed);
    assert_eq!(
        assessment.recommended_action,
        RecoveryAction::AcceptCompletedState
    );
    assert_eq!(
        assessment.latest_outcome,
        Some(ExecutionOutcome::CompletedTask)
    );
    assert!(!assessment.requires_replay);
    assert!(assessment.is_terminal());
    assert!(!assessment.requires_operator_action());
    assert!(assessment.summary().contains("completed"));
}

#[test]
fn runtime_assesses_failed_recovery_state() {
    let agent_definition = AgentDefinition::new("kernel-default", "system");
    let mut runtime = InMemoryKernelRuntime::new([agent_definition.clone()]);

    let result = runtime
        .handle(
            ExecutionRequest::new(agent_definition.id, "Recovery assess", vec!["fail".into()]),
            StepDecision::FailTask("boom".into()),
        )
        .expect("request should fail task");

    let checkpoint = runtime
        .create_checkpoint(result.instance_id)
        .expect("checkpoint should be created");

    let assessment = runtime
        .assess_recovery(result.instance_id, checkpoint.id)
        .expect("recovery assessment should succeed");

    assert_eq!(assessment.disposition, RecoveryDisposition::Failed);
    assert_eq!(
        assessment.recommended_action,
        RecoveryAction::EscalateFailure
    );
    assert_eq!(
        assessment.latest_outcome,
        Some(ExecutionOutcome::FailedTask)
    );
    assert!(!assessment.requires_replay);
    assert!(assessment.is_terminal());
    assert!(assessment.requires_operator_action());
    assert!(assessment.summary().contains("failed"));
}

#[test]
fn runtime_can_assess_latest_checkpoint_directly() {
    let agent_definition = AgentDefinition::new("kernel-default", "system");
    let mut runtime = InMemoryKernelRuntime::new([agent_definition.clone()]);

    let result = runtime
        .handle(
            ExecutionRequest::new(agent_definition.id, "Recovery assess", vec!["done".into()]),
            StepDecision::CompleteTask("done".into()),
        )
        .expect("request should complete");

    let checkpoint = runtime
        .create_checkpoint(result.instance_id)
        .expect("checkpoint should be created");

    let assessment = runtime
        .recover_latest(result.instance_id)
        .expect("latest recovery assessment should succeed");

    assert_eq!(assessment.view.checkpoint.id, checkpoint.id);
    assert_eq!(assessment.disposition, RecoveryDisposition::Completed);
    assert_eq!(
        assessment.recommended_action,
        RecoveryAction::AcceptCompletedState
    );
    assert!(!assessment.requires_replay);
}

#[test]
fn runtime_assesses_waiting_tool_recovery_state() {
    let agent_definition = AgentDefinition::new("kernel-default", "system");
    let mut runtime = InMemoryKernelRuntime::new([agent_definition.clone()]);

    let result = runtime
        .handle(
            ExecutionRequest::new(agent_definition.id, "Recovery assess", vec!["wait".into()]),
            StepDecision::AwaitTool,
        )
        .expect("request should enter waiting tool");

    let checkpoint = runtime
        .create_checkpoint(result.instance_id)
        .expect("checkpoint should be created");

    let assessment = runtime
        .assess_recovery(result.instance_id, checkpoint.id)
        .expect("assessment should succeed");

    assert_eq!(assessment.disposition, RecoveryDisposition::AwaitingTool);
    assert_eq!(
        assessment.recommended_action,
        RecoveryAction::AwaitToolCompletion
    );
    assert!(!assessment.requires_replay);
}

#[test]
fn runtime_assesses_waiting_delegation_recovery_state() {
    let agent_definition = AgentDefinition::new("kernel-default", "system");
    let mut runtime = InMemoryKernelRuntime::new([agent_definition.clone()]);

    let result = runtime
        .handle(
            ExecutionRequest::new(agent_definition.id, "Recovery assess", vec!["wait".into()]),
            StepDecision::AwaitDelegation(torque_kernel::DelegationRequestId::new()),
        )
        .expect("request should enter waiting delegation");

    let checkpoint = runtime
        .create_checkpoint(result.instance_id)
        .expect("checkpoint should be created");

    let assessment = runtime
        .assess_recovery(result.instance_id, checkpoint.id)
        .expect("assessment should succeed");

    assert_eq!(
        assessment.disposition,
        RecoveryDisposition::AwaitingDelegation
    );
    assert_eq!(
        assessment.recommended_action,
        RecoveryAction::AwaitDelegationCompletion
    );
    assert!(!assessment.requires_replay);
}

#[test]
fn runtime_assesses_suspended_recovery_state() {
    let agent_definition = AgentDefinition::new("kernel-default", "system");
    let mut runtime = InMemoryKernelRuntime::new([agent_definition.clone()]);

    let result = runtime
        .handle(
            ExecutionRequest::new(agent_definition.id, "Recovery assess", vec!["pause".into()]),
            StepDecision::SuspendInstance,
        )
        .expect("request should suspend instance");

    let checkpoint = runtime
        .create_checkpoint(result.instance_id)
        .expect("checkpoint should be created");

    let assessment = runtime
        .assess_recovery(result.instance_id, checkpoint.id)
        .expect("assessment should succeed");

    assert_eq!(assessment.disposition, RecoveryDisposition::Suspended);
    assert_eq!(assessment.recommended_action, RecoveryAction::StaySuspended);
    assert!(!assessment.requires_replay);
}

#[test]
fn runtime_assesses_running_recovery_state_as_resume_current() {
    let agent_definition = AgentDefinition::new("kernel-default", "system");
    let mut runtime = InMemoryKernelRuntime::new([agent_definition.clone()]);

    let result = runtime
        .handle(
            ExecutionRequest::new(
                agent_definition.id,
                "Recovery assess",
                vec!["continue".into()],
            ),
            StepDecision::Continue,
        )
        .expect("request should leave runtime running");

    let checkpoint = runtime
        .create_checkpoint(result.instance_id)
        .expect("checkpoint should be created");

    let assessment = runtime
        .assess_recovery(result.instance_id, checkpoint.id)
        .expect("assessment should succeed");

    assert_eq!(assessment.disposition, RecoveryDisposition::ResumeCurrent);
    assert_eq!(
        assessment.recommended_action,
        RecoveryAction::ResumeExecution
    );
    assert!(!assessment.requires_replay);
}

#[test]
fn runtime_assessment_marks_replay_required_when_tail_events_exist() {
    let agent_definition = AgentDefinition::new("kernel-default", "system");
    let mut runtime = InMemoryKernelRuntime::new([agent_definition.clone()]);
    let approval_request_id = ApprovalRequestId::new();

    let first = runtime
        .handle(
            ExecutionRequest::new(agent_definition.id, "Recovery assess", vec!["wait".into()]),
            StepDecision::AwaitApproval(approval_request_id),
        )
        .expect("first request should wait approval");

    let checkpoint = runtime
        .create_checkpoint(first.instance_id)
        .expect("checkpoint should be created");

    let _second = runtime
        .handle_command(
            ExecutionRequest::new(
                agent_definition.id,
                "Recovery assess",
                vec!["resume".into()],
            )
            .with_instance_id(first.instance_id),
            RuntimeCommand::new(StepDecision::CompleteTask("done".into())).with_resume_signal(
                ResumeSignal::ApprovalGranted {
                    approval_request_id,
                },
            ),
        )
        .expect("resume should complete");

    let assessment = runtime
        .assess_recovery(first.instance_id, checkpoint.id)
        .expect("assessment should succeed");

    assert_eq!(assessment.disposition, RecoveryDisposition::Completed);
    assert_eq!(
        assessment.recommended_action,
        RecoveryAction::ReplayTailEvents
    );
    assert!(assessment.requires_replay);
    assert!(assessment.summary().contains("replay tail events"));
}

#[test]
fn runtime_creates_a_new_task_after_previous_task_completes() {
    let agent_definition = AgentDefinition::new("kernel-default", "system");
    let mut runtime = InMemoryKernelRuntime::new([agent_definition.clone()]);

    let first = runtime
        .handle(
            ExecutionRequest::new(
                agent_definition.id,
                "First task",
                vec!["finish first".into()],
            ),
            StepDecision::CompleteTask("first done".into()),
        )
        .expect("first request should succeed");

    let second = runtime
        .handle(
            ExecutionRequest::new(
                agent_definition.id,
                "Second task",
                vec!["start second".into()],
            )
            .with_instance_id(first.instance_id),
            StepDecision::Continue,
        )
        .expect("second request should create a fresh task");

    assert_eq!(first.instance_id, second.instance_id);
    assert_ne!(first.task_id, second.task_id);
    assert_eq!(second.task_state, TaskState::InProgress);
    assert_eq!(
        runtime
            .instance(second.instance_id)
            .expect("instance should persist")
            .active_task_id(),
        Some(second.task_id)
    );
}

#[test]
fn execution_engine_requires_task_packet_for_continue_step() {
    let definition_id = AgentDefinitionId::new();
    let request = ExecutionRequest::new(definition_id, "Continue kernel", vec!["continue".into()])
        .with_constraint("keep packet narrow");
    let mut instance = AgentInstance::new(definition_id);
    let mut task = Task::new("Continue kernel".into(), vec!["continue".into()], vec![]);

    task.start().expect("task should start");
    instance.begin_hydrating().expect("instance should hydrate");
    instance.mark_ready().expect("instance should become ready");
    instance
        .bind_active_task(task.id())
        .expect("task should bind");
    instance.begin_running().expect("instance should start");

    let packet = TaskPacket::from_request_and_task(&request, &task);
    let result = ExecutionEngine::default()
        .step(&mut instance, &mut task, &packet, StepDecision::Continue)
        .expect("continue step should succeed");

    assert_eq!(result.outcome, ExecutionOutcome::Continue);
    assert_eq!(result.instance_state, AgentInstanceState::Running);
    assert_eq!(result.task_state, TaskState::InProgress);
}

#[test]
fn runtime_resumes_waiting_approval_before_completing_task() {
    let agent_definition = AgentDefinition::new("kernel-default", "system");
    let mut runtime = InMemoryKernelRuntime::new([agent_definition.clone()]);
    let approval_request_id = ApprovalRequestId::new();

    let first = runtime
        .handle(
            ExecutionRequest::new(agent_definition.id, "Approval flow", vec!["wait".into()]),
            StepDecision::AwaitApproval(approval_request_id),
        )
        .expect("first request should enter waiting approval");

    assert_eq!(first.instance_state, AgentInstanceState::WaitingApproval);
    assert_eq!(first.task_state, TaskState::InProgress);

    let second = runtime
        .handle_command(
            ExecutionRequest::new(agent_definition.id, "Approval flow", vec!["resume".into()])
                .with_instance_id(first.instance_id),
            RuntimeCommand::new(StepDecision::CompleteTask("approval granted".into()))
                .with_resume_signal(ResumeSignal::ApprovalGranted {
                    approval_request_id,
                }),
        )
        .expect("second request should resume and complete");

    assert_eq!(second.instance_id, first.instance_id);
    assert_eq!(second.task_id, first.task_id);
    assert_eq!(second.outcome, ExecutionOutcome::CompletedTask);
    assert_eq!(second.instance_state, AgentInstanceState::Ready);
    assert_eq!(second.task_state, TaskState::Done);
    assert!(second
        .events
        .iter()
        .any(|event| matches!(event, ExecutionEvent::ResumeApplied { .. })));
    assert!(runtime
        .instance(second.instance_id)
        .expect("instance should persist")
        .pending_approval_ids()
        .is_empty());
    let history = runtime.execution_history(second.instance_id);
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].sequence_number, 1);
    assert_eq!(history[1].sequence_number, 2);
    assert_eq!(history[0].outcome, ExecutionOutcome::AwaitApproval);
    assert_eq!(history[1].outcome, ExecutionOutcome::CompletedTask);
    assert_eq!(
        runtime
            .latest_execution_result(second.instance_id)
            .expect("latest result should exist")
            .outcome,
        ExecutionOutcome::CompletedTask
    );
    assert_eq!(second.sequence_number, 2);
}

#[test]
fn runtime_rejects_completing_waiting_approval_without_resume_signal() {
    let agent_definition = AgentDefinition::new("kernel-default", "system");
    let mut runtime = InMemoryKernelRuntime::new([agent_definition.clone()]);

    let first = runtime
        .handle(
            ExecutionRequest::new(agent_definition.id, "Approval flow", vec!["wait".into()]),
            StepDecision::AwaitApproval(ApprovalRequestId::new()),
        )
        .expect("first request should enter waiting approval");

    let error = runtime
        .handle(
            ExecutionRequest::new(agent_definition.id, "Approval flow", vec!["resume".into()])
                .with_instance_id(first.instance_id),
            StepDecision::CompleteTask("approval granted".into()),
        )
        .expect_err("resume should require explicit approval signal");

    assert!(
        error.to_string().contains("missing approval resume signal"),
        "unexpected error: {error}"
    );
}

#[test]
fn runtime_rejects_resuming_waiting_tool_without_tool_signal() {
    let agent_definition = AgentDefinition::new("kernel-default", "system");
    let mut runtime = InMemoryKernelRuntime::new([agent_definition.clone()]);

    let first = runtime
        .handle(
            ExecutionRequest::new(agent_definition.id, "Tool flow", vec!["wait".into()]),
            StepDecision::AwaitTool,
        )
        .expect("first request should enter waiting tool");

    let error = runtime
        .handle(
            ExecutionRequest::new(agent_definition.id, "Tool flow", vec!["resume".into()])
                .with_instance_id(first.instance_id),
            StepDecision::Continue,
        )
        .expect_err("resume should require explicit tool signal");

    assert!(
        error.to_string().contains("missing tool resume signal"),
        "unexpected error: {error}"
    );
}

#[test]
fn runtime_resumes_waiting_tool_with_explicit_signal() {
    let agent_definition = AgentDefinition::new("kernel-default", "system");
    let mut runtime = InMemoryKernelRuntime::new([agent_definition.clone()]);

    let first = runtime
        .handle(
            ExecutionRequest::new(agent_definition.id, "Tool flow", vec!["wait".into()]),
            StepDecision::AwaitTool,
        )
        .expect("first request should enter waiting tool");

    let second = runtime
        .handle_command(
            ExecutionRequest::new(agent_definition.id, "Tool flow", vec!["resume".into()])
                .with_instance_id(first.instance_id),
            RuntimeCommand::new(StepDecision::Continue)
                .with_resume_signal(ResumeSignal::ToolCompleted),
        )
        .expect("resume should succeed with tool signal");

    assert_eq!(second.instance_state, AgentInstanceState::Running);
    assert_eq!(second.task_state, TaskState::InProgress);
    assert!(second
        .events
        .iter()
        .any(|event| matches!(event, ExecutionEvent::ResumeApplied { .. })));
}

#[test]
fn runtime_resumes_waiting_delegation_with_explicit_signal() {
    let agent_definition = AgentDefinition::new("kernel-default", "system");
    let mut runtime = InMemoryKernelRuntime::new([agent_definition.clone()]);
    let delegation_request_id = torque_kernel::DelegationRequestId::new();

    let first = runtime
        .handle(
            ExecutionRequest::new(agent_definition.id, "Delegation flow", vec!["wait".into()]),
            StepDecision::AwaitDelegation(delegation_request_id),
        )
        .expect("first request should enter waiting delegation");

    let second = runtime
        .handle_command(
            ExecutionRequest::new(
                agent_definition.id,
                "Delegation flow",
                vec!["resume".into()],
            )
            .with_instance_id(first.instance_id),
            RuntimeCommand::new(StepDecision::Continue).with_resume_signal(
                ResumeSignal::DelegationCompleted {
                    delegation_request_id,
                },
            ),
        )
        .expect("resume should succeed with delegation signal");

    assert_eq!(second.instance_state, AgentInstanceState::Running);
    assert_eq!(second.task_state, TaskState::InProgress);
    assert!(second
        .events
        .iter()
        .any(|event| matches!(event, ExecutionEvent::ResumeApplied { .. })));
    assert!(runtime
        .instance(second.instance_id)
        .expect("instance should persist")
        .child_delegation_ids()
        .is_empty());
}

#[test]
fn runtime_resumes_suspended_instance_with_manual_signal() {
    let agent_definition = AgentDefinition::new("kernel-default", "system");
    let mut runtime = InMemoryKernelRuntime::new([agent_definition.clone()]);

    let first = runtime
        .handle(
            ExecutionRequest::new(agent_definition.id, "Suspend flow", vec!["pause".into()]),
            StepDecision::SuspendInstance,
        )
        .expect("first request should suspend instance");

    let second = runtime
        .handle_command(
            ExecutionRequest::new(agent_definition.id, "Suspend flow", vec!["resume".into()])
                .with_instance_id(first.instance_id),
            RuntimeCommand::new(StepDecision::Continue)
                .with_resume_signal(ResumeSignal::ManualResume),
        )
        .expect("manual resume should succeed");

    assert_eq!(second.instance_state, AgentInstanceState::Running);
    assert!(second
        .events
        .iter()
        .any(|event| matches!(event, ExecutionEvent::ResumeApplied { .. })));
}
