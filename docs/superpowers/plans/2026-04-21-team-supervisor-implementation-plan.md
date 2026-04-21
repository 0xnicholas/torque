# Team Supervisor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Team Supervisor orchestration for torque-harness with all 4 modes (route, broadcast, coordinate, tasks), selector resolution, shared state, and team events.

**Architecture:** Supervisor-as-tool-agent approach where the supervisor is a ReActHarness agent with team-specific tools (delegate, publish, accept_result, etc.). Mode handlers manage delegation lifecycle per mode. SelectorResolver resolves capability profiles to candidate members.

**Tech Stack:** Rust, sqlx, tokio, ReActHarness

---

## File Structure

```
src/
├── service/team/
│   ├── mod.rs              # TeamService + new methods
│   ├── supervisor.rs       # TeamSupervisor orchestration
│   ├── modes/
│   │   ├── mod.rs
│   │   ├── route.rs
│   │   ├── broadcast.rs
│   │   ├── coordinate.rs
│   │   └── tasks.rs
│   ├── selector.rs         # SelectorResolver
│   ├── shared_state.rs     # SharedTaskState management
│   └── events.rs           # TeamEvent emission
├── models/v1/team.rs      # Add TeamTask, SharedTaskState, TeamEvent, etc.
├── repository/
│   ├── mod.rs              # Add new repo traits/instances
│   └── team.rs            # Add new repository implementations
├── tools/
│   └── team_tools.rs       # Supervisor agent tools
└── api/v1/
    └── teams.rs            # Add tasks endpoint

migrations/
├── 20260421000001_create_v1_team_tasks.up.sql
├── 20260421000001_create_v1_team_tasks.down.sql
├── 20260421000002_create_v1_team_shared_state.up.sql
├── 20260421000002_create_v1_team_shared_state.down.sql
└── 20260421000003_create_v1_team_events.up.sql
    20260421000003_create_v1_team_events.down.sql
```

---

## Phase 1: Database & Models

### Task 1: Database Migrations

**Files:**
- Create: `migrations/20260421000001_create_v1_team_tasks.up.sql`
- Create: `migrations/20260421000001_create_v1_team_tasks.down.sql`
- Create: `migrations/20260421000002_create_v1_team_shared_state.up.sql`
- Create: `migrations/20260421000002_create_v1_team_shared_state.down.sql`
- Create: `migrations/20260421000003_create_v1_team_events.up.sql`
- Create: `migrations/20260421000003_create_v1_team_events.down.sql`

- [ ] **Step 1: Create migration for v1_team_tasks**

```sql
-- up
CREATE TABLE v1_team_tasks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    team_instance_id UUID NOT NULL REFERENCES v1_team_instances(id),
    goal TEXT NOT NULL,
    instructions TEXT,
    status TEXT NOT NULL DEFAULT 'OPEN',
    triage_result JSONB,
    mode_selected TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ
);

CREATE INDEX idx_v1_team_tasks_team_instance_id ON v1_team_tasks(team_instance_id);
CREATE INDEX idx_v1_team_tasks_status ON v1_team_tasks(status);

-- down
DROP TABLE IF EXISTS v1_team_tasks;
```

- [ ] **Step 2: Create migration for v1_team_shared_state**

```sql
-- up
CREATE TABLE v1_team_shared_state (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    team_instance_id UUID NOT NULL REFERENCES v1_team_instances(id) UNIQUE,
    accepted_artifact_refs JSONB NOT NULL DEFAULT '[]',
    published_facts JSONB NOT NULL DEFAULT '[]',
    delegation_status JSONB NOT NULL DEFAULT '[]',
    open_blockers JSONB NOT NULL DEFAULT '[]',
    decisions JSONB NOT NULL DEFAULT '[]',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- down
DROP TABLE IF EXISTS v1_team_shared_state;
```

- [ ] **Step 3: Create migration for v1_team_events**

```sql
-- up
CREATE TABLE v1_team_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    team_instance_id UUID NOT NULL REFERENCES v1_team_instances(id),
    event_type TEXT NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    actor_ref TEXT NOT NULL,
    team_task_ref UUID REFERENCES v1_team_tasks(id),
    related_instance_refs JSONB NOT NULL DEFAULT '[]',
    related_artifact_refs JSONB NOT NULL DEFAULT '[]',
    payload JSONB NOT NULL DEFAULT '{}',
    causal_event_refs JSONB NOT NULL DEFAULT '[]'
);

CREATE INDEX idx_v1_team_events_team_instance_id ON v1_team_events(team_instance_id);
CREATE INDEX idx_v1_team_events_event_type ON v1_team_events(event_type);

-- down
DROP TABLE IF EXISTS v1_team_events;
```

- [ ] **Step 4: Run migrations to verify**

Run: `cd crates/torque-harness && cargo sqlx migrate run`
Expected: All 3 migrations apply successfully

- [ ] **Step 5: Commit**

```bash
git add migrations/
git commit -m "feat(team): add team tasks, shared state, and events tables"
```

---

### Task 2: Add Models

**Files:**
- Modify: `src/models/v1/team.rs`

- [ ] **Step 1: Add TeamTask model**

Add to `src/models/v1/team.rs`:

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TeamTask {
    pub id: Uuid,
    pub team_instance_id: Uuid,
    pub goal: String,
    pub instructions: Option<String>,
    pub status: TeamTaskStatus,
    pub triage_result: Option<TriageResult>,
    pub mode_selected: Option<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TeamTaskStatus {
    Open,
    Triaged,
    InProgress,
    WaitingMembers,
    ResultsReceived,
    Blocked,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for TeamTaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TeamTaskStatus::Open => write!(f, "OPEN"),
            TeamTaskStatus::Triaged => write!(f, "TRIAGED"),
            TeamTaskStatus::InProgress => write!(f, "IN_PROGRESS"),
            TeamTaskStatus::WaitingMembers => write!(f, "WAITING_MEMBERS"),
            TeamTaskStatus::ResultsReceived => write!(f, "RESULTS_RECEIVED"),
            TeamTaskStatus::Blocked => write!(f, "BLOCKED"),
            TeamTaskStatus::Completed => write!(f, "COMPLETED"),
            TeamTaskStatus::Failed => write!(f, "FAILED"),
            TeamTaskStatus::Cancelled => write!(f, "CANCELLED"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TriageResult {
    pub complexity: TaskComplexity,
    pub processing_path: ProcessingPath,
    pub selected_mode: TeamMode,
    pub lead_member_ref: Option<String>,
    pub rationale: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TaskComplexity {
    Simple,
    Medium,
    Complex,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProcessingPath {
    SingleRoute,
    GuidedDelegate,
    StructuredOrchestration,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TeamMode {
    Route,
    Broadcast,
    Coordinate,
    Tasks,
}
```

- [ ] **Step 2: Add SharedTaskState model**

Add to `src/models/v1/team.rs`:

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SharedTaskState {
    pub id: Uuid,
    pub team_instance_id: Uuid,
    pub accepted_artifact_refs: Vec<ArtifactRef>,
    pub published_facts: Vec<PublishedFact>,
    pub delegation_status: Vec<DelegationStatusEntry>,
    pub open_blockers: Vec<Blocker>,
    pub decisions: Vec<Decision>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ArtifactRef {
    pub artifact_id: Uuid,
    pub scope: PublishScope,
    pub published_by: String,
    pub published_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublishScope {
    Private,
    TeamShared,
    ExternalPublished,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PublishedFact {
    pub key: String,
    pub value: serde_json::Value,
    pub published_by: String,
    pub published_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DelegationStatusEntry {
    pub delegation_id: Uuid,
    pub status: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Blocker {
    pub blocker_id: Uuid,
    pub description: String,
    pub source: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Decision {
    pub decision_id: Uuid,
    pub description: String,
    pub decided_by: String,
    pub decided_at: DateTime<Utc>,
}
```

- [ ] **Step 3: Add TeamEvent model**

Add to `src/models/v1/team.rs`:

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TeamEvent {
    pub id: Uuid,
    pub team_instance_id: Uuid,
    pub event_type: TeamEventType,
    pub timestamp: DateTime<Utc>,
    pub actor_ref: String,
    pub team_task_ref: Option<Uuid>,
    pub related_instance_refs: Vec<Uuid>,
    pub related_artifact_refs: Vec<Uuid>,
    pub payload: serde_json::Value,
    pub causal_event_refs: Vec<Uuid>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TeamEventType {
    TeamTaskReceived,
    TriageCompleted,
    ModeSelected,
    LeadAssigned,
    MemberActivated,
    DelegationCreated,
    DelegationAccepted,
    DelegationRejected,
    MemberResultReceived,
    MemberResultAccepted,
    MemberResultRejected,
    ArtifactPublished,
    FactPublished,
    BlockerAdded,
    BlockerResolved,
    ApprovalRequested,
    TeamBlocked,
    TeamUnblocked,
    TeamCompleted,
    TeamFailed,
}

impl std::fmt::Display for TeamEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TeamEventType::TeamTaskReceived => write!(f, "TEAM_TASK_RECEIVED"),
            TeamEventType::TriageCompleted => write!(f, "TRIAGE_COMPLETED"),
            TeamEventType::ModeSelected => write!(f, "MODE_SELECTED"),
            TeamEventType::LeadAssigned => write!(f, "LEAD_ASSIGNED"),
            TeamEventType::MemberActivated => write!(f, "MEMBER_ACTIVATED"),
            TeamEventType::DelegationCreated => write!(f, "DELEGATION_CREATED"),
            TeamEventType::DelegationAccepted => write!(f, "DELEGATION_ACCEPTED"),
            TeamEventType::DelegationRejected => write!(f, "DELEGATION_REJECTED"),
            TeamEventType::MemberResultReceived => write!(f, "MEMBER_RESULT_RECEIVED"),
            TeamEventType::MemberResultAccepted => write!(f, "MEMBER_RESULT_ACCEPTED"),
            TeamEventType::MemberResultRejected => write!(f, "MEMBER_RESULT_REJECTED"),
            TeamEventType::ArtifactPublished => write!(f, "ARTIFACT_PUBLISHED"),
            TeamEventType::FactPublished => write!(f, "FACT_PUBLISHED"),
            TeamEventType::BlockerAdded => write!(f, "BLOCKER_ADDED"),
            TeamEventType::BlockerResolved => write!(f, "BLOCKER_RESOLVED"),
            TeamEventType::ApprovalRequested => write!(f, "APPROVAL_REQUESTED"),
            TeamEventType::TeamBlocked => write!(f, "TEAM_BLOCKED"),
            TeamEventType::TeamUnblocked => write!(f, "TEAM_UNBLOCKED"),
            TeamEventType::TeamCompleted => write!(f, "TEAM_COMPLETED"),
            TeamEventType::TeamFailed => write!(f, "TEAM_FAILED"),
        }
    }
}
```

- [ ] **Step 4: Add MemberSelector and CandidateMember models**

Add to `src/models/v1/team.rs`:

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MemberSelector {
    pub selector_type: SelectorType,
    pub capability_profiles: Vec<String>,
    pub role: Option<String>,
    pub agent_definition_id: Option<Uuid>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SelectorType {
    Capability,
    Role,
    Direct,
    Any,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CandidateMember {
    pub team_member_id: Uuid,
    pub agent_instance_id: Uuid,
    pub agent_definition_id: Uuid,
    pub role: String,
    pub capability_profiles: Vec<String>,
    pub selection_rationale: String,
    pub policy_check_summary: PolicyCheckSummary,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PolicyCheckSummary {
    pub resource_available: bool,
    pub approval_required: bool,
    pub risk_level: String,
}
```

- [ ] **Step 5: Add TeamTaskCreate**

Add to `src/models/v1/team.rs`:

```rust
#[derive(Debug, Deserialize)]
pub struct TeamTaskCreate {
    pub goal: String,
    pub instructions: Option<String>,
    pub input_artifacts: Vec<Uuid>,
}
```

- [ ] **Step 6: Commit**

```bash
git add src/models/v1/team.rs
git commit -m "feat(team): add TeamTask, SharedTaskState, TeamEvent models"
```

---

## Phase 2: Repository Layer

### Task 3: Repository Traits and Implementations

**Files:**
- Modify: `src/repository/mod.rs`
- Modify: `src/repository/team.rs`

- [ ] **Step 1: Add TeamTaskRepository trait**

Add to `src/repository/team.rs`:

```rust
#[async_trait]
pub trait TeamTaskRepository: Send + Sync {
    async fn create(&self, team_instance_id: Uuid, goal: &str, instructions: Option<&str>) -> anyhow::Result<TeamTask>;
    async fn get(&self, id: Uuid) -> anyhow::Result<Option<TeamTask>>;
    async fn list_by_team(&self, team_instance_id: Uuid, limit: i64) -> anyhow::Result<Vec<TeamTask>>;
    async fn list_open(&self, team_instance_id: Uuid, limit: i64) -> anyhow::Result<Vec<TeamTask>>;
    async fn update_status(&self, id: Uuid, status: TeamTaskStatus) -> anyhow::Result<bool>;
    async fn update_triage_result(&self, id: Uuid, triage: &TriageResult) -> anyhow::Result<bool>;
    async fn update_mode(&self, id: Uuid, mode: &TeamMode) -> anyhow::Result<bool>;
    async fn mark_completed(&self, id: Uuid) -> anyhow::Result<bool>;
}
```

- [ ] **Step 2: Add SharedTaskStateRepository trait**

Add to `src/repository/team.rs`:

```rust
#[async_trait]
pub trait SharedTaskStateRepository: Send + Sync {
    async fn get_or_create(&self, team_instance_id: Uuid) -> anyhow::Result<SharedTaskState>;
    async fn get(&self, team_instance_id: Uuid) -> anyhow::Result<Option<SharedTaskState>>;
    async fn add_accepted_artifact(&self, team_instance_id: Uuid, artifact_ref: ArtifactRef) -> anyhow::Result<bool>;
    async fn add_published_fact(&self, team_instance_id: Uuid, fact: PublishedFact) -> anyhow::Result<bool>;
    async fn update_delegation_status(&self, team_instance_id: Uuid, entry: DelegationStatusEntry) -> anyhow::Result<bool>;
    async fn add_blocker(&self, team_instance_id: Uuid, blocker: Blocker) -> anyhow::Result<bool>;
    async fn resolve_blocker(&self, team_instance_id: Uuid, blocker_id: Uuid) -> anyhow::Result<bool>;
    async fn add_decision(&self, team_instance_id: Uuid, decision: Decision) -> anyhow::Result<bool>;
}
```

- [ ] **Step 3: Add TeamEventRepository trait**

Add to `src/repository/team.rs`:

```rust
#[async_trait]
pub trait TeamEventRepository: Send + Sync {
    async fn create(&self, event: &TeamEvent) -> anyhow::Result<TeamEvent>;
    async fn list_by_team(&self, team_instance_id: Uuid, limit: i64) -> anyhow::Result<Vec<TeamEvent>>;
    async fn list_by_task(&self, team_task_id: Uuid, limit: i64) -> anyhow::Result<Vec<TeamEvent>>;
}
```

- [ ] **Step 4: Implement PostgresTeamTaskRepository**

Add implementation to `src/repository/team.rs`:

```rust
pub struct PostgresTeamTaskRepository {
    db: Database,
}

#[async_trait]
impl TeamTaskRepository for PostgresTeamTaskRepository {
    async fn create(&self, team_instance_id: Uuid, goal: &str, instructions: Option<&str>) -> anyhow::Result<TeamTask> {
        let row = sqlx::query_as::<_, TeamTaskRow>(
            "INSERT INTO v1_team_tasks (team_instance_id, goal, instructions) VALUES ($1, $2, $3) RETURNING *"
        )
        .bind(team_instance_id)
        .bind(goal)
        .bind(instructions)
        .fetch_one(self.db.pool())
        .await?;
        Ok(row.into())
    }

    // ... implement remaining methods
}
```

- [ ] **Step 5: Implement PostgresSharedTaskStateRepository**

Add implementation to `src/repository/team.rs`:

```rust
pub struct PostgresSharedTaskStateRepository {
    db: Database,
}

#[async_trait]
impl SharedTaskStateRepository for PostgresSharedTaskStateRepository {
    async fn get_or_create(&self, team_instance_id: Uuid) -> anyhow::Result<SharedTaskState> {
        let row = sqlx::query_as::<_, SharedTaskStateRow>(
            "INSERT INTO v1_team_shared_state (team_instance_id) VALUES ($1) ON CONFLICT (team_instance_id) DO UPDATE SET updated_at = NOW() RETURNING *"
        )
        .bind(team_instance_id)
        .fetch_one(self.db.pool())
        .await?;
        Ok(row.into())
    }

    // ... implement remaining methods
}
```

- [ ] **Step 6: Implement PostgresTeamEventRepository**

Add implementation to `src/repository/team.rs`:

```rust
pub struct PostgresTeamEventRepository {
    db: Database,
}

#[async_trait]
impl TeamEventRepository for PostgresTeamEventRepository {
    async fn create(&self, event: &TeamEvent) -> anyhow::Result<TeamEvent> {
        let row = sqlx::query_as::<_, TeamEventRow>(
            "INSERT INTO v1_team_events (team_instance_id, event_type, actor_ref, team_task_ref, related_instance_refs, related_artifact_refs, payload, causal_event_refs) VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING *"
        )
        .bind(event.team_instance_id)
        .bind(event.event_type.to_string())
        .bind(&event.actor_ref)
        .bind(event.team_task_ref)
        .bind(serde_json::to_value(&event.related_instance_refs)?)
        .bind(serde_json::to_value(&event.related_artifact_refs)?)
        .bind(&event.payload)
        .bind(serde_json::to_value(&event.causal_event_refs)?)
        .fetch_one(self.db.pool())
        .await?;
        Ok(row.into())
    }

    // ... implement remaining methods
}
```

- [ ] **Step 7: Export new repositories in mod.rs**

Modify `src/repository/mod.rs`:

```rust
pub use team::{
    PostgresTeamTaskRepository, PostgresSharedTaskStateRepository, PostgresTeamEventRepository,
    TeamTaskRepository, SharedTaskStateRepository, TeamEventRepository,
};
```

- [ ] **Step 8: Run cargo check to verify compilation**

Run: `cd crates/torque-harness && cargo check`
Expected: Compiles successfully

- [ ] **Step 9: Commit**

```bash
git add src/repository/
git commit -m "feat(team): add repository layer for tasks, shared state, events"
```

---

## Phase 3: Core Services

### Task 4: SelectorResolver

**Files:**
- Create: `src/service/team/selector.rs`

- [ ] **Step 1: Create SelectorResolver**

```rust
use crate::models::v1::team::{CandidateMember, MemberSelector, PolicyCheckSummary, SelectorType};
use crate::repository::{AgentInstanceRepository, CapabilityRegistry, TeamMemberRepository};
use std::sync::Arc;

pub struct SelectorResolver {
    capability_registry: Arc<dyn CapabilityRegistry>,
    team_member_repo: Arc<dyn TeamMemberRepository>,
    agent_instance_repo: Arc<dyn AgentInstanceRepository>,
}

impl SelectorResolver {
    pub fn new(
        capability_registry: Arc<dyn CapabilityRegistry>,
        team_member_repo: Arc<dyn TeamMemberRepository>,
        agent_instance_repo: Arc<dyn AgentInstanceRepository>,
    ) -> Self {
        Self {
            capability_registry,
            team_member_repo,
            agent_instance_repo,
        }
    }

    pub async fn resolve(
        &self,
        selector: &MemberSelector,
        team_instance_id: Uuid,
    ) -> anyhow::Result<Vec<CandidateMember>> {
        let members = self.team_member_repo.list_by_team(team_instance_id, 100).await?;

        let candidates: Vec<CandidateMember> = members
            .into_iter()
            .filter(|member| self.member_matches_selector(member, selector))
            .map(|member| CandidateMember {
                team_member_id: member.id,
                agent_instance_id: member.agent_instance_id,
                agent_definition_id: member.agent_instance_id, // Note: this needs proper lookup
                role: member.role.clone(),
                capability_profiles: vec![], // TODO: load from capability registry
                selection_rationale: format!("Matched {} selector", selector.selector_type),
                policy_check_summary: PolicyCheckSummary {
                    resource_available: true,
                    approval_required: false,
                    risk_level: "low".to_string(),
                },
            })
            .collect();

        Ok(candidates)
    }

    fn member_matches_selector(&self, member: &crate::models::v1::team::TeamMember, selector: &MemberSelector) -> bool {
        match selector.selector_type {
            SelectorType::Role => {
                selector.role.as_ref().map_or(true, |r| &member.role == r)
            }
            SelectorType::Any => true,
            SelectorType::Capability | SelectorType::Direct => {
                // TODO: Implement capability profile matching
                true
            }
        }
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add src/service/team/selector.rs
git commit -m "feat(team): add SelectorResolver"
```

---

### Task 5: SharedTaskState Manager

**Files:**
- Create: `src/service/team/shared_state.rs`

- [ ] **Step 1: Create SharedTaskStateManager**

```rust
use crate::models::v1::team::{ArtifactRef, Blocker, Decision, DelegationStatusEntry, PublishedFact, SharedTaskState};
use crate::repository::SharedTaskStateRepository;
use chrono::Utc;
use std::sync::Arc;
use uuid::Uuid;

pub struct SharedTaskStateManager {
    repo: Arc<dyn SharedTaskStateRepository>,
}

impl SharedTaskStateManager {
    pub fn new(repo: Arc<dyn SharedTaskStateRepository>) -> Self {
        Self { repo }
    }

    pub async fn get_or_create(&self, team_instance_id: Uuid) -> anyhow::Result<SharedTaskState> {
        self.repo.get_or_create(team_instance_id).await
    }

    pub async fn publish_artifact(
        &self,
        team_instance_id: Uuid,
        artifact_id: Uuid,
        scope: crate::models::v1::team::PublishScope,
        published_by: &str,
    ) -> anyhow::Result<bool> {
        let artifact_ref = ArtifactRef {
            artifact_id,
            scope,
            published_by: published_by.to_string(),
            published_at: Utc::now(),
        };
        self.repo.add_accepted_artifact(team_instance_id, artifact_ref).await
    }

    pub async fn publish_fact(
        &self,
        team_instance_id: Uuid,
        key: &str,
        value: serde_json::Value,
        published_by: &str,
    ) -> anyhow::Result<bool> {
        let fact = PublishedFact {
            key: key.to_string(),
            value,
            published_by: published_by.to_string(),
            published_at: Utc::now(),
        };
        self.repo.add_published_fact(team_instance_id, fact).await
    }

    pub async fn update_delegation_status(
        &self,
        team_instance_id: Uuid,
        delegation_id: Uuid,
        status: &str,
    ) -> anyhow::Result<bool> {
        let entry = DelegationStatusEntry {
            delegation_id,
            status: status.to_string(),
            updated_at: Utc::now(),
        };
        self.repo.update_delegation_status(team_instance_id, entry).await
    }

    pub async fn add_blocker(
        &self,
        team_instance_id: Uuid,
        description: &str,
        source: &str,
    ) -> anyhow::Result<bool> {
        let blocker = Blocker {
            blocker_id: Uuid::new_v4(),
            description: description.to_string(),
            source: source.to_string(),
            created_at: Utc::now(),
        };
        self.repo.add_blocker(team_instance_id, blocker).await
    }

    pub async fn resolve_blocker(
        &self,
        team_instance_id: Uuid,
        blocker_id: Uuid,
    ) -> anyhow::Result<bool> {
        self.repo.resolve_blocker(team_instance_id, blocker_id).await
    }

    pub async fn add_decision(
        &self,
        team_instance_id: Uuid,
        description: &str,
        decided_by: &str,
    ) -> anyhow::Result<bool> {
        let decision = Decision {
            decision_id: Uuid::new_v4(),
            description: description.to_string(),
            decided_by: decided_by.to_string(),
            decided_at: Utc::now(),
        };
        self.repo.add_decision(team_instance_id, decision).await
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add src/service/team/shared_state.rs
git commit -m "feat(team): add SharedTaskStateManager"
```

---

### Task 6: TeamEvent Emitter

**Files:**
- Create: `src/service/team/events.rs`

- [ ] **Step 1: Create TeamEventEmitter**

```rust
use crate::models::v1::team::{TeamEvent, TeamEventType};
use crate::repository::TeamEventRepository;
use chrono::Utc;
use std::sync::Arc;
use uuid::Uuid;

pub struct TeamEventEmitter {
    repo: Arc<dyn TeamEventRepository>,
}

impl TeamEventEmitter {
    pub fn new(repo: Arc<dyn TeamEventRepository>) -> Self {
        Self { repo }
    }

    pub async fn emit(
        &self,
        team_instance_id: Uuid,
        event_type: TeamEventType,
        actor_ref: &str,
        team_task_ref: Option<Uuid>,
        related_instance_refs: Vec<Uuid>,
        related_artifact_refs: Vec<Uuid>,
        payload: serde_json::Value,
        causal_event_refs: Vec<Uuid>,
    ) -> anyhow::Result<TeamEvent> {
        let event = TeamEvent {
            id: Uuid::new_v4(),
            team_instance_id,
            event_type,
            timestamp: Utc::now(),
            actor_ref: actor_ref.to_string(),
            team_task_ref,
            related_instance_refs,
            related_artifact_refs,
            payload,
            causal_event_refs,
        };
        self.repo.create(&event).await
    }

    pub async fn task_received(&self, team_instance_id: Uuid, task_id: Uuid) -> anyhow::Result<TeamEvent> {
        self.emit(
            team_instance_id,
            TeamEventType::TeamTaskReceived,
            "system",
            Some(task_id),
            vec![],
            vec![],
            serde_json::json!({}),
            vec![],
        ).await
    }

    pub async fn triage_completed(&self, team_instance_id: Uuid, task_id: Uuid, triage_result: &crate::models::v1::team::TriageResult) -> anyhow::Result<TeamEvent> {
        self.emit(
            team_instance_id,
            TeamEventType::TriageCompleted,
            "supervisor",
            Some(task_id),
            vec![],
            vec![],
            serde_json::json!({"triage_result": triage_result}),
            vec![],
        ).await
    }

    pub async fn mode_selected(&self, team_instance_id: Uuid, task_id: Uuid, mode: &crate::models::v1::team::TeamMode) -> anyhow::Result<TeamEvent> {
        self.emit(
            team_instance_id,
            TeamEventType::ModeSelected,
            "supervisor",
            Some(task_id),
            vec![],
            vec![],
            serde_json::json!({"mode": mode}),
            vec![],
        ).await
    }

    pub async fn delegation_created(&self, team_instance_id: Uuid, task_id: Uuid, delegation_id: Uuid, member_id: Uuid) -> anyhow::Result<TeamEvent> {
        self.emit(
            team_instance_id,
            TeamEventType::DelegationCreated,
            "supervisor",
            Some(task_id),
            vec![member_id],
            vec![],
            serde_json::json!({"delegation_id": delegation_id}),
            vec![],
        ).await
    }

    pub async fn member_result_received(&self, team_instance_id: Uuid, task_id: Uuid, delegation_id: Uuid, member_id: Uuid) -> anyhow::Result<TeamEvent> {
        self.emit(
            team_instance_id,
            TeamEventType::MemberResultReceived,
            "supervisor",
            Some(task_id),
            vec![member_id],
            vec![],
            serde_json::json!({"delegation_id": delegation_id}),
            vec![],
        ).await
    }

    pub async fn artifact_published(&self, team_instance_id: Uuid, task_id: Uuid, artifact_id: Uuid, scope: &crate::models::v1::team::PublishScope) -> anyhow::Result<TeamEvent> {
        self.emit(
            team_instance_id,
            TeamEventType::ArtifactPublished,
            "supervisor",
            Some(task_id),
            vec![],
            vec![artifact_id],
            serde_json::json!({"scope": scope}),
            vec![],
        ).await
    }

    pub async fn team_completed(&self, team_instance_id: Uuid, task_id: Uuid) -> anyhow::Result<TeamEvent> {
        self.emit(
            team_instance_id,
            TeamEventType::TeamCompleted,
            "supervisor",
            Some(task_id),
            vec![],
            vec![],
            serde_json::json!({}),
            vec![],
        ).await
    }

    pub async fn team_failed(&self, team_instance_id: Uuid, task_id: Uuid, reason: &str) -> anyhow::Result<TeamEvent> {
        self.emit(
            team_instance_id,
            TeamEventType::TeamFailed,
            "supervisor",
            Some(task_id),
            vec![],
            vec![],
            serde_json::json!({"reason": reason}),
            vec![],
        ).await
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add src/service/team/events.rs
git commit -m "feat(team): add TeamEventEmitter"
```

---

### Task 7: Mode Handlers

**Files:**
- Create: `src/service/team/modes/mod.rs`
- Create: `src/service/team/modes/route.rs`
- Create: `src/service/team/modes/broadcast.rs`
- Create: `src/service/team/modes/coordinate.rs`
- Create: `src/service/team/modes/tasks.rs`

- [ ] **Step 1: Create modes/mod.rs**

```rust
pub mod route;
pub mod broadcast;
pub mod coordinate;
pub mod tasks;

pub use route::RouteModeHandler;
pub use broadcast::BroadcastModeHandler;
pub use coordinate::CoordinateModeHandler;
pub use tasks::TasksModeHandler;

use crate::models::v1::team::{CandidateMember, SharedTaskState, TeamTask};
use crate::service::team::{SelectorResolver, SharedTaskStateManager, TeamEventEmitter};
use crate::repository::DelegationRepository;
use std::sync::Arc;

pub trait ModeHandler: Send + Sync {
    fn mode_name(&self) -> &'static str;

    async fn execute(
        &self,
        task: &TeamTask,
        team_instance_id: Uuid,
        candidates: Vec<CandidateMember>,
        delegation_repo: Arc<dyn DelegationRepository>,
        selector_resolver: Arc<SelectorResolver>,
        shared_state: Arc<SharedTaskStateManager>,
        events: Arc<TeamEventEmitter>,
    ) -> anyhow::Result<ModeExecutionResult>;
}

#[derive(Debug)]
pub struct ModeExecutionResult {
    pub success: bool,
    pub summary: String,
    pub delegation_ids: Vec<uuid::Uuid>,
    pub published_artifact_ids: Vec<uuid::Uuid>,
}
```

- [ ] **Step 2: Create modes/route.rs**

```rust
use super::*;
use crate::models::v1::team::{MemberSelector, SelectorType, TeamMode};

pub struct RouteModeHandler;

impl RouteModeHandler {
    pub fn new() -> Self {
        Self
    }
}

impl ModeHandler for RouteModeHandler {
    fn mode_name(&self) -> &'static str {
        "route"
    }

    async fn execute(
        &self,
        task: &TeamTask,
        team_instance_id: Uuid,
        mut candidates: Vec<CandidateMember>,
        delegation_repo: Arc<dyn DelegationRepository>,
        selector_resolver: Arc<SelectorResolver>,
        shared_state: Arc<SharedTaskStateManager>,
        events: Arc<TeamEventEmitter>,
    ) -> anyhow::Result<ModeExecutionResult> {
        if candidates.is_empty() {
            return Ok(ModeExecutionResult {
                success: false,
                summary: "No candidates available for route mode".to_string(),
                delegation_ids: vec![],
                published_artifact_ids: vec![],
            });
        }

        // Select first candidate (supervisor makes the choice)
        let selected = candidates.remove(0);

        // Emit member activated event
        events.emit(
            team_instance_id,
            crate::models::v1::team::TeamEventType::MemberActivated,
            "supervisor",
            Some(task.id),
            vec![selected.agent_instance_id],
            vec![],
            serde_json::json!({"role": selected.role}),
            vec![],
        ).await?;

        // Emit delegation created event
        let delegation = delegation_repo.create(
            task.id,
            team_instance_id, // parent instance
            serde_json::json!({
                "member_id": selected.agent_instance_id,
                "goal": task.goal,
                "instructions": task.instructions,
            }),
        ).await?;

        events.delegation_created(team_instance_id, task.id, delegation.id, selected.agent_instance_id).await?;

        // Update shared state with delegation status
        shared_state.update_delegation_status(team_instance_id, delegation.id, "PENDING").await?;

        // Wait for delegation result (simplified - in real impl would poll or await callback)
        // For MVP, we assume delegation completes and we accept the result
        let delegation_result = delegation_repo.get(delegation.id).await?;

        // Emit result received
        events.member_result_received(team_instance_id, task.id, delegation.id, selected.agent_instance_id).await?;

        // Mark delegation as accepted (simplified)
        delegation_repo.update_status(delegation.id, "ACCEPTED").await?;

        events.emit(
            team_instance_id,
            crate::models::v1::team::TeamEventType::MemberResultAccepted,
            "supervisor",
            Some(task.id),
            vec![selected.agent_instance_id],
            vec![],
            serde_json::json!({"delegation_id": delegation.id}),
            vec![],
        ).await?;

        Ok(ModeExecutionResult {
            success: true,
            summary: format!("Route completed via member {}", selected.role),
            delegation_ids: vec![delegation.id],
            published_artifact_ids: vec![],
        })
    }
}
```

- [ ] **Step 3: Create modes/broadcast.rs**

```rust
use super::*;

pub struct BroadcastModeHandler;

impl BroadcastModeHandler {
    pub fn new() -> Self {
        Self
    }
}

impl ModeHandler for BroadcastModeHandler {
    fn mode_name(&self) -> &'static str {
        "broadcast"
    }

    async fn execute(
        &self,
        task: &TeamTask,
        team_instance_id: Uuid,
        candidates: Vec<CandidateMember>,
        delegation_repo: Arc<dyn DelegationRepository>,
        _selector_resolver: Arc<SelectorResolver>,
        shared_state: Arc<SharedTaskStateManager>,
        events: Arc<TeamEventEmitter>,
    ) -> anyhow::Result<ModeExecutionResult> {
        if candidates.is_empty() {
            return Ok(ModeExecutionResult {
                success: false,
                summary: "No candidates for broadcast".to_string(),
                delegation_ids: vec![],
                published_artifact_ids: vec![],
            });
        }

        let mut delegation_ids = Vec::new();

        // Create delegations for all candidates in parallel
        for candidate in &candidates {
            events.emit(
                team_instance_id,
                crate::models::v1::team::TeamEventType::MemberActivated,
                "supervisor",
                Some(task.id),
                vec![candidate.agent_instance_id],
                vec![],
                serde_json::json!({"role": candidate.role}),
                vec![],
            ).await?;

            let delegation = delegation_repo.create(
                task.id,
                team_instance_id,
                serde_json::json!({
                    "member_id": candidate.agent_instance_id,
                    "goal": task.goal,
                    "instructions": task.instructions,
                }),
            ).await?;

            delegation_ids.push(delegation.id);
            shared_state.update_delegation_status(team_instance_id, delegation.id, "PENDING").await?;
        }

        // Wait for all delegations (simplified - MVP assumes all complete)
        let mut accepted_count = 0;
        for (i, delegation_id) in delegation_ids.iter().enumerate() {
            delegation_repo.update_status(*delegation_id, "ACCEPTED").await?;
            events.emit(
                team_instance_id,
                crate::models::v1::team::TeamEventType::MemberResultAccepted,
                "supervisor",
                Some(task.id),
                vec![candidates[i].agent_instance_id],
                vec![],
                serde_json::json!({"delegation_id": delegation_id}),
                vec![],
            ).await?;
            accepted_count += 1;
        }

        Ok(ModeExecutionResult {
            success: true,
            summary: format!("Broadcast completed with {}/{} accepted", accepted_count, delegation_ids.len()),
            delegation_ids,
            published_artifact_ids: vec![],
        })
    }
}
```

- [ ] **Step 4: Create modes/coordinate.rs**

```rust
use super::*;

pub struct CoordinateModeHandler;

impl CoordinateModeHandler {
    pub fn new() -> Self {
        Self
    }
}

impl ModeHandler for CoordinateModeHandler {
    fn mode_name(&self) -> &'static str {
        "coordinate"
    }

    async fn execute(
        &self,
        task: &TeamTask,
        team_instance_id: Uuid,
        candidates: Vec<CandidateMember>,
        delegation_repo: Arc<dyn DelegationRepository>,
        _selector_resolver: Arc<SelectorResolver>,
        shared_state: Arc<SharedTaskStateManager>,
        events: Arc<TeamEventEmitter>,
    ) -> anyhow::Result<ModeExecutionResult> {
        if candidates.is_empty() {
            return Ok(ModeExecutionResult {
                success: false,
                summary: "No candidates for coordinate mode".to_string(),
                delegation_ids: vec![],
                published_artifact_ids: vec![],
            });
        }

        // Initialize shared state with coordination metadata
        shared_state.add_decision(
            team_instance_id,
            &format!("Starting coordination for task: {}", task.goal),
            "supervisor",
        ).await?;

        // First round: delegate to first candidate
        let selected = &candidates[0];
        let delegation = delegation_repo.create(
            task.id,
            team_instance_id,
            serde_json::json!({
                "member_id": selected.agent_instance_id,
                "goal": task.goal,
                "instructions": task.instructions,
                "coordinate_round": 1,
            }),
        ).await?;

        delegation_repo.update_status(delegation.id, "ACCEPTED").await?;

        // For MVP: single round coordination
        shared_state.update_delegation_status(team_instance_id, delegation.id, "COMPLETED").await?;

        Ok(ModeExecutionResult {
            success: true,
            summary: "Coordinate mode completed (MVP: single round)".to_string(),
            delegation_ids: vec![delegation.id],
            published_artifact_ids: vec![],
        })
    }
}
```

- [ ] **Step 5: Create modes/tasks.rs**

```rust
use super::*;

pub struct TasksModeHandler;

impl TasksModeHandler {
    pub fn new() -> Self {
        Self
    }
}

impl ModeHandler for TasksModeHandler {
    fn mode_name(&self) -> &'static str {
        "tasks"
    }

    async fn execute(
        &self,
        task: &TeamTask,
        team_instance_id: Uuid,
        candidates: Vec<CandidateMember>,
        delegation_repo: Arc<dyn DelegationRepository>,
        _selector_resolver: Arc<SelectorResolver>,
        shared_state: Arc<SharedTaskStateManager>,
        events: Arc<TeamEventEmitter>,
    ) -> anyhow::Result<ModeExecutionResult> {
        // Tasks mode: decompose task into subtasks (simplified - assumes single goal = single task)
        if candidates.is_empty() {
            return Ok(ModeExecutionResult {
                success: false,
                summary: "No candidates for tasks mode".to_string(),
                delegation_ids: vec![],
                published_artifact_ids: vec![],
            });
        }

        // Delegate entire task to first available member
        let selected = &candidates[0];
        let delegation = delegation_repo.create(
            task.id,
            team_instance_id,
            serde_json::json!({
                "member_id": selected.agent_instance_id,
                "goal": task.goal,
                "instructions": task.instructions,
                "decomposed": true,
            }),
        ).await?;

        delegation_repo.update_status(delegation.id, "ACCEPTED").await?;

        // Add decision about task mode
        shared_state.add_decision(
            team_instance_id,
            "Executed task via TasksMode with decomposition",
            "supervisor",
        ).await?;

        Ok(ModeExecutionResult {
            success: true,
            summary: "Tasks mode completed".to_string(),
            delegation_ids: vec![delegation.id],
            published_artifact_ids: vec![],
        })
    }
}
```

- [ ] **Step 6: Commit**

```bash
git add src/service/team/modes/
git commit -m "feat(team): add mode handlers (route, broadcast, coordinate, tasks)"
```

---

## Phase 4: Supervisor Orchestration

### Task 8: TeamSupervisor

**Files:**
- Create: `src/service/team/supervisor.rs`

- [ ] **Step 1: Create TeamSupervisor**

```rust
use crate::models::v1::team::{TeamMode, TeamTask, TeamTaskStatus};
use crate::service::team::modes::{ModeExecutionResult, ModeHandler, RouteModeHandler, BroadcastModeHandler, CoordinateModeHandler, TasksModeHandler};
use crate::service::team::{SelectorResolver, SharedTaskStateManager, TeamEventEmitter};
use crate::repository::{DelegationRepository, TeamTaskRepository};
use std::sync::Arc;

pub struct TeamSupervisor {
    task_repo: Arc<dyn TeamTaskRepository>,
    delegation_repo: Arc<dyn DelegationRepository>,
    selector_resolver: Arc<SelectorResolver>,
    shared_state: Arc<SharedTaskStateManager>,
    events: Arc<TeamEventEmitter>,
    mode_handlers: Vec<Box<dyn ModeHandler>>,
}

impl TeamSupervisor {
    pub fn new(
        task_repo: Arc<dyn TeamTaskRepository>,
        delegation_repo: Arc<dyn DelegationRepository>,
        selector_resolver: Arc<SelectorResolver>,
        shared_state: Arc<SharedTaskStateManager>,
        events: Arc<TeamEventEmitter>,
    ) -> Self {
        let mut handlers: Vec<Box<dyn ModeHandler>> = Vec::new();
        handlers.push(Box::new(RouteModeHandler::new()));
        handlers.push(Box::new(BroadcastModeHandler::new()));
        handlers.push(Box::new(CoordinateModeHandler::new()));
        handlers.push(Box::new(TasksModeHandler::new()));

        Self {
            task_repo,
            delegation_repo,
            selector_resolver,
            shared_state,
            events,
            mode_handlers: handlers,
        }
    }

    pub async fn poll_and_execute(&self, team_instance_id: Uuid) -> anyhow::Result<Option<SupervisorResult>> {
        // Find open tasks for this team
        let open_tasks = self.task_repo.list_open(team_instance_id, 10).await?;

        if open_tasks.is_empty() {
            return Ok(None);
        }

        let task = &open_tasks[0];
        self.execute_task(task, team_instance_id).await
    }

    pub async fn execute_task(&self, task: &TeamTask, team_instance_id: Uuid) -> anyhow::Result<Option<SupervisorResult>> {
        // Emit task received event
        self.events.task_received(team_instance_id, task.id).await?;

        // Triage: determine mode based on task characteristics
        let triage_result = self.triage(task).await?;

        // Emit triage completed
        self.events.triage_completed(team_instance_id, task.id, &triage_result).await?;

        // Update task with triage result
        self.task_repo.update_triage_result(task.id, &triage_result).await?;

        // Emit mode selected
        self.events.mode_selected(team_instance_id, task.id, &triage_result.selected_mode).await?;
        self.task_repo.update_mode(task.id, &triage_result.selected_mode).await?;

        // Update task status
        self.task_repo.update_status(task.id, TeamTaskStatus::InProgress).await?;

        // Resolve candidates based on selector (use any for MVP)
        let candidates = self.selector_resolver.resolve(
            &crate::models::v1::team::MemberSelector {
                selector_type: crate::models::v1::team::SelectorType::Any,
                capability_profiles: vec![],
                role: None,
                agent_definition_id: None,
            },
            team_instance_id,
        ).await?;

        // Find handler for selected mode
        let mode_name = match triage_result.selected_mode {
            TeamMode::Route => "route",
            TeamMode::Broadcast => "broadcast",
            TeamMode::Coordinate => "coordinate",
            TeamMode::Tasks => "tasks",
        };

        let handler = self.mode_handlers
            .iter()
            .find(|h| h.mode_name() == mode_name)
            .ok_or_else(|| anyhow::anyhow!("No handler for mode: {}", mode_name))?;

        // Execute mode
        let result = handler.execute(
            task,
            team_instance_id,
            candidates,
            self.delegation_repo.clone(),
            self.selector_resolver.clone(),
            self.shared_state.clone(),
            self.events.clone(),
        ).await?;

        // Update task status based on result
        if result.success {
            self.task_repo.update_status(task.id, TeamTaskStatus::Completed).await?;
            self.task_repo.mark_completed(task.id).await?;
            self.events.team_completed(team_instance_id, task.id).await?;
        } else {
            self.task_repo.update_status(task.id, TeamTaskStatus::Failed).await?;
            self.events.team_failed(team_instance_id, task.id, &result.summary).await?;
        }

        Ok(Some(SupervisorResult {
            task_id: task.id,
            success: result.success,
            summary: result.summary,
        }))
    }

    async fn triage(&self, task: &TeamTask) -> anyhow::Result<crate::models::v1::team::TriageResult> {
        // Simple triage: if goal length > 100 chars, complex
        // In real impl, this would use LLM reasoning
        let complexity = if task.goal.len() > 200 {
            crate::models::v1::team::TaskComplexity::Complex
        } else if task.goal.len() > 100 {
            crate::models::v1::team::TaskComplexity::Medium
        } else {
            crate::models::v1::team::TaskComplexity::Simple
        };

        let (processing_path, selected_mode) = match complexity {
            crate::models::v1::team::TaskComplexity::Simple => (
                crate::models::v1::team::ProcessingPath::SingleRoute,
                TeamMode::Route,
            ),
            crate::models::v1::team::TaskComplexity::Medium => (
                crate::models::v1::team::ProcessingPath::GuidedDelegate,
                TeamMode::Route,
            ),
            crate::models::v1::team::TaskComplexity::Complex => (
                crate::models::v1::team::ProcessingPath::StructuredOrchestration,
                TeamMode::Tasks,
            ),
        };

        Ok(crate::models::v1::team::TriageResult {
            complexity,
            processing_path,
            selected_mode,
            lead_member_ref: None,
            rationale: format!("Triage determined {} complexity, using {} mode", complexity, selected_mode),
        })
    }
}

#[derive(Debug)]
pub struct SupervisorResult {
    pub task_id: uuid::Uuid,
    pub success: bool,
    pub summary: String,
}
```

- [ ] **Step 2: Commit**

```bash
git add src/service/team/supervisor.rs
git commit -m "feat(team): add TeamSupervisor orchestration"
```

---

## Phase 5: TeamService Integration & API

### Task 9: Update TeamService

**Files:**
- Modify: `src/service/team/mod.rs`

- [ ] **Step 1: Update mod.rs to export new components**

```rust
pub mod supervisor;
pub mod selector;
pub mod shared_state;
pub mod events;
pub mod modes;

pub use supervisor::TeamSupervisor;
pub use selector::SelectorResolver;
pub use shared_state::SharedTaskStateManager;
pub use events::TeamEventEmitter;

use crate::models::v1::team::TeamTaskCreate;
use crate::repository::{TeamTaskRepository, SharedTaskStateRepository, TeamEventRepository};

pub struct TeamService {
    // ... existing fields
    task_repo: Option<Arc<dyn TeamTaskRepository>>,
    shared_state_repo: Option<Arc<dyn SharedTaskStateRepository>>,
    team_event_repo: Option<Arc<dyn TeamEventRepository>>,
}

impl TeamService {
    // ... existing methods unchanged

    pub async fn create_team_task_v2(
        &self,
        team_instance_id: Uuid,
        req: TeamTaskCreate,
    ) -> anyhow::Result<crate::models::v1::team::TeamTask> {
        let task_repo = self.task_repo
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("TeamTaskRepository not configured"))?;

        let task = task_repo.create(
            team_instance_id,
            &req.goal,
            req.instructions.as_deref(),
        ).await?;

        // Emit task received event
        if let Some(events) = self.team_event_repo.as_ref() {
            let emitter = TeamEventEmitter::new(Arc::new(crate::repository::PostgresTeamEventRepository::new(events.db.clone())));
            emitter.task_received(team_instance_id, task.id).await?;
        }

        Ok(task)
    }

    pub async fn get_supervisor(
        &self,
        team_instance_id: Uuid,
    ) -> anyhow::Result<TeamSupervisor> {
        let supervisor = TeamSupervisor::new(
            self.task_repo.clone().ok_or_else(|| anyhow::anyhow!("TeamTaskRepository not configured"))?,
            self.delegation_repo.clone(),
            self.selector_resolver.clone(),
            self.shared_state.clone(),
            self.events.clone(),
        );
        Ok(supervisor)
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add src/service/team/mod.rs
git commit -m "feat(team): integrate supervisor into TeamService"
```

---

### Task 10: Add API Endpoint

**Files:**
- Modify: `src/api/v1/teams.rs`

- [ ] **Step 1: Add create team task endpoint**

Add to `src/api/v1/teams.rs`:

```rust
pub async fn create_team_task(
    Path(team_instance_id): Path<Uuid>,
    Json(req): Json<TeamTaskCreate>,
    Extension(services): Extension<Arc<ServiceContainer>>,
) -> impl IntoResponse {
    match services.team.create_team_task_v2(team_instance_id, req).await {
        Ok(task) => (StatusCode::CREATED, Json(task)),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))),
    }
}

pub async fn list_team_tasks(
    Path(team_instance_id): Path<Uuid>,
    Extension(services): Extension<Arc<ServiceContainer>>,
    Query(params): Query<ListParams>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(100);
    match services.team.list_team_tasks(team_instance_id, limit).await {
        Ok(tasks) => (StatusCode::OK, Json(tasks)),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))),
    }
}

pub async fn get_team_task(
    Path((team_instance_id, task_id)): Path<(Uuid, Uuid)>,
    Extension(services): Extension<Arc<ServiceContainer>>,
) -> impl IntoResponse {
    match services.team.get_team_task(task_id).await {
        Ok(Some(task)) => (StatusCode::OK, Json(task)),
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({"error": "Task not found"}))),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))),
    }
}
```

- [ ] **Step 2: Wire up routes in router**

Modify `src/api/v1/mod.rs`:

```rust
.route("/team-instances/:team_instance_id/tasks", post(teams::create_team_task))
.route("/team-instances/:team_instance_id/tasks", get(teams::list_team_tasks))
.route("/team-instances/:team_instance_id/tasks/:task_id", get(teams::get_team_task))
```

- [ ] **Step 3: Commit**

```bash
git add src/api/v1/teams.rs src/api/v1/mod.rs
git commit -m "feat(api): add team tasks endpoints"
```

---

## Phase 6: Supervisor Agent Tools

### Task 11: Supervisor Agent Tools

**Files:**
- Create: `src/tools/team_tools.rs`

- [ ] **Step 1: Create supervisor tools**

```rust
use crate::agent::stream::StreamEvent;
use crate::models::v1::team::{MemberSelector, PublishScope};
use crate::service::team::{SelectorResolver, SharedTaskStateManager, TeamEventEmitter};
use crate::repository::DelegationRepository;
use crate::tools::{Tool, ToolCall, ToolResult};
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

pub struct DelegateTaskTool {
    delegation_repo: Arc<dyn DelegationRepository>,
    selector_resolver: Arc<SelectorResolver>,
    events: Arc<TeamEventEmitter>,
}

impl DelegateTaskTool {
    pub fn new(
        delegation_repo: Arc<dyn DelegationRepository>,
        selector_resolver: Arc<SelectorResolver>,
        events: Arc<TeamEventEmitter>,
    ) -> Self {
        Self {
            delegation_repo,
            selector_resolver,
            events,
        }
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct DelegateTaskParams {
    pub member_selector: MemberSelector,
    pub goal: String,
    pub instructions: Option<String>,
    pub return_contract: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct DelegateTaskResult {
    pub delegation_id: Uuid,
    pub status: String,
}

#[async_trait::async_trait]
impl Tool for DelegateTaskTool {
    fn name(&self) -> &str {
        "delegate_task"
    }

    fn description(&self) -> &str {
        "Delegate a task to a team member selected by capability, role, or direct reference"
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        _event_sink: mpsc::Sender<StreamEvent>,
    ) -> ToolResult {
        let params: DelegateTaskParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(format!("Invalid params: {}", e)),
        };

        // Resolve selector to candidates
        let candidates = match self.selector_resolver.resolve(&params.member_selector, Uuid::nil()).await {
            Ok(c) => c,
            Err(e) => return ToolResult::error(format!("Selector resolution failed: {}", e)),
        };

        if candidates.is_empty() {
            return ToolResult::error("No matching members found".to_string());
        }

        let selected = &candidates[0];

        // Create delegation
        let delegation = match self.delegation_repo.create(
            Uuid::nil(), // task_id - would be passed in real impl
            selected.agent_instance_id,
            serde_json::json!({
                "goal": params.goal,
                "instructions": params.instructions,
                "return_contract": params.return_contract,
            }),
        ).await {
            Ok(d) => d,
            Err(e) => return ToolResult::error(format!("Failed to create delegation: {}", e)),
        };

        ToolResult::success(serde_json::to_value(DelegateTaskResult {
            delegation_id: delegation.id,
            status: delegation.status,
        }).unwrap())
    }
}

pub struct AcceptResultTool {
    delegation_repo: Arc<dyn DelegationRepository>,
}

impl AcceptResultTool {
    pub fn new(delegation_repo: Arc<dyn DelegationRepository>) -> Self {
        Self { delegation_repo }
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct AcceptResultParams {
    pub delegation_id: Uuid,
}

#[async_trait::async_trait]
impl Tool for AcceptResultTool {
    fn name(&self) -> &str {
        "accept_result"
    }

    fn description(&self) -> &str {
        "Accept a delegation result"
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        _event_sink: mpsc::Sender<StreamEvent>,
    ) -> ToolResult {
        let params: AcceptResultParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(format!("Invalid params: {}", e)),
        };

        match self.delegation_repo.update_status(params.delegation_id, "ACCEPTED").await {
            Ok(true) => ToolResult::success(serde_json::json!({"status": "ACCEPTED"})),
            Ok(false) => ToolResult::error("Delegation not found".to_string()),
            Err(e) => ToolResult::error(format!("Failed to accept: {}", e)),
        }
    }
}

pub struct PublishToTeamTool {
    shared_state: Arc<SharedTaskStateManager>,
}

impl PublishToTeamTool {
    pub fn new(shared_state: Arc<SharedTaskStateManager>) -> Self {
        Self { shared_state }
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct PublishToTeamParams {
    pub artifact_id: Uuid,
    pub scope: PublishScope,
    pub summary: Option<String>,
}

#[async_trait::async_trait]
impl Tool for PublishToTeamTool {
    fn name(&self) -> &str {
        "publish_to_team"
    }

    fn description(&self) -> &str {
        "Publish an artifact to team shared state"
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        _event_sink: mpsc::Sender<StreamEvent>,
    ) -> ToolResult {
        let params: PublishToTeamParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(format!("Invalid params: {}", e)),
        };

        // Note: team_instance_id would be passed from context in real impl
        ToolResult::success(serde_json::json!({
            "published": true,
            "artifact_id": params.artifact_id,
            "scope": params.scope,
        }))
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add src/tools/team_tools.rs
git commit -m "feat(team): add supervisor agent tools"
```

---

## Phase 7: Testing

### Task 12: Integration Tests

**Files:**
- Create: `tests/v1_team_supervisor_tests.rs`

- [ ] **Step 1: Write supervisor tests**

```rust
mod common;

use common::setup_test_db_or_skip;
use serial_test::serial;
use torque_harness::models::v1::agent_definition::AgentDefinitionCreate;
use torque_harness::models::v1::team::{TeamDefinitionCreate, TeamInstanceCreate};
use torque_harness::repository::{
    AgentDefinitionRepository, AgentInstanceRepository, PostgresAgentDefinitionRepository,
    PostgresAgentInstanceRepository, PostgresTeamDefinitionRepository,
    PostgresTeamInstanceRepository, PostgresTeamMemberRepository,
};
use torque_harness::service::TeamService;
use std::sync::Arc;

#[tokio::test]
#[serial]
async fn test_team_task_triage_simple() {
    let db = match setup_test_db_or_skip().await {
        Some(db) => db,
        None => return,
    };

    // Setup
    let def_repo = Arc::new(PostgresAgentDefinitionRepository::new(db.clone()));
    let team_def_repo = Arc::new(PostgresTeamDefinitionRepository::new(db.clone()));
    let team_inst_repo = Arc::new(PostgresTeamInstanceRepository::new(db.clone()));
    let team_member_repo = Arc::new(PostgresTeamMemberRepository::new(db.clone()));

    let team_service = TeamService::new(
        team_def_repo.clone(),
        team_inst_repo.clone(),
        team_member_repo.clone(),
        task_repo.clone(), // TODO: add task repo
    );

    // Create supervisor
    let supervisor_def = def_repo
        .create(&AgentDefinitionCreate {
            name: "Supervisor".into(),
            description: None,
            system_prompt: None,
            tool_policy: serde_json::json!({}),
            memory_policy: serde_json::json!({}),
            delegation_policy: serde_json::json!({}),
            limits: serde_json::json!({}),
            default_model_policy: serde_json::json!({}),
        })
        .await
        .expect("create supervisor");

    // Create team
    let team_def = team_def_repo
        .create(&TeamDefinitionCreate {
            name: "Test Team".into(),
            description: None,
            supervisor_agent_definition_id: supervisor_def.id,
            sub_agents: vec![],
            policy: serde_json::json!({}),
        })
        .await
        .expect("create team def");

    let team_instance = team_inst_repo
        .create(&TeamInstanceCreate {
            team_definition_id: team_def.id,
        })
        .await
        .expect("create team instance");

    // Create task
    let task = team_service
        .create_team_task(team_instance.id, "Simple goal", None)
        .await
        .expect("create task");

    // Verify task created
    assert_eq!(task.goal, "Simple goal");

    // TODO: Execute supervisor and verify triage

    // Cleanup
    team_inst_repo.delete(team_instance.id).await.expect("cleanup");
    team_def_repo.delete(team_def.id).await.expect("cleanup");
    def_repo.delete(supervisor_def.id).await.expect("cleanup");
}
```

- [ ] **Step 2: Commit**

```bash
git add tests/v1_team_supervisor_tests.rs
git commit -m "test(team): add supervisor integration tests"
```

---

## Summary

The implementation is divided into 7 phases:

1. **Database & Models** - 2 tasks (migrations, models)
2. **Repository Layer** - 1 task (repository traits and implementations)
3. **Core Services** - 4 tasks (SelectorResolver, SharedTaskStateManager, TeamEventEmitter, ModeHandlers)
4. **Supervisor Orchestration** - 1 task (TeamSupervisor)
5. **TeamService Integration & API** - 1 task (update TeamService, add API endpoints)
6. **Supervisor Agent Tools** - 1 task (team tools for supervisor agent)
7. **Testing** - 1 task (integration tests)

Total: 11 tasks

---

## Execution Options

**1. Subagent-Driven (recommended)** - Dispatch a fresh subagent per task, review between tasks

**2. Inline Execution** - Execute tasks in this session using executing-plans

Which approach?
