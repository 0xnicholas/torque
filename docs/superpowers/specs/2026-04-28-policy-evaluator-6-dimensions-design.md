# PolicyEvaluator 6-Dimension Completion Design

## Overview

Complete `PolicyEvaluator` to evaluate all 6 policy dimensions (tool, approval, visibility, delegation, resource, memory) per the Torque Policy Model Spec.

**Date**: 2026-04-28
**Status**: Approved design, ready for implementation

---

## Problem

`PolicyEvaluator.evaluate()` (`policy/evaluator.rs:20`) only handles `action_type == "tool_call"`, evaluating just 1 of 6 spec-defined dimensions. The remaining 5 return `PolicyDecision::default()` — silently allowing everything.

Yet `PolicyDecision` and `PolicySources` are already structured for all 6 dimensions, and `merge()` already handles them all.

## Architecture

```
evaluate(input, sources)
  ├── evaluate_tool(sources)          → tool_restrictions + allowed + requires_approval
  ├── evaluate_approval(sources)      → approval_dimensions + requires_approval
  ├── evaluate_visibility(sources)    → visibility_restriction
  ├── evaluate_delegation(sources)    → delegation_restrictions
  ├── evaluate_resource(sources)      → resource_limits
  ├── evaluate_memory(sources)        → memory_restrictions
  └── conservative merge → PolicyDecision
```

Key design decisions:
- **No action_type gating**: each dimension evaluates all 6 sources independently
- **Conservative merge**: within each dimension, the most restrictive rule wins
- **Cross-dimension isolation**: a tool deny does not affect memory, etc.
- **Empty source = no-op**: missing/empty JSON → `PolicyDecision::default()`

## Dimension Schemas

Each dimension reads from `PolicySources` (system, capability, agent, team, selector, runtime), each an `Option<serde_json::Value>`.

### Tool (existing, refactored)

```json
{ "forbidden_tools": ["x"], "require_approval_tools": ["y"], "allowed_tools": ["z"] }
```

### Approval

```json
{ "approval_required": true, "approval_requirements": ["external"], "auto_approve_timeout_seconds": null, "require_operator_escalation": false }
```

### Visibility

```json
{ "visibility_scope": "narrow", "allowed_scopes": ["private"], "denied_scopes": ["external_published"] }
```

### Delegation

```json
{ "delegation_allowed": true, "max_delegation_depth": 3, "child_delegation_allowed": false, "handoff_allowed": false }
```

### Resource

```json
{ "resource_budget_cap": 1000, "max_concurrency": 5, "timeout_seconds": 300, "defer_under_pressure": true }
```

### Memory

```json
{ "memory_write_allowed": true, "memory_candidate_only": false, "max_memory_entries": 500, "require_review_before_write": false }
```

## Code Structure

### File changes

```
crates/torque-harness/src/policy/
├── evaluator.rs        (rewrite: 6 evaluate_* methods, remove action_type gate)
├── decision.rs         (unchanged)
├── mod.rs              (unchanged)
├── filesystem.rs       (unchanged)
└── tool_governance.rs  (unchanged)

crates/torque-harness/tests/
├── policy_evaluator_tests.rs           (NEW: unit tests)
└── policy_evaluator_integration_tests.rs (NEW: integration tests)
```

Each dimension has two methods:
1. `evaluate_xxx(&self, sources: &PolicySources) -> PolicyDecision` — iterates 6 sources, merges
2. `evaluate_single_source_xxx(policy: &Value) -> PolicyDecision` — parses one source JSON

### No breaking changes

- `PolicyEvaluator::evaluate()` signature unchanged
- `PolicyDecision`, `PolicySources`, `PolicyInput` unchanged
- `GovernedToolRegistry` and `RunService` unchanged

## Testing

### Unit tests (`policy_evaluator_tests.rs`)

- Conservative merge: any source can deny/restrict
- Cross-dimension isolation: tool deny ≠ memory block
- Multi-source merge: multiple sources conservative merge
- Schema parsing per dimension
- Regression: existing tool behavior preserved

### Integration tests (`policy_evaluator_integration_tests.rs`)

- GovernedToolRegistry with PolicyEvaluator inline
- RunService.evaluate_tool_policy regression
- Multi-dimension end-to-end: single evaluate call with 3+ dimensions
- Dimension isolation at integration level

## Verification

```bash
cargo test -p torque-harness -- policy_evaluator
cargo test -p torque-harness
cargo check --workspace
```

## Risk

Zero. All existing callers only submit `action_type == "tool_call"`. New dimensions return `PolicyDecision::default()` for empty sources, preserving existing behavior. Additional dimension evaluation only adds restrictions when sources contain relevant policy keys.
