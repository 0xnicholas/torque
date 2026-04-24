mod common;

use chrono::{Duration, Utc};
use common::setup_test_db_or_skip;
use serial_test::serial;
use torque_harness::models::v1::memory::{MemoryDecisionLog, MemoryWriteCandidate, MemoryWriteCandidateStatus};
use torque_harness::repository::{MemoryRepositoryV1, PostgresMemoryRepositoryV1};
use uuid::Uuid;

async fn setup_repo() -> Option<PostgresMemoryRepositoryV1> {
    let db = setup_test_db_or_skip().await?;
    Some(PostgresMemoryRepositoryV1::new(db))
}

fn create_decision_log(
    repo: &PostgresMemoryRepositoryV1,
    decision_type: &str,
    processed_by: &str,
) -> MemoryDecisionLog {
    let factors = serde_json::json!({
        "reason": "test decision",
        "confidence": 0.95
    });

    tokio::runtime::Handle::current()
        .block_on(async {
            repo.log_decision(
                None,
                None,
                decision_type,
                Some("test reason"),
                factors,
                processed_by,
            )
            .await
            .expect("decision should be logged")
        })
}

fn create_decision_log_with_candidate(
    repo: &PostgresMemoryRepositoryV1,
    agent_instance_id: Uuid,
    decision_type: &str,
    processed_by: &str,
) -> (MemoryDecisionLog, Uuid) {
    let factors = serde_json::json!({
        "reason": "test decision",
        "confidence": 0.95
    });

    let candidate_id = Uuid::new_v4();
    let now = chrono::Utc::now();

    let candidate = MemoryWriteCandidate {
        id: candidate_id,
        agent_instance_id,
        team_instance_id: None,
        content: serde_json::json!({"key": "test", "value": "test"}),
        reasoning: Some("test reasoning".to_string()),
        status: MemoryWriteCandidateStatus::Pending,
        memory_entry_id: None,
        reviewed_by: None,
        created_at: now,
        reviewed_at: None,
        updated_at: now,
    };

    tokio::runtime::Handle::current()
        .block_on(async {
            repo.create_candidate(&candidate)
                .await
                .expect("candidate should be created");

            let decision = repo
                .log_decision(
                    Some(candidate_id),
                    None,
                    decision_type,
                    Some("test reason"),
                    factors,
                    processed_by,
                )
                .await
                .expect("decision should be logged");

            (decision, candidate_id)
        })
}

#[tokio::test]
#[serial]
async fn list_decisions_returns_all_decisions_when_no_filters() {
    let repo = match setup_repo().await {
        Some(r) => r,
        None => return,
    };

    let decision1 = create_decision_log(&repo, "approval", "system");
    let decision2 = create_decision_log(&repo, "rejection", "admin");
    let decision3 = create_decision_log(&repo, "approval", "system");

    let decisions = repo
        .list_decisions(None, None, None, None, 100, 0)
        .await
        .expect("list_decisions should succeed");

    assert!(decisions.len() >= 3);
    let ids: Vec<_> = decisions.iter().map(|d| d.id).collect();
    assert!(ids.contains(&decision1.id));
    assert!(ids.contains(&decision2.id));
    assert!(ids.contains(&decision3.id));
}

#[tokio::test]
#[serial]
async fn list_decisions_filters_by_decision_type() {
    let repo = match setup_repo().await {
        Some(r) => r,
        None => return,
    };

    create_decision_log(&repo, "approval", "system");
    create_decision_log(&repo, "rejection", "system");
    create_decision_log(&repo, "approval", "admin");

    let approvals = repo
        .list_decisions(None, Some("approval"), None, None, 100, 0)
        .await
        .expect("list_decisions should succeed");

    for decision in approvals {
        assert_eq!(decision.decision_type, "approval");
    }

    let rejections = repo
        .list_decisions(None, Some("rejection"), None, None, 100, 0)
        .await
        .expect("list_decisions should succeed");

    for decision in rejections {
        assert_eq!(decision.decision_type, "rejection");
    }
}

#[tokio::test]
#[serial]
async fn list_decisions_filters_by_date_range() {
    let repo = match setup_repo().await {
        Some(r) => r,
        None => return,
    };

    let now = Utc::now();
    let yesterday = now - Duration::days(1);
    let tomorrow = now + Duration::days(1);

    create_decision_log(&repo, "approval", "system");

    let decisions = repo
        .list_decisions(None, None, Some(yesterday), Some(tomorrow), 100, 0)
        .await
        .expect("list_decisions should succeed");

    assert!(!decisions.is_empty());
}

#[tokio::test]
#[serial]
async fn list_decisions_respects_limit_and_offset() {
    let repo = match setup_repo().await {
        Some(r) => r,
        None => return,
    };

    for i in 0..5 {
        create_decision_log(&repo, &format!("type_{}", i % 3), "system");
    }

    let first_page = repo
        .list_decisions(None, None, None, None, 2, 0)
        .await
        .expect("list_decisions should succeed");

    assert_eq!(first_page.len(), 2);

    let second_page = repo
        .list_decisions(None, None, None, None, 2, 2)
        .await
        .expect("list_decisions should succeed");

    assert_eq!(second_page.len(), 2);

    let first_ids: Vec<_> = first_page.iter().map(|d| d.id).collect();
    let second_ids: Vec<_> = second_page.iter().map(|d| d.id).collect();
    for id in first_ids.iter() {
        assert!(!second_ids.contains(id), "pages should not overlap");
    }
}

#[tokio::test]
#[serial]
async fn list_decisions_orders_by_created_at_desc() {
    let repo = match setup_repo().await {
        Some(r) => r,
        None => return,
    };

    let decision1 = create_decision_log(&repo, "type_a", "system");
    let decision2 = create_decision_log(&repo, "type_b", "system");
    let decision3 = create_decision_log(&repo, "type_c", "system");

    let decisions = repo
        .list_decisions(None, None, None, None, 10, 0)
        .await
        .expect("list_decisions should succeed");

    assert!(decisions.len() >= 3);

    let recent_ids: Vec<_> = vec![decision3.id, decision2.id, decision1.id];
    for (i, decision) in decisions.iter().take(3).enumerate() {
        assert_eq!(
            decision.id, recent_ids[i],
            "decisions should be ordered by created_at DESC"
        );
    }
}

#[tokio::test]
#[serial]
async fn list_decisions_combines_multiple_filters() {
    let repo = match setup_repo().await {
        Some(r) => r,
        None => return,
    };

    let now = Utc::now();
    let yesterday = now - Duration::days(1);
    let tomorrow = now + Duration::days(1);

    create_decision_log(&repo, "approval", "system");
    create_decision_log(&repo, "rejection", "system");
    let _decision_in_range = create_decision_log(&repo, "approval", "admin");

    let old_decision = {
        let _old_time = now - Duration::days(30);
        repo.log_decision(
            None,
            None,
            "approval",
            Some("old decision"),
            serde_json::json!({"reason": "old"}),
            "system",
        )
        .await
        .expect("decision should be logged")
    };

    let decisions = repo
        .list_decisions(None, Some("approval"), Some(yesterday), Some(tomorrow), 100, 0)
        .await
        .expect("list_decisions should succeed");

    for decision in &decisions {
        assert_eq!(decision.decision_type, "approval");
        assert!(decision.created_at >= yesterday && decision.created_at <= tomorrow);
    }
    assert!(!decisions.iter().any(|d| d.id == old_decision.id));
}

#[tokio::test]
#[serial]
async fn list_decisions_returns_empty_when_no_matches() {
    let repo = match setup_repo().await {
        Some(r) => r,
        None => return,
    };

    create_decision_log(&repo, "approval", "system");

    let decisions = repo
        .list_decisions(None, Some("nonexistent_type"), None, None, 100, 0)
        .await
        .expect("list_decisions should succeed");

    assert!(decisions.is_empty());
}

#[tokio::test]
#[serial]
async fn list_decisions_filters_by_agent_instance_id() {
    let repo = match setup_repo().await {
        Some(r) => r,
        None => return,
    };

    let agent_a = Uuid::new_v4();
    let agent_b = Uuid::new_v4();

    let (decision_a, _candidate_a) =
        create_decision_log_with_candidate(&repo, agent_a, "approval", "system");
    let (_decision_b, _candidate_b) =
        create_decision_log_with_candidate(&repo, agent_b, "rejection", "system");
    let (_decision_no_candidate, _) =
        create_decision_log_with_candidate(&repo, agent_a, "review", "system");

    let decisions_for_a = repo
        .list_decisions(Some(agent_a), None, None, None, 100, 0)
        .await
        .expect("list_decisions should succeed");

    assert!(!decisions_for_a.is_empty());
    for decision in &decisions_for_a {
        assert_eq!(decision.candidate_id, Some(decision_a.candidate_id.unwrap()));
    }

    let decisions_for_b = repo
        .list_decisions(Some(agent_b), None, None, None, 100, 0)
        .await
        .expect("list_decisions should succeed");

    assert_eq!(decisions_for_b.len(), 1);
    assert_eq!(decisions_for_b[0].decision_type, "rejection");
}