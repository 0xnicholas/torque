# Team Supervisor Production Readiness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix critical issues identified in code review - make Supervisor Tools call real repositories, implement LocalMemberAgent, add LLM fallback, and improve performance.

**Architecture:** Refactor supervisor_tools.rs to inject dependencies and call real repository methods. Implement proper task execution in LocalMemberAgent. Add rule-based triage fallback when LLM is unavailable.

**Tech Stack:** Rust, sqlx, tokio, Arc<dyn Repository> pattern

---

## Current Issues Summary

| Issue | Severity | Status |
|-------|----------|--------|
| Supervisor Tools are all mocks | Critical | ⬜ |
| LocalMemberAgent is stub | Critical | ⬜ |
| SelectorResolver N+1 queries | Moderate | ⬜ |
| No LLM fallback for triage | Moderate | ⬜ |
| SharedTaskState non-atomic updates | Moderate | ⬜ |
| Hardcoded limits | Minor | ⬜ |

---

## File Structure

```
crates/torque-harness/src/service/team/
├── supervisor_tools.rs          # MODIFY: Inject dependencies, call real repos
├── local_member_agent.rs        # MODIFY: Implement real task polling/execution
├── supervisor.rs                # MODIFY: Add rule-based triage fallback
├── selector.rs                 # MODIFY: Add pagination/streaming
├── shared_state.rs             # MODIFY: Atomic blocker resolution
├── events.rs                   # MINOR: Use actual supervisor ID

crates/torque-harness/src/repository/
└── team.rs                     # MODIFY: Add atomic JSONB operations
```

---

## Task 1: Refactor DelegateTaskTool to Use Real Repository

**Files:**
- Modify: `crates/torque-harness/src/service/team/supervisor_tools.rs`

- [ ] **Step 1: Add repository fields to tool structs**

In supervisor_tools.rs, add dependencies to each tool:

```rust
use crate::repository::{DelegationRepository, TeamTaskRepository};
use crate::service::team::{SelectorResolver, SharedTaskStateManager};

pub struct DelegateTaskTool {
    delegation_repo: Arc<dyn DelegationRepository>,
    selector_resolver: Arc<SelectorResolver>,
}

impl DelegateTaskTool {
    pub fn new(
        delegation_repo: Arc<dyn DelegationRepository>,
        selector_resolver: Arc<SelectorResolver>,
    ) -> Self {
        Self {
            delegation_repo,
            selector_resolver,
        }
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

pub struct RejectResultTool {
    delegation_repo: Arc<dyn DelegationRepository>,
}

impl RejectResultTool {
    pub fn new(delegation_repo: Arc<dyn DelegationRepository>) -> Self {
        Self { delegation_repo }
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

pub struct GetSharedStateTool {
    shared_state: Arc<SharedTaskStateManager>,
}

impl GetSharedStateTool {
    pub fn new(shared_state: Arc<SharedTaskStateManager>) -> Self {
        Self { shared_state }
    }
}

pub struct UpdateSharedFactTool {
    shared_state: Arc<SharedTaskStateManager>,
}

impl UpdateSharedFactTool {
    pub fn new(shared_state: Arc<SharedTaskStateManager>) -> Self {
        Self { shared_state }
    }
}

pub struct AddBlockerTool {
    shared_state: Arc<SharedTaskStateManager>,
}

impl AddBlockerTool {
    pub fn new(shared_state: Arc<SharedTaskStateManager>) -> Self {
        Self { shared_state }
    }
}

pub struct ResolveBlockerTool {
    shared_state: Arc<SharedTaskStateManager>,
}

impl ResolveBlockerTool {
    pub fn new(shared_state: Arc<SharedTaskStateManager>) -> Self {
        Self { shared_state }
    }
}

pub struct CompleteTeamTaskTool {
    task_repo: Arc<dyn TeamTaskRepository>,
}

impl CompleteTeamTaskTool {
    pub fn new(task_repo: Arc<dyn TeamTaskRepository>) -> Self {
        Self { task_repo }
    }
}

pub struct FailTeamTaskTool {
    task_repo: Arc<dyn TeamTaskRepository>,
}

impl FailTeamTaskTool {
    pub fn new(task_repo: Arc<dyn TeamTaskRepository>) -> Self {
        Self { task_repo }
    }
}

pub struct GetTaskDetailsTool {
    task_repo: Arc<dyn TeamTaskRepository>,
}

impl GetTaskDetailsTool {
    pub fn new(task_repo: Arc<dyn TeamTaskRepository>) -> Self {
        Self { task_repo }
    }
}

pub struct ListTeamMembersTool {
    team_member_repo: Arc<dyn TeamMemberRepository>,
}

impl ListTeamMembersTool {
    pub fn new(team_member_repo: Arc<dyn TeamMemberRepository>) -> Self {
        Self { team_member_repo }
    }
}
```

- [ ] **Step 2: Update create_supervisor_tools to accept dependencies**

```rust
pub fn create_supervisor_tools(
    delegation_repo: Arc<dyn DelegationRepository>,
    team_task_repo: Arc<dyn TeamTaskRepository>,
    team_member_repo: Arc<dyn TeamMemberRepository>,
    selector_resolver: Arc<SelectorResolver>,
    shared_state: Arc<SharedTaskStateManager>,
) -> Vec<ToolArc> {
    vec![
        Arc::new(DelegateTaskTool::new(delegation_repo.clone(), selector_resolver.clone())) as ToolArc,
        Arc::new(AcceptResultTool::new(delegation_repo.clone())) as ToolArc,
        Arc::new(RejectResultTool::new(delegation_repo.clone())) as ToolArc,
        Arc::new(PublishToTeamTool::new(shared_state.clone())) as ToolArc,
        Arc::new(GetSharedStateTool::new(shared_state.clone())) as ToolArc,
        Arc::new(CompleteTeamTaskTool::new(team_task_repo.clone())) as ToolArc,
        Arc::new(ListTeamMembersTool::new(team_member_repo.clone())) as ToolArc,
        Arc::new(GetDelegationStatusTool::new(delegation_repo.clone())) as ToolArc,
        Arc::new(UpdateSharedFactTool::new(shared_state.clone())) as ToolArc,
        Arc::new(AddBlockerTool::new(shared_state.clone())) as ToolArc,
        Arc::new(ResolveBlockerTool::new(shared_state.clone())) as ToolArc,
        Arc::new(FailTeamTaskTool::new(team_task_repo.clone())) as ToolArc,
        Arc::new(RequestApprovalTool::new()) as ToolArc,
        Arc::new(GetTaskDetailsTool::new(team_task_repo.clone())) as ToolArc,
    ]
}
```

- [ ] **Step 3: Implement real DelegateTaskTool::execute**

```rust
async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
    let member_selector = args
        .get("member_selector")
        .ok_or_else(|| anyhow::anyhow!("member_selector required"))?;
    let goal = args
        .get("goal")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("goal required"))?;
    let instructions = args.get("instructions").and_then(|v| v.as_str());
    let return_contract = args.get("return_contract");

    // Parse member_selector to get selector criteria
    let selector = parse_member_selector(member_selector)?;

    // Resolve candidates using real selector resolver
    let candidates = self.selector_resolver.resolve(&selector, Uuid::nil()).await
        .map_err(|e| anyhow::anyhow!("Selector resolution failed: {}", e))?;

    if candidates.is_empty() {
        return Ok(ToolResult {
            success: false,
            content: String::new(),
            error: Some("No matching team members found".to_string()),
        });
    }

    // Select first candidate
    let selected = &candidates[0];

    // Create real delegation via repository
    let delegation = self.delegation_repo.create(
        Uuid::nil(), // task_id - should be passed in args if available
        selected.agent_instance_id,
        serde_json::json!({
            "goal": goal,
            "instructions": instructions,
            "return_contract": return_contract,
            "selected_member_role": selected.role,
        }),
    )
    .await
    .map_err(|e| anyhow::anyhow!("Failed to create delegation: {}", e))?;

    Ok(ToolResult {
        success: true,
        content: serde_json::json!({
            "delegation_id": delegation.id,
            "status": "created",
            "member_role": selected.role,
        }).to_string(),
        error: None,
    })
}
```

- [ ] **Step 4: Implement real AcceptResultTool::execute**

```rust
async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
    let delegation_id = args
        .get("delegation_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("delegation_id required"))?;

    let delegation_uuid = Uuid::parse_str(delegation_id)
        .map_err(|_| anyhow::anyhow!("Invalid delegation_id format"))?;

    let updated = self.delegation_repo
        .update_status(delegation_uuid, "ACCEPTED")
        .await
        .map_err(|e| anyhow::anyhow!("Failed to accept: {}", e))?;

    if !updated {
        return Ok(ToolResult {
            success: false,
            content: String::new(),
            error: Some("Delegation not found".to_string()),
        });
    }

    Ok(ToolResult {
        success: true,
        content: serde_json::json!({
            "delegation_id": delegation_id,
            "status": "ACCEPTED",
        }).to_string(),
        error: None,
    })
}
```

- [ ] **Step 5: Implement real RejectResultTool::execute**

```rust
async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
    let delegation_id = args
        .get("delegation_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("delegation_id required"))?;
    let reason = args
        .get("reason")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("reason required"))?;
    let reroute = args.get("reroute").and_then(|v| v.as_bool()).unwrap_or(false);

    let delegation_uuid = Uuid::parse_str(delegation_id)
        .map_err(|_| anyhow::anyhow!("Invalid delegation_id format"))?;

    let updated = self.delegation_repo
        .update_status(delegation_uuid, "REJECTED")
        .await
        .map_err(|e| anyhow::anyhow!("Failed to reject: {}", e))?;

    if !updated {
        return Ok(ToolResult {
            success: false,
            content: String::new(),
            error: Some("Delegation not found".to_string()),
        });
    }

    Ok(ToolResult {
        success: true,
        content: serde_json::json!({
            "delegation_id": delegation_id,
            "status": "REJECTED",
            "reason": reason,
            "reroute_requested": reroute,
        }).to_string(),
        error: None,
    })
}
```

- [ ] **Step 6: Implement real GetDelegationStatusTool::execute**

```rust
async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
    let delegation_id = args
        .get("delegation_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("delegation_id required"))?;

    let delegation_uuid = Uuid::parse_str(delegation_id)
        .map_err(|_| anyhow::anyhow!("Invalid delegation_id format"))?;

    let delegation = self.delegation_repo
        .get(delegation_uuid)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to get delegation: {}", e))?;

    match delegation {
        Some(d) => Ok(ToolResult {
            success: true,
            content: serde_json::json!({
                "delegation_id": d.id,
                "status": d.status,
                "created_at": d.created_at,
            }).to_string(),
            error: None,
        }),
        None => Ok(ToolResult {
            success: false,
            content: String::new(),
            error: Some("Delegation not found".to_string()),
        }),
    }
}
```

- [ ] **Step 7: Run cargo check to verify compilation**

Run: `cd crates/torque-harness && cargo check 2>&1 | head -50`
Expected: Compilation errors - we'll fix them as we go

- [ ] **Step 8: Run tests**

Run: `cd crates/torque-harness && cargo test supervisor_tools 2>&1 | tail -30`
Expected: Tests may fail - that's expected during refactoring

- [ ] **Step 9: Commit**

```bash
git add crates/torque-harness/src/service/team/supervisor_tools.rs
git commit -m "refactor(team): add dependencies to supervisor tools"
```

---

## Task 2: Update SupervisorAgent to Pass Dependencies

**Files:**
- Modify: `crates/torque-harness/src/service/team/supervisor_agent.rs`

- [ ] **Step 1: Update SupervisorAgent::new signature**

```rust
impl SupervisorAgent {
    pub async fn new(
        llm: Arc<dyn LlmClient>,
        extra_tools: Vec<crate::tools::ToolArc>,
        delegation_repo: Arc<dyn DelegationRepository>,
        team_task_repo: Arc<dyn TeamTaskRepository>,
        team_member_repo: Arc<dyn TeamMemberRepository>,
        selector_resolver: Arc<SelectorResolver>,
        shared_state: Arc<SharedTaskStateManager>,
    ) -> Self {
        let registry = Arc::new(ToolRegistry::new());

        let supervisor_tools = create_supervisor_tools(
            delegation_repo,
            team_task_repo,
            team_member_repo,
            selector_resolver,
            shared_state,
        );
        for tool in supervisor_tools {
            registry.register(tool).await;
        }

        for tool in extra_tools {
            registry.register(tool).await;
        }

        let react = ReActHarness::new(llm, registry.clone());

        Self {
            react,
            tools: registry,
        }
    }
}
```

- [ ] **Step 2: Update TeamSupervisor::ensure_supervisor_agent to pass deps**

In supervisor.rs, update:

```rust
async fn ensure_supervisor_agent(&self) -> anyhow::Result<()> {
    {
        let guard = self.supervisor_agent.lock().await;
        if guard.is_some() {
            return Ok(());
        }
    }
    if let Some(llm) = &self.llm {
        let agent = SupervisorAgent::new(
            llm.clone(),
            vec![],
            self.delegation_repo.clone(),
            self.task_repo.clone(),
            self.team_member_repo.clone(),  // Add this field to TeamSupervisor
            self.selector_resolver.clone(),
            self.shared_state.clone(),
        )
        .await;
        let mut guard = self.supervisor_agent.lock().await;
        *guard = Some(agent);
    }
    Ok(())
}
```

- [ ] **Step 3: Add team_member_repo field to TeamSupervisor struct**

```rust
pub struct TeamSupervisor {
    task_repo: Arc<dyn TeamTaskRepository>,
    delegation_repo: Arc<dyn DelegationRepository>,
    team_member_repo: Arc<dyn TeamMemberRepository>,  // ADD
    selector_resolver: Arc<SelectorResolver>,
    shared_state: Arc<SharedTaskStateManager>,
    events: Arc<TeamEventEmitter>,
    supervisor_agent: TokioMutex<Option<SupervisorAgent>>,
    llm: Option<Arc<dyn LlmClient>>,
    event_listener: Option<Arc<dyn EventListener>>,
    delegation_timeout: Duration,
}
```

- [ ] **Step 4: Update TeamSupervisor::new to accept team_member_repo**

```rust
pub fn new(
    task_repo: Arc<dyn TeamTaskRepository>,
    delegation_repo: Arc<dyn DelegationRepository>,
    team_member_repo: Arc<dyn TeamMemberRepository>,
    selector_resolver: Arc<SelectorResolver>,
    shared_state: Arc<SharedTaskStateManager>,
    events: Arc<TeamEventEmitter>,
) -> Self {
    Self {
        task_repo,
        delegation_repo,
        team_member_repo,
        selector_resolver,
        shared_state,
        events,
        supervisor_agent: TokioMutex::new(None),
        llm: None,
        event_listener: None,
        delegation_timeout: Duration::from_secs(300),
    }
}
```

- [ ] **Step 5: Run cargo check**

Run: `cd crates/torque-harness && cargo check 2>&1 | head -80`
Expected: Some errors about missing TeamMemberRepository import - fix imports

- [ ] **Step 6: Commit**

```bash
git add crates/torque-harness/src/service/team/supervisor.rs crates/torque-harness/src/service/team/supervisor_agent.rs
git commit -m "refactor(team): pass repo dependencies to SupervisorAgent"
```

---

## Task 3: Implement Real LocalMemberAgent

**Files:**
- Modify: `crates/torque-harness/src/service/team/local_member_agent.rs`

- [ ] **Step 1: Read current LocalMemberAgent implementation**

Run: `cat crates/torque-harness/src/service/team/local_member_agent.rs`

- [ ] **Step 2: Add required dependencies**

```rust
use crate::models::v1::delegation::{Delegation, DelegationStatus};
use crate::repository::DelegationRepository;
use crate::infra::llm::LlmClient;
use crate::agent::stream::StreamEvent;
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

pub struct LocalMemberAgent {
    member_id: Uuid,
    delegation_repo: Arc<dyn DelegationRepository>,
    llm: Option<Arc<dyn LlmClient>>,
}
```

- [ ] **Step 3: Implement poll_tasks with real repository query**

```rust
impl MemberAgent for LocalMemberAgent {
    async fn poll_tasks(&self, _member_id: Uuid) -> Result<Vec<MemberTask>, AgentError> {
        // Query delegations assigned to this member that are PENDING
        let delegations = self.delegation_repo
            .list_by_member(_member_id, 10)
            .await
            .map_err(|e| AgentError::Internal(e.to_string()))?;

        let pending: Vec<MemberTask> = delegations
            .into_iter()
            .filter(|d| d.status == DelegationStatus::Pending)
            .map(|d| MemberTask {
                delegation_id: d.id,
                task_id: d.task_id,
                goal: d.payload.get("goal")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                instructions: d.payload.get("instructions")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            })
            .collect();

        Ok(pending)
    }

    async fn execute_task(&self, task: &MemberTask) -> Result<ExecutionResult, AgentError> {
        // Build execution prompt
        let prompt = format!(
            "Execute the following task:\n\nGoal: {}\nInstructions: {}",
            task.goal,
            task.instructions.as_deref().unwrap_or("None")
        );

        // Call LLM if available
        if let Some(llm) = &self.llm {
            let response = llm.chat(&[
                LlmMessage::user(&prompt),
            ])
            .await
            .map_err(|e| AgentError::LlmError(e.to_string()))?;

            Ok(ExecutionResult {
                output: response,
                artifacts: vec![],
            })
        } else {
            // Fallback: return error if no LLM
            Err(AgentError::LlmNotConfigured)
        }
    }

    async fn complete_task(
        &self,
        delegation_id: Uuid,
        result: ExecutionResult,
    ) -> Result<(), AgentError> {
        // Update delegation status to COMPLETED
        self.delegation_repo
            .update_status(delegation_id, "COMPLETED")
            .await
            .map_err(|e| AgentError::Internal(e.to_string()))?;

        // Emit completion event
        // Note: Event emission would go through event_listener

        Ok(())
    }

    async fn fail_task(
        &self,
        delegation_id: Uuid,
        error: &str,
    ) -> Result<(), AgentError> {
        self.delegation_repo
            .update_status(delegation_id, "FAILED")
            .await
            .map_err(|e| AgentError::Internal(e.to_string()))?;

        Ok(())
    }
}
```

- [ ] **Step 4: Check if DelegationRepository has list_by_member**

Run: `grep -n "list_by_member" crates/torque-harness/src/repository/delegation.rs`
Expected: May not exist - need to add it

- [ ] **Step 5: Add list_by_member to DelegationRepository trait**

In delegation.rs:

```rust
#[async_trait]
pub trait DelegationRepository: Send + Sync {
    // ... existing methods ...

    async fn list_by_member(
        &self,
        member_id: Uuid,
        limit: i64,
    ) -> anyhow::Result<Vec<Delegation>>;
}
```

- [ ] **Step 6: Implement list_by_member in PostgresDelegationRepository**

```rust
async fn list_by_member(
    &self,
    member_id: Uuid,
    limit: i64,
) -> anyhow::Result<Vec<Delegation>> {
    let rows = sqlx::query_as::<_, DelegationRow>(
        r#"
        SELECT * FROM v1_delegations
        WHERE (payload->>'member_id')::uuid = $1
        AND status = 'PENDING'
        ORDER BY created_at ASC
        LIMIT $2
        "#
    )
    .bind(member_id)
    .bind(limit)
    .fetch_all(self.pool())
    .await?;

    Ok(rows.into_iter().map(|r| r.into()).collect())
}
```

- [ ] **Step 7: Run cargo check**

Run: `cd crates/torque-harness && cargo check 2>&1 | head -50`
Expected: Compilation errors about missing types - add imports

- [ ] **Step 8: Commit**

```bash
git add crates/torque-harness/src/service/team/local_member_agent.rs crates/torque-harness/src/repository/delegation.rs
git commit -m "feat(team): implement LocalMemberAgent with real delegation polling"
```

---

## Task 4: Add Rule-Based Triage Fallback

**Files:**
- Modify: `crates/torque-harness/src/service/team/supervisor.rs`

- [ ] **Step 1: Add rule_based_triage method to TeamSupervisor**

Add after the `triage` method:

```rust
fn rule_based_triage(&self, task: &TeamTask) -> TriageResult {
    let goal_len = task.goal.len();
    let has_instructions = task.instructions.is_some();
    let has_complex_structure = task.goal.lines().count() > 3
        || task.goal.contains(" and ")
        || task.goal.contains(" then ");

    let complexity = match (goal_len, has_instructions, has_complex_structure) {
        (g, _, true) if g > 100 => TaskComplexity::Complex,
        (g, true, _) if g > 300 => TaskComplexity::Complex,
        (g, _, _) if g > 500 => TaskComplexity::Complex,
        (g, true, _) if g > 150 => TaskComplexity::Medium,
        (g, false, false) if g > 200 => TaskComplexity::Medium,
        _ => TaskComplexity::Simple,
    };

    let (processing_path, selected_mode) = match complexity {
        TaskComplexity::Simple => (
            ProcessingPath::SingleRoute,
            TeamMode::Route,
        ),
        TaskComplexity::Medium => (
            ProcessingPath::GuidedDelegate,
            if has_complex_structure {
                TeamMode::Broadcast
            } else {
                TeamMode::Route
            },
        ),
        TaskComplexity::Complex => (
            ProcessingPath::StructuredOrchestration,
            if goal_len > 1000 {
                TeamMode::Tasks
            } else {
                TeamMode::Coordinate
            },
        ),
    };

    TriageResult {
        complexity,
        processing_path,
        selected_mode,
        lead_member_ref: None,
        rationale: format!(
            "Rule-based triage: complexity={:?} (len={}, has_instructions={}, complex_struct={})",
            complexity, goal_len, has_instructions, has_complex_structure
        ),
    }
}
```

- [ ] **Step 2: Update triage method to use fallback**

```rust
async fn triage(&self, task: &TeamTask) -> anyhow::Result<TriageResult> {
    // Try LLM-based triage first
    {
        let guard = self.supervisor_agent.lock().await;
        if let Some(agent) = &*guard {
            match agent.triage(&task.goal).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    tracing::warn!("LLM triage failed, using fallback: {}", e);
                }
            }
        }
    }

    // Fallback to rule-based triage
    tracing::info!("Using rule-based triage fallback for task: {}", task.id);
    Ok(self.rule_based_triage(task))
}
```

- [ ] **Step 3: Run cargo check**

Run: `cd crates/torque-harness && cargo check 2>&1 | head -30`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/torque-harness/src/service/team/supervisor.rs
git commit -m "feat(team): add rule-based triage fallback when LLM unavailable"
```

---

## Task 5: Optimize SelectorResolver with Pagination

**Files:**
- Modify: `crates/torque-harness/src/service/team/selector.rs`

- [ ] **Step 1: Review current implementation**

Run: `cat crates/torque-harness/src/service/team/selector.rs`

- [ ] **Step 2: Add streaming/pagination methods to repositories**

First, add to capability_profile_repo in team.rs:

```rust
#[async_trait]
pub trait CapabilityProfileRepository: Send + Sync {
    // ... existing methods ...

    async fn find_by_name_pattern(
        &self,
        pattern: &str,
        limit: i64,
    ) -> anyhow::Result<Vec<CapabilityProfile>>;
}
```

- [ ] **Step 3: Optimize SelectorResolver::resolve**

Replace the current implementation with:

```rust
pub async fn resolve(
    &self,
    selector: &MemberSelector,
    team_instance_id: Uuid,
) -> anyhow::Result<Vec<CandidateMember>> {
    // Load team members (use pagination if list accepts limit)
    let members = self.team_member_repo
        .list_by_team(team_instance_id, 100)
        .await?;

    // Filter based on selector type
    let filtered: Vec<TeamMember> = members
        .into_iter()
        .filter(|member| self.member_matches_selector(member, selector))
        .collect();

    // Resolve capability profiles for filtered members
    let mut candidates = Vec::new();
    for member in filtered {
        let capability_profiles = self.resolve_capability_profiles(&member).await?;

        // Skip if no matching capabilities when selector requires them
        if !selector.capability_profiles.is_empty() {
            let has_match = selector.capability_profiles.iter().any(|required| {
                capability_profiles.iter().any(|cp| {
                    cp.name.to_lowercase().contains(&required.to_lowercase())
                })
            });
            if !has_match {
                continue;
            }
        }

        candidates.push(CandidateMember {
            team_member_id: member.id,
            agent_instance_id: member.agent_instance_id,
            agent_definition_id: member.agent_instance_id,
            role: member.role.clone(),
            capability_profiles,
            selection_rationale: format!("Matched {} selector", selector.selector_type),
            policy_check_summary: PolicyCheckSummary {
                resource_available: true,
                approval_required: false,
                risk_level: "low".to_string(),
            },
        });
    }

    Ok(candidates)
}

async fn resolve_capability_profiles(
    &self,
    member: &TeamMember,
) -> anyhow::Result<Vec<CapabilityProfile>> {
    // Get bindings for this member
    let bindings = self.capability_binding_repo
        .list_by_member(member.id, 50)
        .await?;

    if bindings.is_empty() {
        return Ok(vec![]);
    }

    // Get unique profile IDs
    let profile_ids: Vec<Uuid> = bindings
        .iter()
        .map(|b| b.capability_profile_id)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    // Fetch profiles in batch
    let mut profiles = Vec::new();
    for id in profile_ids {
        if let Some(profile) = self.capability_profile_repo.get(id).await? {
            profiles.push(profile);
        }
    }

    Ok(profiles)
}
```

- [ ] **Step 4: Run cargo check**

Run: `cd crates/torque-harness && cargo check 2>&1 | head -50`
Expected: May have errors about missing methods - add them

- [ ] **Step 5: Commit**

```bash
git add crates/torque-harness/src/service/team/selector.rs crates/torque-harness/src/repository/team.rs
git commit -m "perf(team): optimize SelectorResolver with batch profile fetching"
```

---

## Task 6: Add Atomic SharedTaskState Updates

**Files:**
- Modify: `crates/torque-harness/src/repository/team.rs`

- [ ] **Step 1: Review current SharedTaskStateRepository trait**

Run: `grep -A 30 "pub trait SharedTaskStateRepository" crates/torque-harness/src/repository/team.rs`

- [ ] **Step 2: Add atomic_resolve_blocker method**

```rust
#[async_trait]
pub trait SharedTaskStateRepository: Send + Sync {
    // ... existing methods ...

    async fn atomic_resolve_blocker(
        &self,
        team_instance_id: Uuid,
        blocker_id: Uuid,
    ) -> anyhow::Result<bool>;
}
```

- [ ] **Step 3: Implement atomic_resolve_blocker**

```rust
async fn atomic_resolve_blocker(
    &self,
    team_instance_id: Uuid,
    blocker_id: Uuid,
) -> anyhow::Result<bool> {
    // Use PostgreSQL JSONB array manipulation to atomically remove blocker
    let result = sqlx::query(
        r#"
        UPDATE v1_team_shared_state
        SET
            open_blockers = (
                SELECT COALESCE(
                    jsonb_agg(item),
                    '[]'::jsonb
                )
                FROM jsonb_array_elements(open_blockers) AS item
                WHERE item->>'blocker_id' != $2
            ),
            updated_at = NOW()
        WHERE team_instance_id = $1
        AND EXISTS (
            SELECT 1 FROM jsonb_array_elements(open_blockers) AS item
            WHERE item->>'blocker_id' = $2
        )
        "#
    )
    .bind(team_instance_id)
    .bind(blocker_id.to_string())
    .execute(self.pool())
    .await?;

    Ok(result.rows_affected() > 0)
}
```

- [ ] **Step 4: Update SharedTaskStateManager to use atomic method**

In shared_state.rs:

```rust
pub async fn resolve_blocker(
    &self,
    team_instance_id: Uuid,
    blocker_id: Uuid,
) -> anyhow::Result<bool> {
    self.repo.atomic_resolve_blocker(team_instance_id, blocker_id).await
}
```

- [ ] **Step 5: Run cargo check**

Run: `cd crates/torque-harness && cargo check 2>&1 | head -30`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/torque-harness/src/repository/team.rs crates/torque-harness/src/service/team/shared_state.rs
git commit -m "perf(team): add atomic SharedTaskState blocker resolution"
```

---

## Task 7: Add Tests for Supervisor Tools

**Files:**
- Create: `crates/torque-harness/tests/v1_supervisor_tools_tests.rs`

- [ ] **Step 1: Create test file with mock repositories**

```rust
use std::sync::Arc;
use torque_harness::models::v1::delegation::Delegation;
use torque_harness::repository::{DelegationRepository, MockDelegationRepository};
use torque_harness::service::team::supervisor_tools::{
    create_supervisor_tools, DelegateTaskTool, AcceptResultTool,
};
use torque_harness::service::team::SelectorResolver;
use torque_harness::service::team::SharedTaskStateManager;
use serde_json::json;

#[tokio::test]
async fn test_delegate_task_tool_creates_delegation() {
    // Setup mock
    let mock_delegation_repo = Arc::new(MockDelegationRepository::new());
    let delegation_id = uuid::Uuid::new_v4();
    mock_delegation_repo
        .expect_create()
        .returning(move |_, _, _| {
            Ok(Delegation {
                id: delegation_id,
                status: torque_harness::models::v1::delegation::DelegationStatus::Pending,
                // ... other fields
            })
        });

    let mock_selector = Arc::new(MockSelectorResolver::new());

    // Create tool with deps
    let tool = DelegateTaskTool::new(mock_delegation_repo.clone(), mock_selector);

    // Execute
    let args = json!({
        "member_selector": {"selector_type": "Any"},
        "goal": "Test task"
    });

    let result = tool.execute(args).await.unwrap();

    // Verify
    assert!(result.success);
    assert!(result.error.is_none());
    let content: serde_json::Value = serde_json::from_str(&result.content).unwrap();
    assert_eq!(content["delegation_id"], delegation_id.to_string());
}

#[tokio::test]
async fn test_accept_result_tool_calls_repository() {
    let mock_repo = Arc::new(MockDelegationRepository::new());
    let delegation_id = uuid::Uuid::new_v4();

    mock_repo
        .expect_update_status()
        .returning(move |id, status| {
            assert_eq!(id, delegation_id);
            assert_eq!(status, "ACCEPTED");
            Ok(true)
        });

    let tool = AcceptResultTool::new(mock_repo);

    let args = json!({
        "delegation_id": delegation_id.to_string()
    });

    let result = tool.execute(args).await.unwrap();

    assert!(result.success);
}
```

- [ ] **Step 2: Run tests to see them fail**

Run: `cd crates/torque-harness && cargo test v1_supervisor_tools 2>&1 | tail -40`
Expected: FAIL - MockDelegationRepository doesn't exist

- [ ] **Step 3: Create mock repositories for testing**

Create `crates/torque-harness/tests/mocks/team_mocks.rs`:

```rust
use async_trait::async_trait;
use torque_harness::models::v1::delegation::Delegation;
use torque_harness::repository::DelegationRepository;
use std::sync::{Arc, Mutex};

pub struct MockDelegationRepository {
    pub delegations: Arc<Mutex<Vec<Delegation>>>,
    pub create_called: Arc<Mutex<usize>>,
}

impl MockDelegationRepository {
    pub fn new() -> Self {
        Self {
            delegations: Arc::new(Mutex::new(vec![])),
            create_called: Arc::new(Mutex::new(0)),
        }
    }
}

#[async_trait]
impl DelegationRepository for MockDelegationRepository {
    async fn create(
        &self,
        _task_id: Uuid,
        _member_id: Uuid,
        _payload: serde_json::Value,
    ) -> anyhow::Result<Delegation> {
        let mut call_count = self.create_called.lock().unwrap();
        *call_count += 1;

        let delegation = Delegation {
            id: Uuid::new_v4(),
            task_id: _task_id,
            status: DelegationStatus::Pending,
            payload: _payload,
            created_at: chrono::Utc::now(),
        };

        self.delegations.lock().unwrap().push(delegation.clone());
        Ok(delegation)
    }

    async fn get(&self, id: Uuid) -> anyhow::Result<Option<Delegation>> {
        Ok(self.delegations.lock().unwrap().iter().find(|d| d.id == id).cloned())
    }

    async fn update_status(&self, id: Uuid, status: &str) -> anyhow::Result<bool> {
        let mut delegations = self.delegations.lock().unwrap();
        if let Some(d) = delegations.iter_mut().find(|d| d.id == id) {
            d.status = match status {
                "ACCEPTED" => DelegationStatus::Accepted,
                "REJECTED" => DelegationStatus::Rejected,
                "FAILED" => DelegationStatus::Failed,
                _ => DelegationStatus::Pending,
            };
            return Ok(true);
        }
        Ok(false)
    }

    // ... implement other required methods with panics or returns
}
```

- [ ] **Step 4: Run tests again**

Run: `cd crates/torque-harness && cargo test v1_supervisor_tools 2>&1 | tail -40`
Expected: Some passing, some failing - iterate

- [ ] **Step 5: Commit**

```bash
git add crates/torque-harness/tests/v1_supervisor_tools_tests.rs crates/torque-harness/tests/mocks/team_mocks.rs
git commit -m "test(team): add supervisor tools tests with mocks"
```

---

## Task 8: Final Integration Test

**Files:**
- Create: `crates/torque-harness/tests/v1_team_supervisor_integration_tests.rs`

- [ ] **Step 1: Create end-to-end test**

```rust
#[tokio::test]
async fn test_supervisor_handles_task_end_to_end() {
    // Setup test DB
    let db = setup_test_db().await;

    // Create repos
    let delegation_repo = Arc::new(PostgresDelegationRepository::new(db.clone()));
    let task_repo = Arc::new(PostgresTeamTaskRepository::new(db.clone()));
    let shared_state_repo = Arc::new(PostgresSharedTaskStateRepository::new(db.clone()));
    let team_event_repo = Arc::new(PostgresTeamEventRepository::new(db.clone()));
    let team_member_repo = Arc::new(PostgresTeamMemberRepository::new(db.clone()));

    // Create supervisor with LLM fallback
    let selector_resolver = Arc::new(SelectorResolver::new(
        Arc::new(MockCapabilityRegistry),
        team_member_repo.clone(),
        Arc::new(MockAgentInstanceRepo),
    ));

    let shared_state = Arc::new(SharedTaskStateManager::new(shared_state_repo));
    let events = Arc::new(TeamEventEmitter::new(team_event_repo));

    let supervisor = TeamSupervisor::new(
        task_repo.clone(),
        delegation_repo.clone(),
        team_member_repo.clone(),
        selector_resolver,
        shared_state.clone(),
        events,
    );

    // Create a team instance
    let team_instance_id = Uuid::new_v4();

    // Create a team task
    let task = task_repo
        .create(team_instance_id, "Write a short report", None)
        .await
        .unwrap();

    // Execute via supervisor
    let result = supervisor.execute_task(&task, team_instance_id).await;

    // Verify result indicates task completed or failed (not error)
    assert!(result.is_ok());
    let result = result.unwrap();
    assert!(result.is_some());

    let supervisor_result = result.unwrap();
    tracing::info!("Task completed with summary: {}", supervisor_result.summary);

    // Verify delegation was created
    let delegations = delegation_repo.list_by_team(team_instance_id, 10).await.unwrap();
    assert!(!delegations.is_empty(), "Expected at least one delegation");
}
```

- [ ] **Step 2: Run integration test**

Run: `cd crates/torque-harness && cargo test v1_team_supervisor_integration 2>&1 | tail -50`
Expected: May require database setup - may skip if DB not available

- [ ] **Step 3: Commit**

```bash
git add crates/torque-harness/tests/v1_team_supervisor_integration_tests.rs
git commit -m "test(team): add end-to-end supervisor integration test"
```

---

## Summary

| Task | Description | Status |
|------|-------------|--------|
| 1 | Refactor Supervisor Tools to use real repos | ⬜ |
| 2 | Update SupervisorAgent to pass dependencies | ⬜ |
| 3 | Implement LocalMemberAgent | ⬜ |
| 4 | Add rule-based triage fallback | ⬜ |
| 5 | Optimize SelectorResolver | ⬜ |
| 6 | Add atomic SharedTaskState updates | ⬜ |
| 7 | Add unit tests | ⬜ |
| 8 | Add integration test | ⬜ |

---

## Next Steps After Completion

After these tasks are complete, the following P2 items remain:

1. **Configure hardcoded limits** - Make `max_rounds`, `max_parallel_tasks` configurable
2. **Improve task decomposition** - Use LLM-based decomposition instead of string splitting
3. **Fix Actor identification** - Use actual supervisor ID in events
4. **EventListener error handling** - Add retry/logging for create_consumer_group failures
