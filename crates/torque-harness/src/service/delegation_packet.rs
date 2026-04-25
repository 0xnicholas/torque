use serde_json::{json, Map, Value};

pub fn build_delegation_packet(
    goal: &str,
    instructions: Option<&str>,
    selected_artifacts: Vec<String>,
    selected_context_refs: Vec<String>,
    constraints: Vec<String>,
    compact_summary: Option<String>,
    key_facts: Vec<String>,
) -> Value {
    let mut packet = Map::new();
    packet.insert("goal".to_string(), json!(goal));
    packet.insert(
        "selected_artifacts".to_string(),
        json!(selected_artifacts),
    );
    packet.insert(
        "selected_context_refs".to_string(),
        json!(selected_context_refs),
    );
    packet.insert("constraints".to_string(), json!(constraints));
    packet.insert("key_facts".to_string(), json!(key_facts));

    if let Some(instructions) = instructions.filter(|value| !value.trim().is_empty()) {
        packet.insert("instructions".to_string(), json!(instructions));
    }

    if let Some(compact_summary) = compact_summary.filter(|value| !value.trim().is_empty()) {
        packet.insert("compact_summary".to_string(), json!(compact_summary));
    }

    Value::Object(packet)
}
