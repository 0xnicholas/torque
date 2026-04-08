use session_agent::agent::StreamEvent;
use serde_json::json;
use uuid::Uuid;

#[test]
fn start_event_serializes_to_sse() {
    let session_id = Uuid::new_v4();
    let event = StreamEvent::Start { session_id };
    let sse = event.to_sse();

    assert!(sse.contains("\"event\":\"start\""));
    assert!(sse.contains(&session_id.to_string()));
    assert!(sse.starts_with("data: "));
    assert!(sse.ends_with("\n\n"));
}

#[test]
fn done_event_serializes_to_sse() {
    let message_id = Uuid::new_v4();
    let event = StreamEvent::Done {
        message_id,
        artifacts: Some(json!({"kind": "note"})),
    };
    let sse = event.to_sse();

    assert!(sse.contains("\"event\":\"done\""));
    assert!(sse.contains(&message_id.to_string()));
    assert!(sse.contains("\"kind\":\"note\""));
}
