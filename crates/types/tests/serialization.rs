use types::{Node, NodeStatus, QueueEntry, QueueStatus, Run, RunStatus};

#[test]
fn test_run_serialization() {
    let run = Run::new(
        uuid::Uuid::new_v4(),
        "Test instruction".to_string(),
        "abort".to_string(),
    );

    let json = serde_json::to_string(&run).unwrap();
    let parsed: Run = serde_json::from_str(&json).unwrap();

    assert_eq!(run.id, parsed.id);
    assert_eq!(run.status, RunStatus::Planning);
}

#[test]
fn test_node_serialization() {
    let node = Node::new(
        uuid::Uuid::new_v4(),
        uuid::Uuid::new_v4(),
        "researcher".to_string(),
        "Search for X".to_string(),
    );

    let json = serde_json::to_string(&node).unwrap();
    let parsed: Node = serde_json::from_str(&json).unwrap();

    assert_eq!(node.id, parsed.id);
    assert_eq!(node.status, NodeStatus::Pending);
}

#[test]
fn test_queue_entry_serialization() {
    let entry = QueueEntry::new(
        uuid::Uuid::new_v4(),
        uuid::Uuid::new_v4(),
        uuid::Uuid::new_v4(),
        0,
    );

    let json = serde_json::to_string(&entry).unwrap();
    let parsed: QueueEntry = serde_json::from_str(&json).unwrap();

    assert_eq!(entry.id, parsed.id);
    assert_eq!(entry.status, QueueStatus::Pending);
}
