# PolicyEvaluator 6-Dimension Completion — Implementation Plan

## Tasks

- [ ] **Task 1: Rewrite evaluator.rs** — Add 5 missing dimension evaluation methods, remove action_type gate, refactor existing tool evaluation into `evaluate_tool` method.
- [ ] **Task 2: Write unit tests** — `policy_evaluator_tests.rs`: merge semantics, schema parsing, cross-dimension isolation, regression.
- [ ] **Task 3: Write integration tests** — `policy_evaluator_integration_tests.rs`: GovernedToolRegistry, RunService, multi-dimension end-to-end.
- [ ] **Task 4: Verify** — `cargo test -p torque-harness`, `cargo check --workspace`.

## File changes

| File | Action |
|------|--------|
| `crates/torque-harness/src/policy/evaluator.rs` | Rewrite |
| `crates/torque-harness/tests/policy_evaluator_tests.rs` | Create |
| `crates/torque-harness/tests/policy_evaluator_integration_tests.rs` | Create |
