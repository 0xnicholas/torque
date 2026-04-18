# Torque Phase 2: Team Execution Implementation Plan

**Goal:** Implement supervisor-driven team collaboration so TeamInstances can receive tasks, delegate to subagents, and coordinate execution.

**Architecture:** TeamInstance receives TeamTask → creates supervisor AgentInstance → supervisor triages and delegates to subagent AgentInstances → results are collected and published to team shared state.

---

## Current State

- ✅ TeamDefinition/TeamInstance CRUD complete
- ✅ AgentInstance execution engine (Phase 1) complete
- ❌ TeamTask creation and execution not implemented
- ❌ Team member management not implemented
- ❌ Team shared state / publish not implemented

---

## Task 1: Add TeamMember Model and Repository

**Files:**
- Modify: `src/models/v1/team.rs`
- Modify: `src/repository/team.rs`
- Modify: `src/service/team.rs`

Add TeamMember to track which agent instances belong to a team.

## Task 2: Implement create_task Handler

**Files:**
- Modify: `src/api/v1/teams.rs`
- Modify: `src/service/team.rs`

Create a team task that:
1. Validates team instance exists
2. Creates supervisor agent instance from team definition
3. Creates Task with team_instance_id
4. Returns 202 Accepted with task ID

## Task 3: Implement list_tasks Handler

**Files:**
- Modify: `src/api/v1/teams.rs`
- Modify: `src/service/team.rs`
- Modify: `src/repository/task.rs`

List tasks filtered by team_instance_id.

## Task 4: Implement list_members Handler

**Files:**
- Modify: `src/api/v1/teams.rs`
- Modify: `src/service/team.rs`

List active member agent instances for a team.

## Task 5: Implement publish Handler

**Files:**
- Modify: `src/api/v1/teams.rs`
- Modify: `src/service/team.rs`

Publish artifact to team shared state.

## Task 6: Add Team Execution Integration Tests

**Files:**
- Create: `tests/v1_team_execution_tests.rs`

## Task 7: Final Verification

- Run all tests
- Update STATUS.md
- Commit
