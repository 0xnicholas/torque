use torque_harness::models::v1::delegation_event::DelegationEvent;
use uuid::Uuid;

fn parse_delegation_event(data: &serde_json::Value, delegation_id: Uuid) -> Option<DelegationEvent> {
    let type_field = data.get("type")?.as_str()?;
    let event_data = data.get("data")?;

    match type_field {
        "created" => {
            Some(DelegationEvent::Created {
                delegation_id,
                task_id: event_data.get("task_id")?.as_str()?.parse().ok()?,
                member_id: event_data.get("member_id")?.as_str()?.parse().ok()?,
                created_at: chrono::Utc::now(),
            })
        }
        "accepted" => {
            Some(DelegationEvent::Accepted {
                delegation_id,
                member_id: event_data.get("member_id")?.as_str()?.parse().ok()?,
                accepted_at: chrono::Utc::now(),
            })
        }
        "completed" => {
            Some(DelegationEvent::Completed {
                delegation_id,
                member_id: event_data.get("member_id")?.as_str()?.parse().ok()?,
                artifact_id: event_data.get("artifact_id")?.as_str()?.parse().ok()?,
                completed_at: chrono::Utc::now(),
            })
        }
        "failed" => {
            Some(DelegationEvent::Failed {
                delegation_id,
                member_id: event_data.get("member_id")?.as_str()?.parse().ok()?,
                error: event_data.get("error")?.as_str()?.to_string(),
                failed_at: chrono::Utc::now(),
            })
        }
        _ => None,
    }
}

#[test]
fn test_parse_delegation_event_completed() {
    let data = serde_json::json!({
        "type": "completed",
        "data": {
            "member_id": "550e8400-e29b-41d4-a716-446655440000",
            "artifact_id": "550e8400-e29b-41d4-a716-446655440001"
        }
    });
    let delegation_id = Uuid::new_v4();
    let event = parse_delegation_event(&data, delegation_id);

    assert!(matches!(
        event,
        Some(DelegationEvent::Completed { .. })
    ));
}

#[test]
fn test_parse_delegation_event_failed() {
    let data = serde_json::json!({
        "type": "failed",
        "data": {
            "member_id": "550e8400-e29b-41d4-a716-446655440000",
            "error": "something went wrong"
        }
    });
    let delegation_id = Uuid::new_v4();
    let event = parse_delegation_event(&data, delegation_id);

    assert!(matches!(
        event,
        Some(DelegationEvent::Failed { .. })
    ));
}

#[test]
fn test_parse_delegation_event_accepted() {
    let data = serde_json::json!({
        "type": "accepted",
        "data": {
            "member_id": "550e8400-e29b-41d4-a716-446655440000"
        }
    });
    let delegation_id = Uuid::new_v4();
    let event = parse_delegation_event(&data, delegation_id);

    assert!(matches!(
        event,
        Some(DelegationEvent::Accepted { .. })
    ));
}

#[test]
fn test_parse_delegation_event_unknown_type() {
    let data = serde_json::json!({
        "type": "unknown",
        "data": {}
    });
    let delegation_id = Uuid::new_v4();
    let event = parse_delegation_event(&data, delegation_id);

    assert!(event.is_none());
}