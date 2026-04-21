use crate::models::v1::event::Event;
use chrono::Utc;
use torque_kernel::{ExecutionEvent, ExecutionResult};
use uuid::Uuid;

pub struct EventRecorder;

impl EventRecorder {
    pub fn to_db_events(result: &ExecutionResult, sequence_offset: u64) -> Vec<Event> {
        let mut events = Vec::new();
        let seq = sequence_offset as i64;

        for (idx, event) in result.events.iter().enumerate() {
            let db_event = match event {
                ExecutionEvent::InstanceStateChanged { from, to } => Event {
                    event_id: Uuid::new_v4(),
                    event_type: "instance_state_changed".to_string(),
                    timestamp: Utc::now(),
                    resource_type: "agent_instance".to_string(),
                    resource_id: result.instance_id.as_uuid(),
                    payload: serde_json::json!({
                        "from": format!("{:?}", from),
                        "to": format!("{:?}", to),
                        "task_id": result.task_id.as_uuid(),
                    }),
                    sequence_number: Some(seq + idx as i64),
                },
                ExecutionEvent::TaskStateChanged { from, to } => Event {
                    event_id: Uuid::new_v4(),
                    event_type: "task_state_changed".to_string(),
                    timestamp: Utc::now(),
                    resource_type: "task".to_string(),
                    resource_id: result.task_id.as_uuid(),
                    payload: serde_json::json!({
                        "from": format!("{:?}", from),
                        "to": format!("{:?}", to),
                    }),
                    sequence_number: Some(seq + idx as i64),
                },
                ExecutionEvent::ArtifactProduced { artifact_id } => Event {
                    event_id: Uuid::new_v4(),
                    event_type: "artifact_produced".to_string(),
                    timestamp: Utc::now(),
                    resource_type: "task".to_string(),
                    resource_id: result.task_id.as_uuid(),
                    payload: serde_json::json!({
                        "artifact_id": artifact_id.as_uuid(),
                    }),
                    sequence_number: Some(seq + idx as i64),
                },
                ExecutionEvent::ApprovalRequested {
                    approval_request_id,
                } => Event {
                    event_id: Uuid::new_v4(),
                    event_type: "approval_requested".to_string(),
                    timestamp: Utc::now(),
                    resource_type: "agent_instance".to_string(),
                    resource_id: result.instance_id.as_uuid(),
                    payload: serde_json::json!({
                        "approval_request_id": approval_request_id.as_uuid(),
                    }),
                    sequence_number: Some(seq + idx as i64),
                },
                ExecutionEvent::DelegationRequested {
                    delegation_request_id,
                } => Event {
                    event_id: Uuid::new_v4(),
                    event_type: "delegation_requested".to_string(),
                    timestamp: Utc::now(),
                    resource_type: "agent_instance".to_string(),
                    resource_id: result.instance_id.as_uuid(),
                    payload: serde_json::json!({
                        "delegation_request_id": delegation_request_id.as_uuid(),
                    }),
                    sequence_number: Some(seq + idx as i64),
                },
                ExecutionEvent::ResumeApplied { resume_signal } => Event {
                    event_id: Uuid::new_v4(),
                    event_type: "resume_applied".to_string(),
                    timestamp: Utc::now(),
                    resource_type: "agent_instance".to_string(),
                    resource_id: result.instance_id.as_uuid(),
                    payload: serde_json::json!({
                        "resume_signal": format!("{:?}", resume_signal),
                    }),
                    sequence_number: Some(seq + idx as i64),
                },
            };
            events.push(db_event);
        }
        events
    }

    pub fn checkpoint_created_event(
        checkpoint_id: Uuid,
        instance_id: Uuid,
        reason: &str,
    ) -> Event {
        Event {
            event_id: Uuid::new_v4(),
            event_type: "checkpoint.created".to_string(),
            timestamp: Utc::now(),
            resource_type: "agent_instance".to_string(),
            resource_id: instance_id,
            payload: serde_json::json!({
                "checkpoint_id": checkpoint_id,
                "reason": reason,
            }),
            sequence_number: None,
        }
    }
}
