use torque_harness::service::team::event_listener::parse_delegation_event;
use torque_harness::models::v1::delegation_event::DelegationEvent;
use uuid::Uuid;

#[test]
fn test_parse_delegation_event_completed() {
    let data = serde_json::json!({
        "type": "completed",
        "data": {
            "delegation_id": "550e8400-e29b-41d4-a716-446655440000",
            "member_id": "550e8400-e29b-41d4-a716-446655440001",
            "artifact_id": "550e8400-e29b-41d4-a716-446655440002"
        }
    });
    let event = parse_delegation_event(&data);

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
            "delegation_id": "550e8400-e29b-41d4-a716-446655440000",
            "member_id": "550e8400-e29b-41d4-a716-446655440001",
            "error": "something went wrong"
        }
    });
    let event = parse_delegation_event(&data);

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
            "delegation_id": "550e8400-e29b-41d4-a716-446655440000",
            "member_id": "550e8400-e29b-41d4-a716-446655440001"
        }
    });
    let event = parse_delegation_event(&data);

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
    let event = parse_delegation_event(&data);

    assert!(event.is_none());
}

#[test]
fn test_parse_delegation_event_rejected() {
    let data = serde_json::json!({
        "type": "rejected",
        "data": {
            "delegation_id": "550e8400-e29b-41d4-a716-446655440000",
            "member_id": "550e8400-e29b-41d4-a716-446655440001",
            "reason": "capacity_full"
        }
    });
    let event = parse_delegation_event(&data);

    assert!(matches!(
        event,
        Some(DelegationEvent::Rejected { .. })
    ));
}

#[test]
fn test_parse_delegation_event_extension_requested() {
    let data = serde_json::json!({
        "type": "extension_requested",
        "data": {
            "delegation_id": "550e8400-e29b-41d4-a716-446655440000",
            "member_id": "550e8400-e29b-41d4-a716-446655440001",
            "requested_seconds": 30,
            "reason": "need more time"
        }
    });
    let event = parse_delegation_event(&data);

    assert!(matches!(
        event,
        Some(DelegationEvent::ExtensionRequested { .. })
    ));
}