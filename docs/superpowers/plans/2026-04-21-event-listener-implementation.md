# EventListener Implementation Plan

> **For agentic workers:** Use superpowers:subagent-driven-development to implement this plan task-by-task.

**Goal:** Complete RedisStreamEventListener to poll Redis Streams and forward DelegationEvent messages.

**Architecture:** EventListener wraps StreamBus with polling loop using xreadgroup for consumer semantics.

---

## Tasks

### Task 1: Complete RedisStreamEventListener Structure
- Add `stream_bus: Arc<RedisStreamBus>` field
- Update `new()` to create StreamBus from redis URL
- Add `consumer_id: String` field

### Task 2: Add DelegationEvent Parsing
- Add `parse_delegation_event(data, delegation_id) -> Option<DelegationEvent>`
- Handles: created, accepted, completed, failed event types

### Task 3: Implement subscribe_delegation with Polling
- Use async_stream for stream generation
- Poll Redis via xreadgroup with timeout
- Parse results and yield DelegationEvent
- Acknowledge messages with xack

### Task 4: Implement subscribe_team and subscribe_member
- Similar pattern to subscribe_delegation
- Stream keys: `team:{id}:tasks:shared`, `member:{id}:tasks`

### Task 5: Add Tests
- Unit tests for parse_delegation_event
- Integration tests require Redis

---

## Dependencies
- `async-stream = "0.3"` (add to Cargo.toml)

## Files
- `crates/torque-harness/src/service/team/event_listener.rs` (modify)
- `crates/torque-harness/tests/event_listener_tests.rs` (create)