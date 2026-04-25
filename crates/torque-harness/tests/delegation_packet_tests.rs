use serde_json::json;
use torque_harness::service::build_delegation_packet;

#[test]
fn delegation_packet_tests_builds_narrow_child_packet() {
    let packet = build_delegation_packet(
        "Implement the parser",
        Some("Return a concise summary"),
        vec!["artifact://123".to_string()],
        vec!["context://repo".to_string()],
        vec!["do not read full transcript".to_string()],
        Some("Compacted 6 earlier messages.".to_string()),
        vec!["Parser bug is isolated to one file".to_string()],
    );

    assert_eq!(packet["goal"], "Implement the parser");
    assert_eq!(packet["instructions"], "Return a concise summary");
    assert_eq!(packet["selected_artifacts"], json!(["artifact://123"]));
    assert_eq!(packet["selected_context_refs"], json!(["context://repo"]));
    assert_eq!(
        packet["constraints"],
        json!(["do not read full transcript"])
    );
    assert_eq!(
        packet["compact_summary"],
        "Compacted 6 earlier messages."
    );
    assert_eq!(
        packet["key_facts"],
        json!(["Parser bug is isolated to one file"])
    );
    assert!(packet.get("message_history").is_none());
}

#[test]
fn delegation_packet_tests_omits_optional_fields_when_empty() {
    let packet = build_delegation_packet(
        "Review the patch",
        None,
        vec![],
        vec![],
        vec![],
        None,
        vec![],
    );

    assert_eq!(packet["goal"], "Review the patch");
    assert!(packet.get("instructions").is_none());
    assert_eq!(packet["selected_artifacts"], json!([]));
    assert_eq!(packet["selected_context_refs"], json!([]));
    assert_eq!(packet["constraints"], json!([]));
    assert!(packet.get("compact_summary").is_none());
    assert_eq!(packet["key_facts"], json!([]));
}
