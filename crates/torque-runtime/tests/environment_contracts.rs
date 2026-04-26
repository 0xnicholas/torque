use torque_runtime::checkpoint::{HydrationState, RuntimeCheckpointPayload, RuntimeCheckpointRef};
use torque_runtime::environment::{
    ApprovalGateway, RuntimeCheckpointSink, RuntimeEventSink, RuntimeExecutionContext,
    RuntimeHydrationSource, RuntimeModelDriver, RuntimeOutputSink, RuntimeToolExecutor,
};
use torque_runtime::events::{ModelTurnResult, RuntimeFinishReason, RuntimeOutputEvent};
use torque_runtime::message::{RuntimeMessage, RuntimeMessageRole};
use torque_runtime::tools::{RuntimeOffloadRef, RuntimeToolDef, RuntimeToolResult};
use checkpointer::r#trait::ArtifactPointer;

#[test]
fn runtime_message_round_trips_role_and_content() {
    let message = RuntimeMessage::new(RuntimeMessageRole::Assistant, "hello");
    assert_eq!(message.role, RuntimeMessageRole::Assistant);
    assert_eq!(message.content, "hello");

    let llm_message: llm::Message = message.clone().into();
    assert_eq!(llm_message.role, "assistant");
    assert_eq!(llm_message.content, "hello");

    let round_trip = RuntimeMessage::from(llm_message);
    assert_eq!(round_trip, message);
}

#[test]
fn runtime_tool_result_can_carry_offload_metadata() {
    let result = RuntimeToolResult {
        success: true,
        content: "stored elsewhere".to_string(),
        error: None,
        offload_ref: Some(RuntimeOffloadRef {
            storage: "artifact".to_string(),
            locator: "artifact://123".to_string(),
            artifact_id: None,
        }),
    };

    assert!(result.success);
    assert_eq!(result.offload_ref.as_ref().unwrap().storage, "artifact");
}

#[test]
fn runtime_checkpoint_payload_captures_kernel_owned_checkpoint_context() {
    let payload = RuntimeCheckpointPayload {
        instance_id: torque_kernel::AgentInstanceId::new(),
        node_id: uuid::Uuid::new_v4(),
        reason: "task_complete".to_string(),
        state: checkpointer::CheckpointState {
            messages: vec![],
            tool_call_count: 1,
            intermediate_results: vec![ArtifactPointer {
                task_id: "task-1".to_string(),
                storage: "artifact".to_string(),
                location: "artifact://1".to_string(),
                size_bytes: 4,
                content_type: "text/plain".to_string(),
            }],
            custom_state: None,
        },
    };

    assert_eq!(payload.reason, "task_complete");
    assert_eq!(payload.state.tool_call_count, 1);
}

#[test]
fn runtime_output_event_is_transport_neutral() {
    let event = RuntimeOutputEvent::CheckpointCreated {
        checkpoint_id: uuid::Uuid::nil(),
        reason: "awaiting_llm".to_string(),
    };

    let serialized = serde_json::to_string(&event).expect("event should serialize");
    assert!(!serialized.contains("StreamEvent"));
    assert!(serialized.contains("checkpoint_created"));
}

#[test]
fn runtime_ports_are_publicly_available() {
    let _ = std::any::type_name::<dyn RuntimeModelDriver>();
    let _ = std::any::type_name::<dyn RuntimeToolExecutor>();
    let _ = std::any::type_name::<dyn RuntimeEventSink>();
    let _ = std::any::type_name::<dyn RuntimeCheckpointSink>();
    let _ = std::any::type_name::<dyn RuntimeHydrationSource>();
    let _ = std::any::type_name::<dyn RuntimeOutputSink>();
    let _ = std::any::type_name::<dyn ApprovalGateway>();
    let _ = std::any::type_name::<RuntimeExecutionContext>();
    let _ = std::any::type_name::<RuntimeToolDef>();
    let _ = std::any::type_name::<RuntimeCheckpointRef>();
    let _ = std::any::type_name::<HydrationState>();
    let _ = std::any::type_name::<ModelTurnResult>();
    let _ = std::any::type_name::<RuntimeFinishReason>();
}
