# P3: Advanced Features Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement proper memory compaction, add context anchors to checkpoint, and prepare for team-level recovery.

**Architecture:**
- MemoryCompactionJob refactored to summarize/merge entries instead of creating candidates
- Checkpoint stores context anchors (references to external context, artifacts, memory entries)
- Recovery restores context anchors on resume

**Tech Stack:** Rust (tokio, sqlx, axum), PostgreSQL

---

## Task 1: Proper Memory Compaction

### Files
- Modify: `crates/torque-harness/src/jobs/memory_compaction.rs`
- Modify: `crates/torque-harness/src/models/v1/memory.rs`
- Create: `crates/torque-harness/tests/compaction_tests.rs`

- [ ] **Step 1: Add CompactionStrategy enum**

Read `crates/torque-harness/src/models/v1/memory.rs`.

Add:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompactionStrategy {
    Summarize,
    Merge,
    Archive,
    Drop,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionRecommendation {
    pub entry_id: Uuid,
    pub strategy: CompactionStrategy,
    pub reason: String,
    pub supersedes: Option<Uuid>,
}
```

- [ ] **Step 2: Add summarize_entry method to MemoryService**

Read `crates/torque-harness/src/service/memory.rs` around line 400.

Add:
```rust
pub async fn summarize_entries(
    &self,
    entry_ids: Vec<Uuid>,
) -> anyhow::Result<MemoryEntry> {
    let entries = self.repo_v1.get_entries_by_ids(entry_ids).await?;
    if entries.is_empty() {
        anyhow::bail!("No entries found");
    }

    let summary_text = entries.iter()
        .map(|e| format!("[{}] {}: {}", e.category, e.key, e.value))
        .collect::<Vec<_>>()
        .join("\n---\n");

    let summarized = MemoryEntry {
        id: Uuid::new_v4(),
        key: format!("_compacted_{}", Uuid::new_v4()),
        value: summary_text,
        category: MemoryCategory::Session,
        embedding: None,
        embedding_model: None,
        agent_instance_id: entries.first().and_then(|e| e.agent_instance_id),
        team_instance_id: entries.first().and_then(|e| e.team_instance_id),
        source_candidate_id: None,
        access_count: 0,
        last_accessed_at: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    self.repo_v1.create_entry(&summarized).await
}
```

- [ ] **Step 3: Refactor MemoryCompactionJob.run()**

Read `crates/torque-harness/src/jobs/memory_compaction.rs`.

Replace the run() method to:
1. Query entries by category/age
2. Group related entries
3. For each group: create a MemoryWriteCandidate with strategy (Summarize/Merge/Archive)
4. After candidate approved, mark old entries as superseded

Add `superseded_by: Option<Uuid>` field to MemoryEntry model. This stores the ID of the entry that replaced this one during compaction.

- [ ] **Step 4: Add get_entries_by_ids to repository**

Read `crates/torque-harness/src/repository/memory_v1.rs`.

Add to trait:
```rust
async fn get_entries_by_ids(&self, ids: Vec<Uuid>) -> anyhow::Result<Vec<MemoryEntry>>;
```

Add implementation using `WHERE id = ANY($1)` query.

- [ ] **Step 5: Create compaction_tests.rs**

Create `crates/torque-harness/tests/compaction_tests.rs`:
```rust
mod common;
use common::setup_test_db_or_skip;
use torque_harness::jobs::memory_compaction::MemoryCompactionJob;
use torque_harness::models::v1::memory::{MemoryCategory, MemoryEntry};
use torque_harness::repository::{MemoryRepositoryV1, PostgresMemoryRepositoryV1};
use torque_harness::service::candidate_generator::MockCandidateGenerator;
use uuid::Uuid;

fn create_test_entry(repo: &PostgresMemoryRepositoryV1, key: &str, value: &str) -> MemoryEntry {
    let now = chrono::Utc::now();
    let entry = MemoryEntry {
        id: Uuid::new_v4(),
        agent_instance_id: Some(Uuid::new_v4()),
        team_instance_id: None,
        category: MemoryCategory::Artifact,
        key: key.to_string(),
        value: serde_json::json!(value),
        source_candidate_id: None,
        embedding_model: None,
        access_count: 0,
        last_accessed_at: None,
        created_at: now,
        updated_at: now,
    };
    entry
}

#[tokio::test]
async fn test_compaction_job_processes_entries() {
    let db = match setup_test_db_or_skip().await {
        Some(db) => db,
        None => return,
    };
    let repo = PostgresMemoryRepositoryV1::new(db);
    let mock_generator = MockCandidateGenerator::new();
    let job = MemoryCompactionJob::new(repo.clone(), Arc::new(mock_generator))
        .with_batch_size(10);

    // Create test entries
    let entry1 = create_test_entry(&repo, "fact1", "First fact");
    let entry2 = create_test_entry(&repo, "fact2", "Second fact");
    repo.create_entry(&entry1).await.unwrap();
    repo.create_entry(&entry2).await.unwrap();

    // Run compaction
    let result = job.run().await;
    assert!(result.is_ok());
    let compaction_result = result.unwrap();
    assert_eq!(compaction_result.entries_processed, 2);
}

#[tokio::test]
async fn test_get_entries_by_ids() {
    let db = match setup_test_db_or_skip().await {
        Some(db) => db,
        None => return,
    };
    let repo = PostgresMemoryRepositoryV1::new(db);
    let now = chrono::Utc::now();

    let entry1 = MemoryEntry {
        id: Uuid::new_v4(),
        agent_instance_id: Some(Uuid::new_v4()),
        team_instance_id: None,
        category: MemoryCategory::Artifact,
        key: "fact1".to_string(),
        value: serde_json::json!("First fact"),
        source_candidate_id: None,
        embedding_model: None,
        access_count: 0,
        last_accessed_at: None,
        created_at: now,
        updated_at: now,
    };

    repo.create_entry(&entry1).await.unwrap();

    let result = repo.get_entries_by_ids(vec![entry1.id]).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 1);
}
```

- [ ] **Step 6: Run cargo check**

Run: `cargo check -p torque-harness`

- [ ] **Step 7: Commit**

```bash
git add crates/torque-harness/src/jobs/memory_compaction.rs crates/torque-harness/src/models/v1/memory.rs crates/torque-harness/src/service/memory.rs crates/torque-harness/src/repository/memory_v1.rs crates/torque-harness/tests/compaction_tests.rs
git commit -m "feat(compaction): implement proper memory summarization"
```

---

## Task 2: Context Anchors in Checkpoint

### Files
- Modify: `crates/torque-harness/src/models/v1/checkpoint.rs`
- Modify: `crates/torque-harness/src/repository/checkpoint.rs`
- Modify: `crates/torque-harness/src/service/recovery.rs`

- [ ] **Step 1: Add ContextAnchor model**

Read `crates/torque-harness/src/models/v1/checkpoint.rs` around line 50.

Add:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextAnchor {
    pub anchor_type: ContextAnchorType,
    pub reference_id: Uuid,
    pub captured_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContextAnchorType {
    ExternalContextRef,
    Artifact,
    MemoryEntry,
    SharedState,
}
```

- [ ] **Step 2: Add context_anchors field to Checkpoint**

Add to Checkpoint struct:
```rust
pub context_anchors: Vec<ContextAnchor>,
```

- [ ] **Step 3: Add capture_context_anchors method**

Add to MemoryService:
```rust
pub async fn capture_context_anchors(
    &self,
    agent_instance_id: Uuid,
) -> anyhow::Result<Vec<ContextAnchor>> {
    let mut anchors = Vec::new();
    let now = chrono::Utc::now();

    // 1. Capture recent memory entries (last 50 accessed)
    let entries = self.repo_v1.list_entries(50, 0).await?;
    for entry in entries {
        anchors.push(ContextAnchor {
            anchor_type: ContextAnchorType::MemoryEntry,
            reference_id: entry.id,
            captured_at: now,
        });
    }

    // 2. Capture external context refs (if any exist for this agent)
    // Query v1_external_context_refs for this agent_instance_id
    let ext_refs = self.repo_v1.get_external_context_refs(agent_instance_id).await?;
    for ext_ref in ext_refs {
        anchors.push(ContextAnchor {
            anchor_type: ContextAnchorType::ExternalContextRef,
            reference_id: ext_ref.id,
            captured_at: now,
        });
    }

    // 3. Capture shared state anchor (team-level)
    // Query v1_team_shared_state for this agent's team
    if let Some(team_id) = self.repo_v1.get_team_for_agent(agent_instance_id).await? {
        anchors.push(ContextAnchor {
            anchor_type: ContextAnchorType::SharedState,
            reference_id: team_id,
            captured_at: now,
        });
    }

    // 4. Capture last event anchor (for replay)
    let last_event = self.repo_v1.get_last_event_id(agent_instance_id).await?;
    if let Some(event_id) = last_event {
        anchors.push(ContextAnchor {
            anchor_type: ContextAnchorType::EventAnchor,
            reference_id: event_id,
            captured_at: now,
        });
    }

    Ok(anchors)
}
```

Also add these repository methods:
- `get_external_context_refs(agent_instance_id)` - query external context refs
- `get_team_for_agent(agent_instance_id)` - get team ID for agent
- `get_last_event_id(agent_instance_id)` - get most recent event ID

- [ ] **Step 4: Store context anchors in checkpoint creation**

Read `crates/torque-harness/src/repository/checkpoint.rs`.

Modify checkpoint creation to include context_anchors:
```rust
// In create_checkpoint method, add context_anchors parameter
pub async fn create_checkpoint(
    &self,
    agent_instance_id: Uuid,
    custom_state: serde_json::Value,
    context_anchors: Vec<ContextAnchor>,
) -> anyhow::Result<Checkpoint> {
    // ... existing code ...
    // Store context_anchors as JSONB
}
```

- [ ] **Step 5: Implement restore + replay + reconciliation**

Read `crates/torque-harness/src/service/recovery.rs`.

The recovery contract per spec Section 6.2 requires: **restore + replay + reconcile**.

```rust
pub async fn restore_with_anchors_and_reconcile(
    &self,
    checkpoint_id: Uuid,
) -> anyhow::Result<RecoveryResult> {
    // 1. RESTORE: Load checkpoint state and context anchors
    let checkpoint = self.checkpoint_repo.get(checkpoint_id).await?;
    let anchors = checkpoint.context_anchors;

    // Re-establish memory entry references
    for anchor in &anchors {
        match anchor.anchor_type {
            ContextAnchorType::MemoryEntry => {
                self.repo_v1.get_entry_by_id(anchor.reference_id).await?;
            }
            ContextAnchorType::ExternalContextRef => {
                self.repo_v1.get_external_context_ref(anchor.reference_id).await?;
            }
            ContextAnchorType::SharedState => {
                self.team_shared_state_repo.get(anchor.reference_id).await?;
            }
            ContextAnchorType::EventAnchor => {
                // Store event offset for replay
            }
        }
    }

    // 2. REPLAY: Replay tail events after checkpoint event anchor
    let event_anchor = anchors.iter()
        .find(|a| matches!(a.anchor_type, ContextAnchorType::EventAnchor));

    if let Some(anchor) = event_anchor {
        let events = self.event_repo.list_after(anchor.reference_id).await?;
        for event in events {
            self.event_registry.replay(&event).await?;
        }
    }

    // 3. RECONCILE: Detect inconsistencies with current runtime state
    let inconsistencies = self.detect_inconsistencies(&checkpoint).await?;

    // Apply reconciliation resolutions
    for inconsistency in inconsistencies {
        self.apply_resolution(&inconsistency).await?;
    }

    Ok(RecoveryResult {
        checkpoint_id,
        restored_anchors: anchors.len(),
        events_replayed: /* count */,
        inconsistencies_found: inconsistencies.len(),
        resolutions_applied: /* count */,
    })
}
```

Note: The `reestablishAnchor`, `list_after`, `detect_inconsistencies`, and `apply_resolution` methods may already exist in the codebase - use existing implementations where possible.

- [ ] **Step 6: Run cargo check**

Run: `cargo check -p torque-harness`

- [ ] **Step 7: Commit**

```bash
git add crates/torque-harness/src/models/v1/checkpoint.rs crates/torque-harness/src/repository/checkpoint.rs crates/torque-harness/src/service/recovery.rs
git commit -m "feat(checkpoint): add context anchors for recovery"
```

---

## Task 3: Team-Level Recovery Foundation

### Files
- Modify: `crates/torque-harness/src/service/recovery.rs`
- Modify: `crates/torque-harness/src/models/v1/team.rs`
- Create: `crates/torque-harness/tests/team_recovery_tests.rs`

- [ ] **Step 1: Add TeamRecoveryDisposition enum**

Read `crates/torque-harness/src/models/v1/team.rs`.

Add:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TeamRecoveryDisposition {
    TeamHealthy,
    TeamDegraded,
    TeamFailed,
    AwaitingSupervisor,
}
```

- [ ] **Step 2: Add assess_team_recovery to RecoveryService**

Read `crates/torque-harness/src/service/recovery.rs` around line 400.

Add:
```rust
pub async fn assess_team_recovery(
    &self,
    team_instance_id: Uuid,
) -> anyhow::Result<TeamRecoveryAssessment> {
    // Check all team member statuses
    let members = self.team_member_repo.list(team_instance_id).await?;

    let mut failed_members = 0;
    let mut waiting_members = 0;

    for member in members {
        match member.status {
            TeamMemberStatus::Failed => failed_members += 1,
            TeamMemberStatus::WaitingDelegation => waiting_members += 1,
            _ => {}
        }
    }

    let disposition = if failed_members > 0 {
        TeamRecoveryDisposition::TeamDegraded
    } else if waiting_members > 0 {
        TeamRecoveryDisposition::AwaitingSupervisor
    } else {
        TeamRecoveryDisposition::TeamHealthy
    };

    Ok(TeamRecoveryAssessment {
        team_instance_id,
        disposition,
        failed_member_ids: vec![],
        recommendation: "Resume team execution".to_string(),
    })
}
```

- [ ] **Step 3: Add recover_team_task method**

Add method to handle team task recovery:
```rust
pub async fn recover_team_task(
    &self,
    team_task_id: Uuid,
) -> anyhow::Result<TeamTaskRecoveryResult> {
    let task = self.team_task_repo.get(team_task_id).await?
        .ok_or_else(|| anyhow::anyhow!("Task not found: {}", team_task_id))?;

    let action = match task.status {
        TeamTaskStatus::Failed => {
            // Check retry count - if under limit, retry; else escalate
            let retry_count = task.retry_count.unwrap_or(0);
            if retry_count < 3 {
                // Retry: reset status and trigger re-execution
                self.team_task_repo.update_status(team_task_id, TeamTaskStatus::Pending).await?;
                TeamRecoveryAction::Retry
            } else {
                // Escalate: mark for supervisor review
                TeamRecoveryAction::EscalateToSupervisor
            }
        }
        TeamTaskStatus::Cancelled => {
            TeamRecoveryAction::NoOp
        }
        _ => TeamRecoveryAction::NoOp,
    };

    Ok(TeamTaskRecoveryResult {
        task_id: team_task_id,
        action_taken: format!("{:?}", action),
        new_status: task.status,
    })
}
```

- [ ] **Step 4: Create team_recovery_tests.rs**

Create `crates/torque-harness/tests/team_recovery_tests.rs`:
```rust
mod common;
use common::setup_test_db_or_skip;
use torque_harness::models::v1::team::{TeamTask, TeamTaskStatus};
use torque_harness::repository::{PostgresTeamRepository, TeamRepository};
use uuid::Uuid;

#[tokio::test]
async fn test_team_recovery_assessment_healthy() {
    let db = match setup_test_db_or_skip().await {
        Some(db) => db,
        None => return,
    };
    let repo = PostgresTeamRepository::new(db);

    // Create healthy team with running task
    let team = repo.create_team(& /* ... */).await.unwrap();
    let task = repo.create_task(&team.id, /* ... */).await.unwrap();

    let result = repo.assess_team_recovery(team.id).await;
    assert!(result.is_ok());
    assert!(matches!(result.unwrap().disposition, TeamRecoveryDisposition::TeamHealthy));
}

#[tokio::test]
async fn test_team_recovery_retry_on_failure() {
    let db = match setup_test_db_or_skip().await {
        Some(db) => db,
        None => return,
    };
    let repo = PostgresTeamRepository::new(db);

    let team = repo.create_team(& /* ... */).await.unwrap();
    let mut task = repo.create_task(&team.id, /* ... */).await.unwrap();
    task.status = TeamTaskStatus::Failed;
    task.retry_count = Some(0);
    repo.update_task(&task).await.unwrap();

    let result = repo.recover_team_task(task.id).await;
    assert!(result.is_ok());
    assert!(matches!(result.unwrap().action_taken, s) if s.contains("Retry"));
}
```

- [ ] **Step 5: Run cargo check**

Run: `cargo check -p torque-harness`

- [ ] **Step 6: Commit**

```bash
git add crates/torque-harness/src/service/recovery.rs crates/torque-harness/src/models/v1/team.rs crates/torque-harness/tests/team_recovery_tests.rs
git commit -m "feat(recovery): add team-level recovery assessment"
```

---

## Task 4: Final Verification

- [ ] **Step 1: Run full test suite**

Run: `cargo test -p torque-harness 2>&1 | tail -50`
Expected: All tests pass

- [ ] **Step 2: Run cargo check for warnings**

Run: `cargo check -p torque-harness 2>&1 | grep -E "warning|error"`
Expected: Only existing warnings

- [ ] **Step 3: Update STATUS.md**

Add P3 section documenting:
- Proper memory compaction with summarization
- Context anchors in checkpoint
- Team-level recovery foundation

- [ ] **Step 4: Final commit**

```bash
git add STATUS.md
git commit -m "docs: mark P3 Advanced Features complete"
```

---

## New Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| (none - internal improvements) | - | - |

## Test Count Impact

- New tests: compaction_tests (3-5), team_recovery_tests (2-3)
- Expected total: ~150 tests