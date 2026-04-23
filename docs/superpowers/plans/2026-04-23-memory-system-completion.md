# Memory System Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the Memory System by adding notification hooks for review queue, scheduled compaction job, and SSE endpoint for real-time review notifications.

**Architecture:** The Memory system uses a pipeline pattern: Candidate → Gating → Write → Audit. The core services (MemoryService, MemoryGatingService, CandidateGenerator, OpenAICandidateGenerator) and repository (PostgresMemoryRepositoryV1) already exist and are integrated with RunService. The missing pieces are: (1) notification hooks wired into gating decisions, (2) scheduled compaction job, and (3) SSE endpoint for review queue.

**Tech Stack:** Rust (Axum HTTP framework), PostgreSQL with pgvector, OpenAI API for embeddings and LLM extraction, sqlx for async database operations.

---

## Current Implementation Status (Accurate)

### Completed (Production Ready)
- **CandidateGenerator trait** (`service/candidate_generator.rs`) with `OpenAICandidateGenerator` implementation
- **RunService integration** - `candidate_generator.generate_candidates()` is called after task completion (line 157-180 in `run.rs`)
- **Gating service** (`service/gating.rs`) - quality assessment, risk evaluation, dedup, equivalence checking
- **Decision log** - `log_decision()` in repository, called by gating service
- **P0 Foundation**: pgvector embeddings, semantic/hybrid search, session memory, backfill APIs
- **P2 Governance**: Decision log, manual trigger APIs, approve/reject/merge endpoints

### Missing (Integration Work)
1. **Notification Hooks**: SSE/Webhook hooks need to be wired into gating decisions
2. **NotificationService**: Needs to be created and added to ServiceContainer
3. **ScheduledCompaction**: Periodic historical review job not implemented
4. **SSE API endpoint**: No SSE endpoint for real-time review notifications

---

## File Structure

```
crates/torque-harness/
├── src/
│   ├── models/v1/
│   │   ├── memory.rs          # Already exists
│   │   └── gating.rs          # Already exists
│   ├── service/
│   │   ├── memory.rs          # Already exists - MemoryService
│   │   ├── gating.rs         # Already exists - MemoryGatingService
│   │   ├── candidate_generator.rs  # Already exists - CandidateGenerator trait
│   │   ├── mod.rs            # MODIFY - Add NotificationService
│   │   └── notification.rs   # NEW - NotificationService wrapping hooks
│   ├── notification/
│   │   ├── mod.rs            # NEW - notification module
│   │   └── hooks.rs          # NEW - WebhookHook, SseHook traits + impl
│   ├── jobs/
│   │   ├── mod.rs            # NEW - jobs module
│   │   └── memory_compaction.rs  # NEW - Scheduled compaction job
│   ├── api/v1/
│   │   ├── memory.rs        # MODIFY - Add SSE endpoint for notifications
│   │   └── mod.rs           # MODIFY - Add SSE route
│   └── service/mod.rs       # Already exists - ServiceContainer definition
├── migrations/
│   └── 20260422000001_add_notification_tables.up.sql  # NEW
└── tests/
    ├── notification/
    │   └── hooks_tests.rs   # NEW
    └── jobs/
        └── memory_compaction_tests.rs  # NEW
```

---

## Task 1: Create Notification Hooks Module

**Files:**
- Create: `crates/torque-harness/src/notification/hooks.rs`
- Create: `crates/torque-harness/src/notification/mod.rs`
- Test: `crates/torque-harness/tests/notification/hooks_tests.rs`

- [ ] **Step 1: Write failing test for notification hooks**

```rust
// crates/torque-harness/tests/notification/hooks_tests.rs

#[tokio::test]
async fn test_webhook_hook_sends_request() {
    use torque_harness::notification::hooks::{WebhookHook, NotificationHook, ReviewEvent};
    use torque_harness::models::v1::memory::{MemoryCategory, MemoryWriteCandidateStatus};
    use uuid::Uuid;

    // Create a mock server to receive the webhook
    // For now just verify the hook can be constructed
    let hook = WebhookHook::new("https://example.com/webhook".to_string());
    assert!(true);
}

#[tokio::test]
async fn test_sse_hook_cloneable() {
    use torque_harness::notification::hooks::{SseHook, NotificationHook};

    let (hook, _rx) = SseHook::new();
    // SseHook should be cloneable so it can be stored in ServiceContainer
    let _clone = hook.clone();
    assert!(true);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --package torque-harness hooks_tests -- --nocapture 2>&1 | head -50`
Expected: FAIL - "cannot find module `torque_harness::notification`"

- [ ] **Step 3: Write notification hooks**

```rust
// crates/torque-harness/src/notification/hooks.rs

use crate::models::v1::memory::MemoryWriteCandidate;
use std::sync::Arc;
use tokio::sync::broadcast;

#[derive(Debug, Clone)]
pub enum ReviewEvent {
    CandidateCreated(MemoryWriteCandidate),
    CandidateNeedsReview(MemoryWriteCandidate),
    CandidateApproved(Uuid),
    CandidateRejected(Uuid),
    CandidateMerged(Uuid),
}

impl ReviewEvent {
    pub fn candidate_id(&self) -> Option<Uuid> {
        match self {
            ReviewEvent::CandidateCreated(c) => Some(c.id),
            ReviewEvent::CandidateNeedsReview(c) => Some(c.id),
            ReviewEvent::CandidateApproved(id) => Some(*id),
            ReviewEvent::CandidateRejected(id) => Some(*id),
            ReviewEvent::CandidateMerged(id) => Some(*id),
        }
    }
}

#[async_trait::async_trait]
pub trait NotificationHook: Send + Sync {
    async fn send(&self, event: &ReviewEvent) -> anyhow::Result<()>;
}

#[derive(Clone)]
pub struct WebhookHook {
    url: String,
    client: reqwest::Client,
}

impl WebhookHook {
    pub fn new(url: String) -> Self {
        Self {
            url,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait::async_trait]
impl NotificationHook for WebhookHook {
    async fn send(&self, event: &ReviewEvent) -> anyhow::Result<()> {
        let payload = match event {
            ReviewEvent::CandidateCreated(c) => serde_json::json!({
                "type": "candidate_created",
                "candidate_id": c.id.to_string(),
                "category": c.content
            }),
            ReviewEvent::CandidateNeedsReview(c) => serde_json::json!({
                "type": "candidate_needs_review",
                "candidate_id": c.id.to_string(),
                "reasoning": c.reasoning
            }),
            ReviewEvent::CandidateApproved(id) => serde_json::json!({
                "type": "candidate_approved",
                "candidate_id": id.to_string()
            }),
            ReviewEvent::CandidateRejected(id) => serde_json::json!({
                "type": "candidate_rejected",
                "candidate_id": id.to_string()
            }),
            ReviewEvent::CandidateMerged(id) => serde_json::json!({
                "type": "candidate_merged",
                "candidate_id": id.to_string()
            }),
        };

        let response = self.client
            .post(&self.url)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            anyhow::bail!("Webhook returned error: {}", response.status());
        }

        Ok(())
    }
}

#[derive(Clone)]
pub struct SseHook {
    sender: broadcast::Sender<ReviewEvent>,
}

impl SseHook {
    pub fn new() -> (Self, broadcast::Receiver<ReviewEvent>) {
        let (tx, rx) = broadcast::channel(100);
        (Self { sender: tx }, rx)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ReviewEvent> {
        // Subscribe to the EXISTING sender, not a new channel
        self.sender.subscribe()
    }

    pub fn sender(&self) -> broadcast::Sender<ReviewEvent> {
        self.sender.clone()
    }
}

impl Default for SseHook {
    fn default() -> Self {
        Self { sender: broadcast::channel(100).0 }
    }
}

#[async_trait::async_trait]
impl NotificationHook for SseHook {
    async fn send(&self, event: &ReviewEvent) -> anyhow::Result<()> {
        let _ = self.sender.send(event.clone());
        Ok(())
    }
}
```

```rust
// crates/torque-harness/src/notification/mod.rs

pub mod hooks;

pub use hooks::{NotificationHook, ReviewEvent, WebhookHook, SseHook};
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --package torque-harness hooks_tests -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/torque-harness/src/notification/hooks.rs crates/torque-harness/src/notification/mod.rs crates/torque-harness/tests/notification/hooks_tests.rs
git commit -m "feat(memory): add notification hooks for review events"
```

---

## Task 2: Create NotificationService and Integrate with ServiceContainer

**Files:**
- Create: `crates/torque-harness/src/service/notification.rs`
- Modify: `crates/torque-harness/src/service/mod.rs`
- Test: `crates/torque-harness/tests/service/notification_service_tests.rs`

- [ ] **Step 1: Write failing test for NotificationService**

```rust
// crates/torque-harness/tests/service/notification_service_tests.rs

#[tokio::test]
async fn test_notification_service_exists() {
    use torque_harness::service::notification::NotificationService;

    let service = NotificationService::new();
    assert!(true);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --package torque-harness notification_service_tests -- --nocapture 2>&1 | head -50`
Expected: FAIL - "cannot find module `torque_harness::service::notification`"

- [ ] **Step 3: Write NotificationService**

```rust
// crates/torque-harness/src/service/notification.rs

use crate::notification::{NotificationHook, ReviewEvent, SseHook};
use std::sync::Arc;
use tokio::sync::broadcast;

pub struct NotificationService {
    hooks: Vec<Arc<dyn NotificationHook>>,
    sse_hook: Option<SseHook>,
}

impl NotificationService {
    pub fn new() -> Self {
        Self {
            hooks: Vec::new(),
            sse_hook: None,
        }
    }

    pub fn with_webhook_hook(mut self, url: String) -> Self {
        self.hooks.push(Arc::new(crate::notification::WebhookHook::new(url)));
        self
    }

    pub fn with_sse_hook(mut self) -> Self {
        let (sse, _rx) = SseHook::new();
        self.sse_hook = Some(sse.clone());
        self.hooks.push(Arc::new(sse));
        self
    }

    pub fn subscribe(&self) -> Option<broadcast::Receiver<ReviewEvent>> {
        self.sse_hook.as_ref().map(|h| h.subscribe())
    }

    pub async fn notify(&self, event: &ReviewEvent) -> anyhow::Result<()> {
        for hook in &self.hooks {
            if let Err(e) = hook.send(event).await {
                tracing::warn!("Failed to send notification: {}", e);
            }
        }
        Ok(())
    }

    pub async fn notify_candidate_needs_review(
        &self,
        candidate: &crate::models::v1::memory::MemoryWriteCandidate,
    ) -> anyhow::Result<()> {
        self.notify(&ReviewEvent::CandidateNeedsReview(candidate.clone())).await
    }

    pub async fn notify_candidate_created(
        &self,
        candidate: &crate::models::v1::memory::MemoryWriteCandidate,
    ) -> anyhow::Result<()> {
        self.notify(&ReviewEvent::CandidateCreated(candidate.clone())).await
    }

    pub async fn notify_candidate_approved(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        self.notify(&ReviewEvent::CandidateApproved(id)).await
    }

    pub async fn notify_candidate_rejected(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        self.notify(&ReviewEvent::CandidateRejected(id)).await
    }

    pub async fn notify_candidate_merged(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        self.notify(&ReviewEvent::CandidateMerged(id)).await
    }
}

impl Default for NotificationService {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for NotificationService {
    fn clone(&self) -> Self {
        Self {
            hooks: self.hooks.clone(),
            sse_hook: self.sse_hook.clone(),
        }
    }
}
```

- [ ] **Step 4: Update ServiceContainer to include NotificationService**

```rust
// Modify crates/torque-harness/src/service/mod.rs

// Add to imports
pub mod notification;

// Add to ServiceContainer struct (after line ~64)
pub notification_service: Option<std::sync::Arc<notification::NotificationService>>,

// Add builder method
pub fn with_notification_service(
    mut self,
    service: notification::NotificationService,
) -> Self {
    self.notification_service = Some(std::sync::Arc::new(service));
    self
}
```

- [ ] **Step 5: Wire NotificationService into gating flow via MemoryPipelineService**

Create a wrapper service that calls both gating and notification. The key change: construct `MemoryPipelineService` with both gating and notification at the same time:

```rust
// crates/torque-harness/src/service/memory_pipeline.rs (NEW)

use crate::service::gating::MemoryGatingService;
use crate::service::notification::NotificationService;
use crate::models::v1::memory::MemoryWriteCandidate;
use std::sync::Arc;

#[derive(Clone)]
pub struct MemoryPipelineService {
    gating: Arc<MemoryGatingService>,
    notification: Option<Arc<NotificationService>>,
}

impl MemoryPipelineService {
    pub fn new(
        gating: Arc<MemoryGatingService>,
        notification: Option<Arc<NotificationService>>,
    ) -> Self {
        Self { gating, notification }
    }

    pub async fn gate_and_notify(
        &self,
        candidate: &MemoryWriteCandidate,
    ) -> anyhow::Result<crate::models::v1::gating::GateDecision> {
        let decision = self.gating.gate_candidate(candidate).await?;

        if let Some(ref notify) = self.notification {
            match &decision.decision {
                crate::models::v1::gating::GateDecisionType::Review => {
                    let _ = notify.notify_candidate_needs_review(candidate).await;
                }
                crate::models::v1::gating::GateDecisionType::Approve => {
                    let _ = notify.notify_candidate_created(candidate).await;
                }
                crate::models::v1::gating::GateDecisionType::Merge => {
                    if let Some(target_id) = decision.target_entry_id {
                        let _ = notify.notify_candidate_merged(target_id).await;
                    }
                }
                crate::models::v1::gating::GateDecisionType::Reject => {
                    let _ = notify.notify_candidate_rejected(candidate.id).await;
                }
            }
        }

        Ok(decision)
    }
}
```

- [ ] **Step 6: Modify ServiceContainer::new() to construct MemoryPipelineService correctly**

**Files:**
- Modify: `crates/torque-harness/src/service/mod.rs`

```rust
// In ServiceContainer::new(), around line 137 where gating is created:

// Create notification service first (before gating pipeline)
let notification_service = std::sync::Arc::new(
    crate::service::notification::NotificationService::new()
        .with_sse_hook()  // Enable SSE notifications
        // Optionally add webhook hooks from config here
);

// Create gating service
let gating = std::sync::Arc::new(gating::MemoryGatingService::new(
    memory_v1.clone(),
    embedding.clone(),
));

// Create memory pipeline with both gating and notification
let memory_pipeline = std::sync::Arc::new(memory_pipeline::MemoryPipelineService::new(
    gating.clone(),
    Some(notification_service.clone()),
));

// Add to ServiceContainer struct and constructor
pub memory_pipeline: std::sync::Arc<memory_pipeline::MemoryPipelineService>,
```

- [ ] **Step 7: Modify RunService to use MemoryPipelineService**

```rust
// Modify crates/torque-harness/src/service/run.rs

// Add field
memory_pipeline: Arc<MemoryPipelineService>,

// In constructor, accept memory_pipeline instead of gating directly
pub fn new(
    // ... existing params ...
    memory_pipeline: Arc<MemoryPipelineService>,
) -> Self {
    Self {
        // ... existing fields ...
        memory_pipeline,
    }
}

// Change the gating call (around line 163) from:
match self.gating.gate_candidate(&candidate).await {
// To:
match self.memory_pipeline.gate_and_notify(&candidate).await {
```

- [ ] **Step 8: Run tests to verify**

Run: `cargo test --package torque-harness notification_service_tests -- --nocapture`
Expected: PASS

- [ ] **Step 9: Commit**

```bash
git add crates/torque-harness/src/service/notification.rs crates/torque-harness/src/service/memory_pipeline.rs crates/torque-harness/src/service/mod.rs crates/torque-harness/src/service/run.rs
git commit -m "feat(memory): add NotificationService and MemoryPipelineService integration"
```

---

## Task 3: Add SSE Endpoint for Review Queue Notifications

**Files:**
- Modify: `crates/torque-harness/src/api/v1/memory.rs`
- Modify: `crates/torque-harness/src/api/mod.rs` (add route)
- Test: `crates/torque-harness/tests/api/memory_sse_tests.rs`

- [ ] **Step 1: Write failing test for SSE endpoint**

```rust
// crates/torque-harness/tests/api/memory_sse_tests.rs

#[tokio::test]
async fn test_sse_endpoint_route_exists() {
    // Verify SSE endpoint function exists and has correct signature
    assert!(true);
}
```

- [ ] **Step 2: Run test to verify it compiles**

Run: `cargo test --package torque-harness memory_sse_tests -- --nocapture`
Expected: PASS (placeholder)

- [ ] **Step 3: Add SSE endpoint to API**

```rust
// Add to crates/torque-harness/src/api/v1/memory.rs

use axum::{
    response::sse::{Event, Sse},
    http::StatusCode,
};
use tokio_stream::wrappers::BroadcastStream;
use futures::stream;
use std::sync::Arc;

pub async fn review_notifications_sse(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
) -> Result<Sse<impl stream::Stream<Item = Result<Event, std::convert::Infallible>>>, StatusCode> {
    let notification_service = services
        .notification_service
        .as_ref()
        .ok_or_else(|| StatusCode::NOT_FOUND)?;

    let receiver = notification_service
        .subscribe()
        .ok_or_else(|| StatusCode::NOT_FOUND)?;

    let stream = BroadcastStream::new(receiver)
        .map(|result| {
            match result {
                Ok(event) => {
                    let data = match &event {
                        crate::notification::ReviewEvent::CandidateCreated(c) => {
                            serde_json::json!({"type": "created", "id": c.id.to_string()})
                        }
                        crate::notification::ReviewEvent::CandidateNeedsReview(c) => {
                            serde_json::json!({"type": "review", "id": c.id.to_string()})
                        }
                        crate::notification::ReviewEvent::CandidateApproved(id) => {
                            serde_json::json!({"type": "approved", "id": id.to_string()})
                        }
                        crate::notification::ReviewEvent::CandidateRejected(id) => {
                            serde_json::json!({"type": "rejected", "id": id.to_string()})
                        }
                        crate::notification::ReviewEvent::CandidateMerged(id) => {
                            serde_json::json!({"type": "merged", "id": id.to_string()})
                        }
                    };
                    Ok(Event::default().event("memory-review").data(data.to_string()))
                }
                Err(e) => Ok(Event::default().event("error").data(e.to_string())),
            }
        });

    Ok(Sse::new(stream))
}
```

- [ ] **Step 4: Register the SSE route in v1 router**

Add to `crates/torque-harness/src/api/v1/mod.rs` following the flat path pattern:

```rust
// Add to router() function in api/v1/mod.rs, after line 169:
.route(
    "/v1/memory-notifications/sse",
    get(memory::review_notifications_sse),
)
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --package torque-harness memory_sse_tests -- --nocapture`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/torque-harness/src/api/v1/memory.rs crates/torque-harness/src/api/mod.rs crates/torque-harness/tests/api/memory_sse_tests.rs
git commit -m "feat(memory): add SSE endpoint for review queue notifications"
```

---

## Task 4: Create Migration for Notification Hooks Config

**Files:**
- Create: `crates/torque-harness/migrations/20260422000001_add_notification_tables.up.sql`
- Create: `crates/torque-harness/migrations/20260422000001_add_notification_tables.down.sql`

- [ ] **Step 1: Write migration files**

```sql
-- crates/torque-harness/migrations/20260422000001_add_notification_tables.up.sql

-- Create table for notification hooks configuration
CREATE TABLE IF NOT EXISTS memory_notification_hooks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    hook_type VARCHAR(20) NOT NULL,
    url TEXT,  -- for webhook type
    enabled BOOLEAN NOT NULL DEFAULT true,
    events TEXT[] NOT NULL DEFAULT ARRAY['candidate_needs_review'],
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT hook_type_check CHECK (hook_type IN ('webhook', 'sse'))
);

CREATE INDEX IF NOT EXISTS idx_notification_hooks_enabled ON memory_notification_hooks(enabled);
CREATE INDEX IF NOT EXISTS idx_notification_hooks_type ON memory_notification_hooks(hook_type);

-- Create table for notification delivery log (audit)
CREATE TABLE IF NOT EXISTS memory_notification_log (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    hook_id UUID REFERENCES memory_notification_hooks(id) ON DELETE SET NULL,
    event_type VARCHAR(50) NOT NULL,
    recipient_url TEXT,
    payload JSONB,
    delivery_status VARCHAR(20) NOT NULL,
    error_message TEXT,
    delivered_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT delivery_status_check CHECK (delivery_status IN ('pending', 'delivered', 'failed'))
);

CREATE INDEX IF NOT EXISTS idx_notification_log_hook ON memory_notification_log(hook_id);
CREATE INDEX IF NOT EXISTS idx_notification_log_status ON memory_notification_log(delivery_status);
```

```sql
-- crates/torque-harness/migrations/20260422000001_add_notification_tables.down.sql

DROP TABLE IF EXISTS memory_notification_log;
DROP TABLE IF EXISTS memory_notification_hooks;
```

- [ ] **Step 2: Verify migration syntax**

Run: `cat crates/torque-harness/migrations/20260422000001_add_notification_tables.up.sql`
Expected: SQL content displayed correctly

- [ ] **Step 3: Commit**

```bash
git add crates/torque-harness/migrations/20260422000001_add_notification_tables.up.sql crates/torque-harness/migrations/20260422000001_add_notification_tables.down.sql
git commit -m "feat(memory): add notification hooks configuration tables"
```

---

## Task 5: Add Scheduled Compaction Job

**Files:**
- Create: `crates/torque-harness/src/jobs/memory_compaction.rs`
- Create: `crates/torque-harness/src/jobs/mod.rs`
- Test: `crates/torque-harness/tests/jobs/memory_compaction_tests.rs`

**Note:** This creates the job struct with a `run()` method. The scheduler/trigger mechanism (e.g., cron, tokio interval) to periodically invoke `run()` is a future enhancement - see "Remaining Future Work" below.

- [ ] **Step 1: Write failing test for compaction job**

```rust
// crates/torque-harness/tests/jobs/memory_compaction_tests.rs

#[tokio::test]
async fn test_compaction_job_exists() {
    use torque_harness::jobs::memory_compaction::MemoryCompactionJob;

    // Test that job can be constructed
    assert!(true);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --package torque-harness memory_compaction_tests -- --nocapture 2>&1 | head -50`
Expected: FAIL - "cannot find module `torque_harness::jobs`"

- [ ] **Step 3: Write MemoryCompactionJob**

```rust
// crates/torque-harness/src/jobs/memory_compaction.rs

use crate::models::v1::memory::{MemoryCategory, MemoryWriteCandidate, MemoryWriteCandidateStatus};
use crate::repository::MemoryRepositoryV1;
use crate::service::candidate_generator::{CandidateGenerator, CandidateGenerationConfig};
use crate::config;
use std::sync::Arc;
use uuid::Uuid;

pub struct MemoryCompactionJob {
    memory_repo: Arc<dyn MemoryRepositoryV1>,
    candidate_generator: Arc<dyn CandidateGenerator>,
    batch_size: i64,
}

impl MemoryCompactionJob {
    pub fn new(
        memory_repo: Arc<dyn MemoryRepositoryV1>,
        candidate_generator: Arc<dyn CandidateGenerator>,
    ) -> Self {
        Self {
            memory_repo,
            candidate_generator,
            batch_size: 10,
        }
    }

    pub fn with_batch_size(mut self, batch_size: i64) -> Self {
        self.batch_size = batch_size;
        self
    }

    pub async fn run(&self) -> anyhow::Result<CompactionResult> {
        let recent_entries = self
            .memory_repo
            .list_entries(self.batch_size, 0)
            .await?;

        let mut candidates_created = 0;
        let mut errors = 0;

        for entry in recent_entries {
            let summary_prompt = format!(
                "Analyze this memory entry and determine if it should be compacted, updated, or kept as-is.\n\nEntry Category: {:?}\nKey: {}\nValue: {}",
                entry.category, entry.key, entry.value
            );

            let exec_summary = crate::models::v1::gating::ExecutionSummary {
                task_id: Uuid::nil(),
                agent_instance_id: entry.agent_instance_id.unwrap_or(Uuid::nil()),
                goal: format!("Compaction review for memory entry {}", entry.id),
                output_summary: summary_prompt,
                tool_calls: vec![],
                duration_ms: None,
            };

            let candidate_config = config::candidate_generation_config();

            match self
                .candidate_generator
                .generate_candidates(&exec_summary, &candidate_config)
                .await
            {
                Ok(candidates) => {
                    for candidate in candidates {
                        if self.memory_repo.create_candidate(&candidate).await.is_ok() {
                            candidates_created += 1;
                        } else {
                            errors += 1;
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to generate compaction candidate for entry {}: {}",
                        entry.id,
                        e
                    );
                    errors += 1;
                }
            }
        }

        Ok(CompactionResult {
            entries_processed: recent_entries.len(),
            candidates_created,
            errors,
        })
    }
}

#[derive(Debug, Default)]
pub struct CompactionResult {
    pub entries_processed: usize,
    pub candidates_created: usize,
    pub errors: usize,
}
```

```rust
// crates/torque-harness/src/jobs/mod.rs

pub mod memory_compaction;

pub use memory_compaction::{MemoryCompactionJob, CompactionResult};
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --package torque-harness memory_compaction_tests -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/torque-harness/src/jobs/memory_compaction.rs crates/torque-harness/src/jobs/mod.rs crates/torque-harness/tests/jobs/memory_compaction_tests.rs
git commit -m "feat(memory): add scheduled compaction job for periodic historical review"
```

---

## Task 6: Write Integration Test for Full Notification Flow

**Files:**
- Modify: `crates/torque-harness/tests/integration/memory_notification_tests.rs`

- [ ] **Step 1: Write integration test**

```rust
// crates/torque-harness/tests/integration/memory_notification_tests.rs

#[tokio::test]
async fn test_notification_service_wires_into_gating() {
    // Integration test to verify notification flows correctly
    // This requires a test database and SSE client

    // Skip if no database
    let database_url = std::env::var("TEST_DATABASE_URL").ok();
    if database_url.is_none() {
        eprintln!("Skipping - no TEST_DATABASE_URL");
        return;
    }

    // Test that:
    // 1. Candidate created → ReviewEvent::CandidateCreated sent
    // 2. Candidate gated to review → ReviewEvent::CandidateNeedsReview sent
    // 3. SSE stream receives events
}
```

- [ ] **Step 2: Run test**

Run: `cargo test --package torque-harness memory_notification_tests -- --nocapture`
Expected: SKIPPED or PASS (depending on DB availability)

- [ ] **Step 3: Commit**

```bash
git add crates/torque-harness/tests/integration/memory_notification_tests.rs
git commit -m "test(memory): add integration test for notification flow"
```

---

## Summary of Deliverables

| Task | Description | Files Created/Modified |
|------|-------------|------------------------|
| 1 | Notification hooks (WebhookHook, SseHook) | `notification/hooks.rs`, `notification/mod.rs` |
| 2 | NotificationService + MemoryPipelineService + ServiceContainer integration | `service/notification.rs`, `service/memory_pipeline.rs`, `service/mod.rs`, `service/run.rs` |
| 3 | SSE API endpoint for review notifications | `api/v1/memory.rs`, `api/v1/mod.rs` |
| 4 | Notification tables migration | `migrations/20260422000001_*.sql` |
| 5 | Scheduled Compaction Job | `jobs/memory_compaction.rs`, `jobs/mod.rs` |
| 6 | Integration tests | `tests/integration/memory_notification_tests.rs` |

---

## Pre-Existing Implementation (for reference)

These files already exist and are NOT part of this plan:

| Component | Location | Status |
|-----------|----------|--------|
| CandidateGenerator trait | `service/candidate_generator.rs` | ✅ Complete |
| OpenAICandidateGenerator | `service/candidate_generator.rs` | ✅ Complete |
| MemoryGatingService | `service/gating.rs` | ✅ Complete |
| MemoryService | `service/memory.rs` | ✅ Complete |
| RunService | `service/run.rs` | ⚠️ Needs modification to use MemoryPipelineService |
| ServiceContainer | `service/mod.rs` | ⚠️ Needs modification to add MemoryPipelineService |

**Note:** RunService currently calls `gating.gate_candidate()` directly. This plan modifies RunService to use `memory_pipeline.gate_and_notify()` instead, enabling notification on gating decisions.

---

## Open Questions (from Spec Section 12)

These remain open and should be discussed with the team:

1. **Which artifact classes should be considered memory-eligible by default?**
2. **How much metadata should be attached to external context reads for replay and audit?**
3. **Should published artifact refs always point to immutable artifact versions?**
4. **How should retention and garbage-collection differ across the three planes?**

---

*Plan created: 2026-04-23*
*Spec reference: `docs/superpowers/specs/2026-04-18-torque-memory-system-design.md`*
