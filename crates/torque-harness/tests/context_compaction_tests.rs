use torque_harness::infra::llm::LlmMessage;
use torque_harness::service::{CompactSummary, ContextCompactionPolicy, ContextCompactionService};

fn long_messages(count: usize) -> Vec<LlmMessage> {
    (0..count)
        .map(|idx| LlmMessage::user(format!("message-{idx} {}", "x".repeat(80))))
        .collect()
}

#[test]
fn context_compaction_tests_policy_triggers_on_message_count() {
    let policy = ContextCompactionPolicy {
        message_threshold: 4,
        estimated_token_threshold: 10_000,
        preserve_recent_messages: 2,
        preview_chars: 120,
    };

    assert!(policy.should_compact(&long_messages(5)));
    assert!(!policy.should_compact(&long_messages(3)));
}

#[test]
fn context_compaction_tests_policy_triggers_on_estimated_tokens() {
    let policy = ContextCompactionPolicy {
        message_threshold: 100,
        estimated_token_threshold: 10,
        preserve_recent_messages: 2,
        preview_chars: 120,
    };

    assert!(policy.should_compact(&long_messages(2)));
}

#[test]
fn context_compaction_tests_compacts_older_messages_and_preserves_recent_tail() {
    let service = ContextCompactionService::new(ContextCompactionPolicy {
        message_threshold: 4,
        estimated_token_threshold: 10_000,
        preserve_recent_messages: 2,
        preview_chars: 120,
    });

    let messages = long_messages(5);
    let result = service
        .compact(&messages)
        .expect("compaction should produce a summary");

    assert!(result.compact_summary.contains("Compacted 3 earlier messages"));
    assert_eq!(result.key_facts.len(), 3);
    assert_eq!(result.preserved_tail.len(), 2);
    assert!(result.preserved_tail[0].content.contains("message-3"));
    assert!(result.preserved_tail[1].content.contains("message-4"));
}

#[test]
fn context_compaction_tests_returns_none_when_below_threshold() {
    let service = ContextCompactionService::default();
    let messages = vec![
        LlmMessage::user("short-1".to_string()),
        LlmMessage::assistant("short-2".to_string()),
    ];

    assert!(service.compact(&messages).is_none());
}
