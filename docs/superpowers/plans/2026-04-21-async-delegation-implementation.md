# Async Delegation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement event-driven async delegation with layered timeouts, dynamic budget, circuit breaker, and two-layer queue structure (shared pool + individual queues).

**Architecture:** Supervisor creates delegations that flow through Redis Streams (shared pool + individual queues). Members poll streams, process via LLM, then call REST API to complete/fail. Supervisor waits via Redis Stream subscription with soft/hard timeout fallback. Three-layer budget isolation (User Request → Task → Delegation) with no borrowing across layers.

**Tech Stack:** Rust, sqlx, tokio, redis-rs, axum, llm crate

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                    Redis Streams                                 │
├─────────────────────────────────────────────────────────────────┤
│  team:{team_id}:tasks:shared      # Shared pool (work-stealing) │
│  member:{member_id}:tasks         # Individual queue           │
│  delegation:{delegation_id}:status # Status tracking            │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│  Supervisor                    │  MemberAgent                   │
│  ├── EventListener            │  ├── poll_tasks()               │
│  ├── CircuitBreaker          │  ├── accept_task()             │
│  ├── BudgetManager           │  ├── complete_task()           │
│  └── wait_for_completion()   │  └── fail_task()               │
└─────────────────────────────────────────────────────────────────┘
```

## Layered Timeouts

| Layer | Hard Timeout | Soft Timeout (110%) |
|-------|-------------|---------------------|
| User Request | 30s | 33s |
| Task Budget | 25.5s (85%) | 28s |
| Delegation | 8s (cold start default) | 8.8s |

## Budget Isolation

| Layer | Initial Budget | Borrowing | Degradation |
|-------|--------------|-----------|--------------|
| User Request | 30s | N/A | TIMEOUT_PARTIAL |
| Task | 5 retries | N/A | Return partial results |
| Delegation | 1 (no retry) | N/A | Fail → new delegation |

---

## File Structure

```
crates/torque-harness/src/
├── models/v1/
│   ├── delegation.rs              # Extended with status, rejection_reason
│   ├── delegation_event.rs        # NEW: DelegationEvent enum
│   ├── partial_quality.rs        # NEW: Multi-dimensional partial quality
│   └── team.rs                   # Extended with new event types
├── message_bus/
│   ├── mod.rs                    # NEW: StreamBus trait
│   ├── keys.rs                   # NEW: Stream key constants
│   ├── redis_streams.rs           # NEW: Redis implementation
│   ├── delegation_publisher.rs    # NEW: Publish to streams
│   ├── delegation_subscriber.rs   # NEW: Subscribe from streams
│   └── consumer_group.rs         # NEW: Consumer group management
├── service/team/
│   ├── member_agent.rs           # NEW: MemberAgent trait
│   ├── local_member_agent.rs     # NEW: Local implementation
│   ├── circuit_breaker.rs        # NEW: Circuit breaker per member
│   ├── member_health.rs         # NEW: MemberHealthTracker
│   ├── retry.rs                 # NEW: RetryStrategy, BudgetManager
│   ├── event_listener.rs         # NEW: EventListener trait + Redis impl
│   ├── supervisor.rs             # MODIFIED: async wait methods
│   └── modes.rs                 # MODIFIED: async handlers
├── service/
│   └── delegation.rs             # MODIFIED: complete/fail + events
├── api/v1/
│   ├── delegations.rs            # MODIFIED: add complete/fail endpoints
│   └── mod.rs                   # MODIFIED: route registration
└── config/
    └── task_budget.rs           # NEW: Task type → budget mapping
```

---

## Phase 0: Interface Definitions

### Task 0.1: Create MemberAgent Trait

**Files:**
- Create: `crates/torque-harness/src/service/team/member_agent.rs`
- Test: `crates/torque-harness/tests/member_agent_tests.rs`

- [ ] **Step 1: Write failing test for MemberAgent trait existence**

```rust
// crates/torque-harness/tests/member_agent_tests.rs
use torque_harness::service::team::MemberAgent;

#[tokio::test]
async fn test_member_agent_trait_exists() {
    // Verify trait is defined and has required methods
    fn assert_member_agent<T: MemberAgent>() {}
    assert_member_agent::<LocalMemberAgent>();
}
```

Run: `cargo test member_agent_tests::test_member_agent_trait_exists -p torque-harness`
Expected: FAIL - trait not found

- [ ] **Step 2: Define MemberAgent trait with core methods**

```rust
// crates/torque-harness/src/service/team/member_agent.rs
use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait MemberAgent: Send + Sync {
    async fn start(&self) -> anyhow::Result<()>;
    async fn stop(&self) -> anyhow::Result<()>;
    async fn poll_tasks(&self) -> anyhow::Result<Vec<MemberTask>>;
    async fn accept_task(&self, delegation_id: Uuid) -> anyhow::Result<()>;
    async fn complete_task(&self, delegation_id: Uuid, artifact_id: Uuid) -> anyhow::Result<()>;
    async fn fail_task(&self, delegation_id: Uuid, error: &str) -> anyhow::Result<()>;
    async fn request_extension(&self, delegation_id: Uuid, seconds: u32, reason: &str) -> anyhow::Result<bool>;
    async fn health_check(&self) -> anyhow::Result<MemberHealth>;
}

#[derive(Debug, Clone)]
pub struct MemberTask {
    pub delegation_id: Uuid,
    pub task_id: Uuid,
    pub goal: String,
    pub instructions: Option<String>,
    pub created_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct MemberHealth {
    pub member_id: Uuid,
    pub is_healthy: bool,
    pub active_tasks: usize,
    pub completed_tasks: usize,
    pub failed_tasks: usize,
}
```

Run: `cargo test member_agent_tests::test_member_agent_trait_exists -p torque-harness`
Expected: PASS

- [ ] **Step 3: Create stub LocalMemberAgent for trait object safety**

```rust
// crates/torque-harness/src/service/team/local_member_agent.rs
use super::*;

pub struct LocalMemberAgent {
    member_id: Uuid,
    // Add fields as needed
}

impl LocalMemberAgent {
    pub fn new(member_id: Uuid) -> Self {
        Self { member_id }
    }
}

#[async_trait]
impl MemberAgent for LocalMemberAgent {
    async fn start(&self) -> anyhow::Result<()> { Ok(()) }
    async fn stop(&self) -> anyhow::Result<()> { Ok(()) }
    async fn poll_tasks(&self) -> anyhow::Result<Vec<MemberTask>> { Ok(vec![]) }
    async fn accept_task(&self, _id: Uuid) -> anyhow::Result<()> { Ok(()) }
    async fn complete_task(&self, _id: Uuid, _artifact_id: Uuid) -> anyhow::Result<()> { Ok(()) }
    async fn fail_task(&self, _id: Uuid, _error: &str) -> anyhow::Result<()> { Ok(()) }
    async fn request_extension(&self, _id: Uuid, _seconds: u32, _reason: &str) -> anyhow::Result<bool> { Ok(false) }
    async fn health_check(&self) -> anyhow::Result<MemberHealth> {
        Ok(MemberHealth {
            member_id: self.member_id,
            is_healthy: true,
            active_tasks: 0,
            completed_tasks: 0,
            failed_tasks: 0,
        })
    }
}
```

Run: `cargo test member_agent_tests -p torque-harness`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/torque-harness/src/service/team/member_agent.rs
git add crates/torque-harness/src/service/team/local_member_agent.rs
git add crates/torque-harness/tests/member_agent_tests.rs
git commit -m "feat(async-delegation): add MemberAgent trait and LocalMemberAgent stub"
```

---

### Task 0.2: Create DelegationEvent Enum

**Files:**
- Create: `crates/torque-harness/src/models/v1/delegation_event.rs`
- Test: `crates/torque-harness/tests/delegation_event_tests.rs`

- [ ] **Step 1: Write failing test for DelegationEvent**

```rust
// crates/torque-harness/tests/delegation_event_tests.rs
use torque_harness::models::v1::delegation_event::*;

#[test]
fn test_delegation_event_serialization() {
    let event = DelegationEvent::Created {
        delegation_id: uuid::Uuid::new_v4(),
        task_id: uuid::Uuid::new_v4(),
        member_id: uuid::Uuid::new_v4(),
    };
    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("created"));
}
```

Run: `cargo test delegation_event_tests -p torque-harness`
Expected: FAIL - module not found

- [ ] **Step 2: Define DelegationEvent enum**

```rust
// crates/torque-harness/src/models/v1/delegation_event.rs
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum DelegationEvent {
    Created {
        delegation_id: Uuid,
        task_id: Uuid,
        member_id: Uuid,
        created_at: DateTime<Utc>,
    },
    Accepted {
        delegation_id: Uuid,
        member_id: Uuid,
        accepted_at: DateTime<Utc>,
    },
    Rejected {
        delegation_id: Uuid,
        member_id: Uuid,
        reason: RejectionReason,
        rejected_at: DateTime<Utc>,
    },
    Completed {
        delegation_id: Uuid,
        member_id: Uuid,
        artifact_id: Uuid,
        completed_at: DateTime<Utc>,
    },
    Failed {
        delegation_id: Uuid,
        member_id: Uuid,
        error: String,
        failed_at: DateTime<Utc>,
    },
    TimeoutPartial {
        delegation_id: Uuid,
        member_id: Uuid,
        partial_quality: PartialQuality,
        timed_out_at: DateTime<Utc>,
    },
    ExtensionRequested {
        delegation_id: Uuid,
        member_id: Uuid,
        requested_seconds: u32,
        reason: String,
        requested_at: DateTime<Utc>,
    },
    ExtensionGranted {
        delegation_id: Uuid,
        granted_seconds: u32,
        new_deadline: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RejectionReason {
    CapacityFull,
    CapabilityMismatch,
    PolicyViolation,
    MemberUnavailable,
    Timeout,
    Other(String),
}
```

Run: `cargo test delegation_event_tests -p torque-harness`
Expected: PASS

- [ ] **Step 3: Write test for PartialQuality**

```rust
// crates/torque-harness/tests/delegation_event_tests.rs
#[test]
fn test_partial_quality_serialization() {
    let quality = PartialQuality {
        completeness: 0.8,
        correctness_confidence: 0.6,
        usable_as_is: true,
        requires_repair: vec!["missing_imports".to_string()],
        estimated_remaining_work: Some("15s".to_string()),
    };
    let json = serde_json::to_string(&quality).unwrap();
    assert!(json.contains("completeness"));
    assert!(json.contains("0.8"));
}
```

Run: `cargo test delegation_event_tests::test_partial_quality_serialization -p torque-harness`
Expected: PASS

- [ ] **Step 4: Define PartialQuality struct**

```rust
// crates/torque-harness/src/models/v1/partial_quality.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartialQuality {
    pub completeness: f32,
    pub correctness_confidence: f32,
    pub usable_as_is: bool,
    pub requires_repair: Vec<String>,
    pub estimated_remaining_work: Option<String>,
}
```

Run: `cargo test delegation_event_tests -p torque-harness`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/torque-harness/src/models/v1/delegation_event.rs
git add crates/torque-harness/src/models/v1/partial_quality.rs
git add crates/torque-harness/tests/delegation_event_tests.rs
git commit -m "feat(async-delegation): add DelegationEvent and PartialQuality models"
```

---

## Phase 1: Model/Repository Extensions

### Task 1.1: Extend Delegation Model with Status and Result

**Files:**
- Modify: `crates/torque-harness/src/models/v1/delegation.rs:1-21`
- Test: `crates/torque-harness/tests/delegation_status_tests.rs`

- [ ] **Step 1: Write failing test for DelegationStatus enum**

```rust
// crates/torque-harness/tests/delegation_status_tests.rs
use torque_harness::models::v1::delegation::*;

#[test]
fn test_delegation_status_display() {
    assert_eq!(DelegationStatus::Pending.to_string(), "PENDING");
    assert_eq!(DelegationStatus::Completed.to_string(), "COMPLETED");
    assert_eq!(DelegationStatus::Failed.to_string(), "FAILED");
}
```

Run: `cargo test delegation_status_tests -p torque-harness`
Expected: FAIL - enum not found

- [ ] **Step 2: Add DelegationStatus and extend Delegation model**

```rust
// crates/torque-harness/src/models/v1/delegation.rs:1-35
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DelegationStatus {
    Pending,
    Accepted,
    Rejected,
    Completed,
    Failed,
    TimeoutPartial,
}

impl std::fmt::Display for DelegationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DelegationStatus::Pending => write!(f, "PENDING"),
            DelegationStatus::Accepted => write!(f, "ACCEPTED"),
            DelegationStatus::Rejected => write!(f, "REJECTED"),
            DelegationStatus::Completed => write!(f, "COMPLETED"),
            DelegationStatus::Failed => write!(f, "FAILED"),
            DelegationStatus::TimeoutPartial => write!(f, "TIMEOUT_PARTIAL"),
        }
    }
}

impl TryFrom<&str> for DelegationStatus {
    type Error = String;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "PENDING" => Ok(DelegationStatus::Pending),
            "ACCEPTED" => Ok(DelegationStatus::Accepted),
            "REJECTED" => Ok(DelegationStatus::Rejected),
            "COMPLETED" => Ok(DelegationStatus::Completed),
            "FAILED" => Ok(DelegationStatus::Failed),
            "TIMEOUT_PARTIAL" => Ok(DelegationStatus::TimeoutPartial),
            _ => Err(format!("Unknown status: {}", s)),
        }
    }
}

#[derive(Debug, Serialize, FromRow)]
pub struct Delegation {
    pub id: Uuid,
    pub task_id: Uuid,
    pub parent_agent_instance_id: Uuid,
    pub child_agent_definition_selector: serde_json::Value,
    pub status: DelegationStatus,
    pub result_artifact_id: Option<Uuid>,
    pub error_message: Option<String>,
    pub rejection_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct DelegationCreate {
    pub task_id: Uuid,
    pub parent_agent_instance_id: Uuid,
    pub child_agent_definition_selector: serde_json::Value,
}
```

Run: `cargo test delegation_status_tests -p torque-harness`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/torque-harness/src/models/v1/delegation.rs
git add crates/torque-harness/tests/delegation_status_tests.rs
git commit -m "feat(async-delegation): extend Delegation model with status, result, error fields"
```

---

### Task 1.2: Extend DelegationRepository with complete/fail methods

**Files:**
- Modify: `crates/torque-harness/src/repository/delegation.rs`
- Test: `crates/torque-harness/tests/delegation_repo_tests.rs`

- [ ] **Step 1: Write failing test for complete/fail methods**

```rust
// crates/torque-harness/tests/delegation_repo_tests.rs
use torque_harness::repository::{DelegationRepository, PostgresDelegationRepository};
use torque_harness::db::Database;

#[tokio::test]
async fn test_delegation_complete() {
    let db = Database::test().await;
    let repo = PostgresDelegationRepository::new(db.clone());

    // Create delegation first
    let delegation = repo.create(
        task_id,
        parent_instance_id,
        serde_json::json!({}),
    ).await.unwrap();

    // Complete it
    let artifact_id = Uuid::new_v4();
    let result = repo.complete(delegation.id, artifact_id).await;
    assert!(result.is_ok());

    // Verify status changed
    let updated = repo.get(delegation.id).await.unwrap().unwrap();
    assert_eq!(updated.status, DelegationStatus::Completed);
    assert_eq!(updated.result_artifact_id, Some(artifact_id));
}
```

Run: `cargo test delegation_repo_tests -p torque-harness`
Expected: FAIL - method not found

- [ ] **Step 2: Extend DelegationRepository trait**

```rust
// crates/torque-harness/src/repository/delegation.rs:1-30
#[async_trait]
pub trait DelegationRepository: Send + Sync {
    async fn create(
        &self,
        task_id: Uuid,
        parent_instance_id: Uuid,
        selector: serde_json::Value,
    ) -> anyhow::Result<Delegation>;
    async fn list(&self, limit: i64) -> anyhow::Result<Vec<Delegation>>;
    async fn list_by_instance(&self, instance_id: Uuid, limit: i64) -> anyhow::Result<Vec<Delegation>>;
    async fn list_by_task(&self, task_id: Uuid, limit: i64) -> anyhow::Result<Vec<Delegation>>;
    async fn get(&self, id: Uuid) -> anyhow::Result<Option<Delegation>>;
    async fn update_status(&self, id: Uuid, status: &str) -> anyhow::Result<bool>;
    async fn complete(&self, id: Uuid, artifact_id: Uuid) -> anyhow::Result<bool>;
    async fn fail(&self, id: Uuid, error: &str) -> anyhow::Result<bool>;
    async fn reject(&self, id: Uuid, reason: &str) -> anyhow::Result<bool>;
    async fn list_by_status(&self, task_id: Uuid, status: DelegationStatus) -> anyhow::Result<Vec<Delegation>>;
}
```

Run: `cargo test delegation_repo_tests::test_delegation_complete -p torque-harness`
Expected: FAIL - method not implemented

- [ ] **Step 3: Implement complete/fail in PostgresDelegationRepository**

```rust
// crates/torque-harness/src/repository/delegation.rs:90-130
async fn complete(&self, id: Uuid, artifact_id: Uuid) -> anyhow::Result<bool> {
    let result = sqlx::query(
        "UPDATE v1_delegations SET status = 'COMPLETED', result_artifact_id = $1, updated_at = NOW() WHERE id = $2"
    )
    .bind(artifact_id)
    .bind(id)
    .execute(self.db.pool())
    .await?;
    Ok(result.rows_affected() > 0)
}

async fn fail(&self, id: Uuid, error: &str) -> anyhow::Result<bool> {
    let result = sqlx::query(
        "UPDATE v1_delegations SET status = 'FAILED', error_message = $1, updated_at = NOW() WHERE id = $2"
    )
    .bind(error)
    .bind(id)
    .execute(self.db.pool())
    .await?;
    Ok(result.rows_affected() > 0)
}

async fn reject(&self, id: Uuid, reason: &str) -> anyhow::Result<bool> {
    let result = sqlx::query(
        "UPDATE v1_delegations SET status = 'REJECTED', rejection_reason = $1, updated_at = NOW() WHERE id = $2"
    )
    .bind(reason)
    .bind(id)
    .execute(self.db.pool())
    .await?;
    Ok(result.rows_affected() > 0)
}

async fn list_by_status(&self, task_id: Uuid, status: DelegationStatus) -> anyhow::Result<Vec<Delegation>> {
    let rows = sqlx::query_as::<_, Delegation>(
        "SELECT * FROM v1_delegations WHERE task_id = $1 AND status = $2 ORDER BY created_at DESC"
    )
    .bind(task_id)
    .bind(status.to_string())
    .fetch_all(self.db.pool())
    .await?;
    Ok(rows)
}
```

Run: `cargo test delegation_repo_tests -p torque-harness`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/torque-harness/src/repository/delegation.rs
git commit -m "feat(async-delegation): add complete/fail/reject/list_by_status to DelegationRepository"
```

---

## Phase 2: Redis Streams Infrastructure

### Task 2.1: Define Stream Key Constants

**Files:**
- Create: `crates/torque-harness/src/message_bus/keys.rs`
- Test: `crates/torque-harness/tests/stream_keys_tests.rs`

- [ ] **Step 1: Write failing test for stream key constants**

```rust
// crates/torque-harness/tests/stream_keys_tests.rs
use torque_harness::message_bus::keys::*;

#[test]
fn test_shared_pool_key() {
    let team_id = uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let key = TEAM_SHARED_POOL.key(team_id);
    assert_eq!(key, "team:550e8400-e29b-41d4-a716-446655440000:tasks:shared");
}
```

Run: `cargo test stream_keys_tests -p torque-harness`
Expected: FAIL - module not found

- [ ] **Step 2: Create message_bus module and keys**

```rust
// crates/torque-harness/src/message_bus/mod.rs
pub mod keys;
pub mod stream_bus;
```

```rust
// crates/torque-harness/src/message_bus/keys.rs
use uuid::Uuid;

pub struct StreamKeys;

impl StreamKeys {
    pub fn team_shared_pool(team_id: Uuid) -> String {
        format!("team:{}:tasks:shared", team_id)
    }

    pub fn member_tasks(member_id: Uuid) -> String {
        format!("member:{}:tasks", member_id)
    }

    pub fn delegation_status(delegation_id: Uuid) -> String {
        format!("delegation:{}:status", delegation_id)
    }
}

pub const TEAM_SHARED_POOL: StreamKeys = StreamKeys;
pub const MEMBER_TASKS: StreamKeys = StreamKeys;
pub const DELEGATION_STATUS: StreamKeys = StreamKeys;
```

Run: `cargo test stream_keys_tests -p torque-harness`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/torque-harness/src/message_bus/mod.rs
git add crates/torque-harness/src/message_bus/keys.rs
git commit -m "feat(async-delegation): add Redis stream key constants"
```

---

### Task 2.2: Implement StreamBus Trait and Redis Stream Publisher

**Files:**
- Create: `crates/torque-harness/src/message_bus/stream_bus.rs`
- Create: `crates/torque-harness/src/message_bus/delegation_publisher.rs`
- Test: `crates/torque-harness/tests/stream_bus_tests.rs`

- [ ] **Step 1: Write failing test for StreamBus trait**

```rust
// crates/torque-harness/tests/stream_bus_tests.rs
use torque_harness::message_bus::{StreamBus, StreamMessage};
use redis::aio::ConnectionManager;

#[tokio::test]
async fn test_stream_bus_xadd() {
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost".to_string());
    let client = redis::Client::open(redis_url).unwrap();
    let conn = ConnectionManager::new(client).await.unwrap();

    let bus = RedisStreamBus::new(conn);
    let msg = StreamMessage::new("key", serde_json::json!({"test": "data"}));
    let result = bus.xadd("test-stream", &msg).await;
    assert!(result.is_ok());
}
```

Run: `cargo test stream_bus_tests -p torque-harness`
Expected: FAIL - module not found

- [ ] **Step 2: Define StreamBus trait**

```rust
// crates/torque-harness/src/message_bus/stream_bus.rs
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamMessage {
    pub id: Option<String>,
    pub data: serde_json::Value,
    pub timestamp: chrono::DateTime<Utc>,
}

impl StreamMessage {
    pub fn new(key: &str, data: serde_json::Value) -> Self {
        Self {
            id: None,
            data: serde_json::json!({
                "key": key,
                "data": data,
                "timestamp": chrono::Utc::now().to_rfc3339(),
            }),
            timestamp: chrono::Utc::now(),
        }
    }
}

#[async_trait]
pub trait StreamBus: Send + Sync {
    async fn xadd(&self, stream: &str, message: &StreamMessage) -> anyhow::Result<String>;
    async fn xread(&self, streams: &[(&str, &str)], count: usize) -> anyhow::Result<Vec<StreamReadResult>>;
    async fn xreadgroup(
        &self,
        group: &str,
        consumer: &str,
        streams: &[(&str, &str)],
        count: usize,
    ) -> anyhow::Result<Vec<StreamReadResult>>;
    async fn xack(&self, stream: &str, group: &str, ids: &[&str]) -> anyhow::Result<()>;
    async fn create_consumer_group(&self, stream: &str, group: &str, start_id: &str) -> anyhow::Result<()>;
}

#[derive(Debug)]
pub struct StreamReadResult {
    pub stream: String,
    pub id: String,
    pub data: serde_json::Value,
}
```

Run: `cargo test stream_bus_tests::test_stream_bus_trait_exists -p torque-harness`
Expected: FAIL - impl not found

- [ ] **Step 3: Implement RedisStreamBus**

```rust
// crates/torque-harness/src/message_bus/stream_bus.rs (add impl)
use redis::aio::ConnectionManager;
use redis::AsyncCommands;

pub struct RedisStreamBus {
    conn: ConnectionManager,
}

impl RedisStreamBus {
    pub fn new(conn: ConnectionManager) -> Self {
        Self { conn }
    }
}

#[async_trait]
impl StreamBus for RedisStreamBus {
    async fn xadd(&self, stream: &str, message: &StreamMessage) -> anyhow::Result<String> {
        let mut conn = self.conn.clone();
        let id: String = redis::cmd("XADD")
            .arg(stream)
            .arg("*")
            .arg(&message.data)
            .query_async(&mut conn)
            .await?;
        Ok(id)
    }

    async fn xread(&self, streams: &[(&str, &str)], count: usize) -> anyhow::Result<Vec<StreamReadResult>> {
        let mut conn = self.conn.clone();
        let mut args: Vec<String> = vec!["COUNT".to_string(), count.to_string()];
        for (stream, id) in streams {
            args.push(stream.to_string());
            args.push(id.to_string());
        }
        let result: Vec<(String, Vec<(String, Vec<(String, String)>)>)> = redis::cmd("XREAD")
            .arg(&args)
            .query_async(&mut conn)
            .await?;

        let mut results = Vec::new();
        for (stream, entries) in result {
            for (id, fields) in entries {
                let data: serde_json::Value = fields.into_iter().collect();
                results.push(StreamReadResult {
                    stream,
                    id,
                    data,
                });
            }
        }
        Ok(results)
    }

    async fn xreadgroup(&self, group: &str, consumer: &str, streams: &[(&str, &str)], count: usize) -> anyhow::Result<Vec<StreamReadResult>> {
        let mut conn = self.conn.clone();
        let mut args: Vec<String> = vec![
            "GROUP".to_string(), group.to_string(), consumer.to_string(),
            "COUNT".to_string(), count.to_string(),
        ];
        for (stream, id) in streams {
            args.push(stream.to_string());
            args.push(id.to_string());
        }
        let result: Vec<(String, Vec<(String, Vec<(String, String)>)>)> = redis::cmd("XREADGROUP")
            .arg(&args)
            .query_async(&mut conn)
            .await?;

        let mut results = Vec::new();
        for (stream, entries) in result {
            for (id, fields) in entries {
                let data: serde_json::Value = fields.into_iter().collect();
                results.push(StreamReadResult {
                    stream,
                    id,
                    data,
                });
            }
        }
        Ok(results)
    }

    async fn xack(&self, stream: &str, group: &str, ids: &[&str]) -> anyhow::Result<()> {
        let mut conn = self.conn.clone();
        let mut args = vec![stream.to_string(), group.to_string()];
        args.extend(ids.iter().map(|s| s.to_string()));
        redis::cmd("XACK")
            .arg(&args)
            .query_async(&mut conn)
            .await?;
        Ok(())
    }

    async fn create_consumer_group(&self, stream: &str, group: &str, start_id: &str) -> anyhow::Result<()> {
        let mut conn = self.conn.clone();
        let _: () = redis::cmd("XGROUP")
            .arg("CREATE")
            .arg(stream)
            .arg(group)
            .arg(start_id)
            .arg("MKSTREAM")
            .query_async(&mut conn)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create consumer group: {}", e))?;
        Ok(())
    }
}
```

Run: `cargo test stream_bus_tests -p torque-harness`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/torque-harness/src/message_bus/stream_bus.rs
git commit -m "feat(async-delegation): add StreamBus trait and RedisStreamBus implementation"
```

---

## Phase 3: Circuit Breaker and Retry

### Task 3.1: Implement CircuitBreaker

**Files:**
- Create: `crates/torque-harness/src/service/team/circuit_breaker.rs`
- Test: `crates/torque-harness/tests/circuit_breaker_tests.rs`

- [ ] **Step 1: Write failing test for CircuitBreaker**

```rust
// crates/torque-harness/tests/circuit_breaker_tests.rs
use torque_harness::service::team::circuit_breaker::*;

#[test]
fn test_circuit_breaker_initial_closed() {
    let cb = CircuitBreaker::new(5, 3);
    assert_eq!(cb.state(), CircuitState::Closed);
}

#[test]
fn test_circuit_breaker_opens_after_threshold() {
    let mut cb = CircuitBreaker::new(3, 3);
    for _ in 0..3 {
        cb.record_failure(&RejectionReason::CapacityFull);
    }
    assert_eq!(cb.state(), CircuitState::Open);
}
```

Run: `cargo test circuit_breaker_tests -p torque-harness`
Expected: FAIL - module not found

- [ ] **Step 2: Define CircuitBreaker**

```rust
// crates/torque-harness/src/service/team/circuit_breaker.rs
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

pub struct CircuitBreaker {
    failure_threshold: usize,
    success_threshold: usize,
    state: RwLock<CircuitState>,
    failure_count: RwLock<usize>,
    success_count: RwLock<usize>,
    last_failure_time: RwLock<Option<DateTime<Utc>>,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: usize, success_threshold: usize) -> Self {
        Self {
            failure_threshold,
            success_threshold,
            state: RwLock::new(CircuitState::Closed),
            failure_count: RwLock::new(0),
            success_count: RwLock::new(0),
            last_failure_time: RwLock::new(None),
        }
    }

    pub async fn state(&self) -> CircuitState {
        *self.state.read().await
    }

    pub async fn record_failure(&self, reason: &RejectionReason) {
        let mut count = self.failure_count.write().await;
        *count += 1;
        *self.last_failure_time.write().await = Some(Utc::now());

        if *count >= self.failure_threshold {
            *self.state.write().await = CircuitState::Open;
        }
    }

    pub async fn record_success(&self) {
        let mut count = self.success_count.write().await;
        *count += 1;

        if *count >= self.success_threshold {
            *self.state.write().await = CircuitState::Closed;
            *self.failure_count.write().await = 0;
            *count = 0;
        }
    }

    pub async fn allow_request(&self) -> bool {
        let state = self.state.read().await;
        match *state {
            CircuitState::Closed => true,
            CircuitState::HalfOpen => true,
            CircuitState::Open => false,
        }
    }

    pub async fn transition_to_half_open(&self) {
        *self.state.write().await = CircuitState::HalfOpen;
        *self.success_count.write().await = 0;
    }
}
```

Run: `cargo test circuit_breaker_tests -p torque-harness`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/torque-harness/src/service/team/circuit_breaker.rs
git commit -m "feat(async-delegation): add CircuitBreaker with state management"
```

---

### Task 3.2: Implement MemberHealthTracker and BudgetManager

**Files:**
- Create: `crates/torque-harness/src/service/team/member_health.rs`
- Create: `crates/torque-harness/src/service/team/retry.rs`
- Test: `crates/torque-harness/tests/retry_tests.rs`

- [ ] **Step 1: Write failing test for MemberHealthTracker**

```rust
// crates/torque-harness/tests/retry_tests.rs
use torque_harness::service::team::member_health::*;
use torque_harness::service::team::circuit_breaker::*;

#[tokio::test]
async fn test_member_health_track_failure() {
    let tracker = MemberHealthTracker::new();
    let member_id = Uuid::new_v4();

    tracker.record_failure(member_id, RejectionReason::CapacityFull).await;
    let health = tracker.get_health(member_id).await.unwrap();

    assert_eq!(health.failure_count, 1);
    assert!(!health.is_healthy);
}
```

Run: `cargo test retry_tests -p torque-harness`
Expected: FAIL - module not found

- [ ] **Step 2: Define MemberHealthTracker**

```rust
// crates/torque-harness/src/service/team/member_health.rs
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use super::circuit_breaker::{CircuitBreaker, CircuitState};

#[derive(Debug, Clone)]
pub struct MemberHealth {
    pub member_id: Uuid,
    pub circuit_state: CircuitState,
    pub failure_count: usize,
    pub success_count: usize,
    pub is_healthy: bool,
    pub last_seen: chrono::DateTime<Utc>,
}

pub struct MemberHealthTracker {
    members: RwLock<HashMap<Uuid, Arc<CircuitBreaker>>,
}

impl MemberHealthTracker {
    pub fn new() -> Self {
        Self {
            members: RwLock::new(HashMap::new()),
        }
    }

    pub async fn get_or_create(&self, member_id: Uuid) -> Arc<CircuitBreaker> {
        let mut members = self.members.write().await;
        if let Some(cb) = members.get(&member_id) {
            return cb.clone();
        }
        let cb = Arc::new(CircuitBreaker::new(5, 3));
        members.insert(member_id, cb.clone());
        cb
    }

    pub async fn record_failure(&self, member_id: Uuid, reason: RejectionReason) {
        let cb = self.get_or_create(member_id).await;
        cb.record_failure(&reason).await;
    }

    pub async fn record_success(&self, member_id: Uuid) {
        let cb = self.get_or_create(member_id).await;
        cb.record_success().await;
    }

    pub async fn is_healthy(&self, member_id: Uuid) -> bool {
        let cb = self.get_or_create(member_id).await;
        cb.allow_request().await
    }

    pub async fn get_health(&self, member_id: Uuid) -> Option<MemberHealth> {
        let cb = self.get_or_create(member_id).await;
        Some(MemberHealth {
            member_id,
            circuit_state: cb.state().await,
            failure_count: 0,
            success_count: 0,
            is_healthy: cb.allow_request().await,
            last_seen: chrono::Utc::now(),
        })
    }
}
```

Run: `cargo test retry_tests::test_member_health_track_failure -p torque-harness`
Expected: PASS

- [ ] **Step 3: Define BudgetManager**

```rust
// crates/torque-harness/src/service/team/retry.rs
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct RetryBudget {
    pub total: usize,
    pub remaining: usize,
    pub spent: usize,
}

impl RetryBudget {
    pub fn new(total: usize) -> Self {
        Self {
            total,
            remaining: total,
            spent: 0,
        }
    }

    pub fn consume(&mut self, amount: usize) -> bool {
        if self.remaining >= amount {
            self.remaining -= amount;
            self.spent += amount;
            true
        } else {
            false
        }
    }

    pub fn can_retry(&self) -> bool {
        self.remaining > 0
    }

    pub fn is_exhausted(&self) -> bool {
        self.remaining == 0
    }
}

pub enum RetryDecision {
    RetryWithSameMember,
    RetryWithOtherMember,
    DoNotRetry { reason: String },
}

pub fn classify_rejection(reason: &RejectionReason, budget: &RetryBudget) -> RetryDecision {
    match reason {
        RejectionReason::CapacityFull => {
            if budget.can_retry() {
                RetryDecision::RetryWithOtherMember
            } else {
                RetryDecision::DoNotRetry { reason: "Budget exhausted".to_string() }
            }
        }
        RejectionReason::Timeout => {
            if budget.can_retry() {
                RetryDecision::RetryWithSameMember
            } else {
                RetryDecision::DoNotRetry { reason: "Budget exhausted".to_string() }
            }
        }
        RejectionReason::CapabilityMismatch => {
            RetryDecision::DoNotRetry { reason: "Capability mismatch".to_string() }
        }
        RejectionReason::PolicyViolation => {
            RetryDecision::DoNotRetry { reason: "Policy violation".to_string() }
        }
        RejectionReason::MemberUnavailable => {
            if budget.can_retry() {
                RetryDecision::RetryWithOtherMember
            } else {
                RetryDecision::DoNotRetry { reason: "Budget exhausted".to_string() }
            }
        }
        RejectionReason::Other(_) => {
            RetryDecision::DoNotRetry { reason: "Unknown error".to_string() }
        }
    }
}
```

Run: `cargo test retry_tests -p torque-harness`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/torque-harness/src/service/team/member_health.rs
git add crates/torque-harness/src/service/team/retry.rs
git commit -m "feat(async-delegation): add MemberHealthTracker and BudgetManager with RetryStrategy"
```

---

## Phase 4: Event Listener for Async Wait

### Task 4.1: Implement RedisStreamEventListener

**Files:**
- Create: `crates/torque-harness/src/service/team/event_listener.rs`
- Test: `crates/torque-harness/tests/event_listener_tests.rs`

- [ ] **Step 1: Write failing test for EventListener**

```rust
// crates/torque-harness/tests/event_listener_tests.rs
use torque_harness::service::team::event_listener::*;
use futures::StreamExt;

#[tokio::test]
async fn test_event_listener_subscribes_to_delegation() {
    let listener = RedisStreamEventListener::new(redis_conn).await.unwrap();
    let delegation_id = Uuid::new_v4();

    let mut stream = listener.subscribe_delegation(delegation_id).await.unwrap();

    // Later: publish event and verify we receive it
}
```

Run: `cargo test event_listener_tests -p torque-harness`
Expected: FAIL - module not found

- [ ] **Step 2: Define EventListener trait**

```rust
// crates/torque-harness/src/service/team/event_listener.rs
use async_trait::async_trait;
use futures::Stream;
use uuid::Uuid;
use crate::models::v1::delegation_event::DelegationEvent;

#[async_trait]
pub trait EventListener: Send + Sync {
    async fn subscribe_delegation(&self, delegation_id: Uuid) -> anyhow::Result<impl Stream<Item = DelegationEvent>>>;
    async fn subscribe_team(&self, team_id: Uuid) -> anyhow::Result<impl Stream<Item = DelegationEvent>>>;
    async fn subscribe_member(&self, member_id: Uuid) -> anyhow::Result<impl Stream<Item = DelegationEvent>>>;
}

pub struct RedisStreamEventListener {
    // Add redis connection and subscription state
}

impl RedisStreamEventListener {
    pub async fn new(redis_url: &str) -> anyhow::Result<Self> {
        // Initialize redis connection
        todo!()
    }
}

#[async_trait]
impl EventListener for RedisStreamEventListener {
    async fn subscribe_delegation(&self, delegation_id: Uuid) -> anyhow::Result<impl Stream<Item = DelegationEvent>> {
        // Subscribe to delegation status stream
        todo!()
    }

    async fn subscribe_team(&self, team_id: Uuid) -> anyhow::Result<impl Stream<Item = DelegationEvent>> {
        // Subscribe to team shared pool
        todo!()
    }

    async fn subscribe_member(&self, member_id: Uuid) -> anyhow::Result<impl Stream<Item = DelegationEvent>> {
        // Subscribe to member individual queue
        todo!()
    }
}
```

Run: `cargo test event_listener_tests -p torque-harness`
Expected: PASS (with todo!())

- [ ] **Step 3: Implement full RedisStreamEventListener**

```rust
// Full implementation with tokio_stream::StreamExt for polling
```

Run: `cargo test event_listener_tests -p torque-harness`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/torque-harness/src/service/team/event_listener.rs
git commit -m "feat(async-delegation): add EventListener trait and RedisStreamEventListener"
```

---

## Phase 5: Supervisor Async Wait

### Task 5.1: Add wait_for_delegation_completion to Supervisor

**Files:**
- Modify: `crates/torque-harness/src/service/team/supervisor.rs`
- Test: `crates/torque-harness/tests/supervisor_async_tests.rs`

- [ ] **Step 1: Write failing test for async wait**

```rust
// crates/torque-harness/tests/supervisor_async_tests.rs
use torque_harness::service::team::SupervisorTestHarness;

#[tokio::test]
async fn test_supervisor_waits_for_delegation_completion() {
    let harness = SupervisorTestHarness::new().await;

    // Create delegation
    let delegation_id = harness.create_delegation().await.unwrap();

    // Start waiting in background
    let handle = harness.wait_for_delegation(delegation_id, Duration::from_secs(5));

    // Complete delegation
    harness.complete_delegation(delegation_id).await;

    // Verify wait completes
    let result = handle.await.unwrap();
    assert!(result.is_completed());
}
```

Run: `cargo test supervisor_async_tests -p torque-harness`
Expected: FAIL - method not found

- [ ] **Step 2: Add wait_for_delegation_completion to TeamSupervisor**

```rust
// crates/torque-harness/src/service/team/supervisor.rs
use tokio::time::{timeout, Duration};
use crate::service::team::event_listener::EventListener;

pub struct SupervisorConfig {
    pub delegation_timeout: Duration,
    pub task_timeout: Duration,
}

impl Default for SupervisorConfig {
    fn default() -> Self {
        Self {
            delegation_timeout: Duration::from_secs(8),
            task_timeout: Duration::from_secs(25),
        }
    }
}

impl TeamSupervisor {
    pub async fn wait_for_delegation_completion(
        &self,
        delegation_id: Uuid,
        event_listener: Arc<dyn EventListener>,
        timeout_duration: Duration,
    ) -> anyhow::Result<DelegationWaitResult> {
        let deadline = tokio::time::Instant::now() + timeout_duration;

        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Ok(DelegationWaitResult::Timeout);
            }

            let stream = event_listener.subscribe_delegation(delegation_id).await?;

            tokio::pin!(stream);

            match timeout(remaining, stream.next()).await {
                Ok(Some(event)) => {
                    match event {
                        DelegationEvent::Completed { .. } => {
                            return Ok(DelegationWaitResult::Completed);
                        }
                        DelegationEvent::Failed { error, .. } => {
                            return Ok(DelegationWaitResult::Failed(error));
                        }
                        DelegationEvent::TimeoutPartial { partial_quality, .. } => {
                            return Ok(DelegationWaitResult::TimeoutPartial(partial_quality));
                        }
                        _ => continue,
                    }
                }
                Ok(None) => continue,
                Err(_) => return Ok(DelegationWaitResult::Timeout),
            }
        }
    }
}

pub enum DelegationWaitResult {
    Completed,
    Failed(String),
    TimeoutPartial(crate::models::v1::partial_quality::PartialQuality),
    Timeout,
}
```

Run: `cargo test supervisor_async_tests -p torque-harness`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/torque-harness/src/service/team/supervisor.rs
git commit -m "feat(async-delegation): add wait_for_delegation_completion to TeamSupervisor"
```

---

## Phase 6: API Extensions

### Task 6.1: Add complete/fail endpoints

**Files:**
- Modify: `crates/torque-harness/src/api/v1/delegations.rs`
- Test: `crates/torque-harness/tests/delegation_api_tests.rs`

- [ ] **Step 1: Write failing test for complete endpoint**

```rust
// crates/torque-harness/tests/delegation_api_tests.rs
use axum::{TestClient, Router};
use serde_json::json;

#[tokio::test]
async fn test_delegation_complete_endpoint() {
    let app = create_test_app();
    let client = TestClient::new(app);

    let response = client
        .post("/v1/delegations/550e8400-e29b-41d4-a716-446655440000/complete")
        .json(&json!({ "artifact_id": "..." }))
        .send()
        .await;

    assert_eq!(response.status(), 200);
}
```

Run: `cargo test delegation_api_tests -p torque-harness`
Expected: FAIL - endpoint not found

- [ ] **Step 2: Add complete/fail endpoint handlers**

```rust
// crates/torque-harness/src/api/v1/delegations.rs

#[derive(serde::Deserialize)]
pub struct CompleteRequest {
    pub artifact_id: Uuid,
}

#[derive(serde::Deserialize)]
pub struct FailRequest {
    pub error: String,
}

pub async fn complete(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
    Json(req): Json<CompleteRequest>,
) -> Result<StatusCode, StatusCode> {
    match services.delegation.complete(id, req.artifact_id).await {
        Ok(true) => Ok(StatusCode::OK),
        Ok(false) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub async fn fail(
    State((_, _, services)): State<(Database, Arc<OpenAiClient>, Arc<ServiceContainer>)>,
    Path(id): Path<Uuid>,
    Json(req): Json<FailRequest>,
) -> Result<StatusCode, StatusCode> {
    match services.delegation.fail(id, &req.error).await {
        Ok(true) => Ok(StatusCode::OK),
        Ok(false) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}
```

- [ ] **Step 3: Register routes in mod.rs**

```rust
// crates/torque-harness/src/api/v1/mod.rs
.route("/v1/delegations/:id/complete", post(delegations::complete))
.route("/v1/delegations/:id/fail", post(delegations::fail))
```

Run: `cargo test delegation_api_tests -p torque-harness`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/torque-harness/src/api/v1/delegations.rs
git add crates/torque-harness/src/api/v1/mod.rs
git commit -m "feat(async-delegation): add complete and fail API endpoints"
```

---

## Phase 7: Integration Tests

### Task 7.1: Write full async delegation flow test

**Files:**
- Create: `crates/torque-harness/tests/async_delegation_flow_tests.rs`

- [ ] **Step 1: Write end-to-end test**

```rust
// crates/torque-harness/tests/async_delegation_flow_tests.rs
#[tokio::test]
#[serial]
async fn test_full_async_delegation_flow() {
    // 1. Setup
    let db = setup_test_db().await;
    let redis = redis::Client::open(std::env::var("REDIS_URL").unwrap());
    let harness = TestHarness::new(db, redis).await;

    // 2. Create team with supervisor
    let team = harness.create_team().await.unwrap();

    // 3. Create task
    let task = harness.create_task(team.id, "Analyze code").await.unwrap();

    // 4. Supervisor creates delegation
    let delegation = harness.create_delegation(task.id, team.id).await.unwrap();

    // 5. Member polls and accepts
    let member_task = harness.poll_by_member(MEMBER_ID).await.unwrap();
    assert_eq!(member_task.delegation_id, delegation.id);

    // 6. Member completes task
    let artifact = harness.create_artifact("analysis result").await.unwrap();
    harness.complete_delegation(delegation.id, artifact.id).await.unwrap();

    // 7. Supervisor waits and receives completion
    let result = harness.wait_for(delegation.id, Duration::from_secs(10)).await;
    assert!(matches!(result, WaitResult::Completed));
}
```

Run: `cargo test async_delegation_flow_tests -p torque-harness`
Expected: Should pass with full implementation

- [ ] **Step 2: Commit**

```bash
git add crates/torque-harness/tests/async_delegation_flow_tests.rs
git commit -m "test(async-delegation): add end-to-end async delegation flow test"
```

---

## Summary

| Phase | Tasks | Estimated Time |
|-------|-------|----------------|
| 0 | 2 | 2-3h |
| 1 | 2 | 2-3h |
| 2 | 2 | 4-6h |
| 3 | 2 | 2-3h |
| 4 | 2 | 4-5h |
| 5 | 1 | 2-3h |
| 6 | 1 | 2h |
| 7 | 1 | 2h |
| **Total** | **13** | **20-28h** |

---

## Execution Options

**Plan complete and saved to `docs/superpowers/plans/2026-04-21-async-delegation-implementation.md`.**

**Two execution options:**

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

**Which approach?**
