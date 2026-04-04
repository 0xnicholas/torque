# Torque Core Skeleton Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build foundational infrastructure - core types, DAG structures, database schema, and queue mechanism.

**Architecture:** Four independent but ordered crates. types (no deps) → dag (depends on types) → db (depends on types) → queue (depends on types, db).

**Tech Stack:** Rust, serde, sqlx, thiserror, uuid, chrono

---

## File Structure Overview

```
crates/
├── types/                      # NEW
│   ├── src/
│   │   ├── lib.rs            # Public exports
│   │   ├── run.rs            # Run type
│   │   ├── node.rs           # Node type
│   │   ├── edge.rs           # Edge type
│   │   ├── artifact.rs       # Artifact type
│   │   ├── queue.rs          # QueueEntry type
│   │   ├── tenant.rs         # Tenant type
│   │   ├── agent_type.rs     # AgentType type
│   │   └── error.rs          # Common error types
│   ├── tests/
│   │   └── serialization.rs
│   └── Cargo.toml
├── dag/                       # NEW
│   ├── src/
│   │   ├── lib.rs
│   │   ├── validate.rs       # DAG validation
│   │   ├── topo_sort.rs     # Topological sort
│   │   ├── layers.rs         # Layer computation
│   │   └── error.rs
│   ├── tests/
│   │   ├── validation.rs
│   │   ├── topo_sort.rs
│   │   └── layers.rs
│   └── Cargo.toml
├── db/                        # NEW
│   ├── src/
│   │   ├── lib.rs
│   │   ├── runs.rs
│   │   ├── nodes.rs
│   │   ├── edges.rs
│   │   ├── artifacts.rs
│   │   ├── queue.rs
│   │   └── migrations.rs
│   ├── migrations/
│   │   ├── 001_create_tenants.sql
│   │   ├── 002_create_agent_types.sql
│   │   ├── 003_create_runs.sql
│   │   ├── 004_create_nodes.sql
│   │   ├── 005_create_edges.sql
│   │   ├── 006_create_artifacts.sql
│   │   └── 007_create_queue.sql
│   ├── tests/
│   │   └── crud.rs
│   └── Cargo.toml
└── queue/                     # NEW
    ├── src/
    │   ├── lib.rs
    │   ├── enqueue.rs
    │   ├── dequeue.rs
    │   ├── complete.rs
    │   └── error.rs
    ├── tests/
    │   └── concurrent_dequeue.rs
    └── Cargo.toml
```

---

## Phase 1: Types Crate (Day 1)

### Task 1: Create types crate scaffold

**Files:**
- Create: `crates/types/Cargo.toml`
- Create: `crates/types/src/lib.rs`

- [ ] **Step 1: Create Cargo.toml**

```toml
[package]
name = "types"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
thiserror = "1"
```

- [ ] **Step 2: Create lib.rs with public exports**

```rust
pub mod run;
pub mod node;
pub mod edge;
pub mod artifact;
pub mod queue;
pub mod tenant;
pub mod agent_type;
pub mod error;

pub use run::Run;
pub use node::Node;
pub use edge::Edge;
pub use artifact::Artifact;
pub use queue::QueueEntry;
pub use tenant::Tenant;
pub use agent_type::AgentType;
pub use error::{Error, ErrorKind};
```

- [ ] **Step 3: Commit**

```bash
git add crates/types/
git commit -m "feat(types): create types crate scaffold"
```

---

### Task 2: Define Run type

**Files:**
- Create: `crates/types/src/run.rs`

- [ ] **Step 1: Create run.rs**

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Planning,
    Pending,
    Running,
    Done,
    Failed,
    PlanningFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub status: RunStatus,
    pub instruction: String,
    pub failure_policy: String,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

impl Run {
    pub fn new(tenant_id: Uuid, instruction: String, failure_policy: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            tenant_id,
            status: RunStatus::Planning,
            instruction,
            failure_policy,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            error: None,
        }
    }
}
```

- [ ] **Step 2: Run cargo check**

```bash
cd crates/types && cargo check
```

- [ ] **Step 3: Commit**

```bash
git add crates/types/src/run.rs
git commit -m "feat(types): define Run type with status enum"
```

---

### Task 3: Define Node type

**Files:**
- Create: `crates/types/src/node.rs`

- [ ] **Step 1: Create node.rs**

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeStatus {
    Pending,
    Running,
    Done,
    Failed,
    Skipped,
    PendingApproval,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: Uuid,
    pub run_id: Uuid,
    pub tenant_id: Uuid,
    pub agent_type: String,
    pub fallback_agent_type: Option<String>,
    pub instruction: String,
    pub tools: Option<Vec<String>>,
    pub failure_policy: Option<String>,
    pub requires_approval: bool,
    pub status: NodeStatus,
    pub layer: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub retry_count: i32,
    pub error: Option<String>,
    pub executor_id: Option<String>,
}

impl Node {
    pub fn new(
        run_id: Uuid,
        tenant_id: Uuid,
        agent_type: String,
        instruction: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            run_id,
            tenant_id,
            agent_type,
            fallback_agent_type: None,
            instruction,
            tools: None,
            failure_policy: None,
            requires_approval: false,
            status: NodeStatus::Pending,
            layer: None,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            retry_count: 0,
            error: None,
            executor_id: None,
        }
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/types/src/node.rs
git commit -m "feat(types): define Node type with status enum"
```

---

### Task 4: Define remaining types (Edge, Artifact, QueueEntry, Tenant, AgentType)

**Files:**
- Create: `crates/types/src/edge.rs`
- Create: `crates/types/src/artifact.rs`
- Create: `crates/types/src/queue.rs`
- Create: `crates/types/src/tenant.rs`
- Create: `crates/types/src/agent_type.rs`
- Create: `crates/types/src/error.rs`

- [ ] **Step 1: Create edge.rs**

```rust
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub id: Uuid,
    pub run_id: Uuid,
    pub source_node: Uuid,
    pub target_node: Uuid,
}

impl Edge {
    pub fn new(run_id: Uuid, source_node: Uuid, target_node: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            run_id,
            source_node,
            target_node,
        }
    }
}
```

- [ ] **Step 2: Create artifact.rs**

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageType {
    Redis,
    S3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub id: Uuid,
    pub node_id: Uuid,
    pub tenant_id: Uuid,
    pub storage: StorageType,
    pub location: String,
    pub size_bytes: i64,
    pub content_type: String,
    pub created_at: DateTime<Utc>,
}

impl Artifact {
    pub fn new(
        node_id: Uuid,
        tenant_id: Uuid,
        storage: StorageType,
        location: String,
        size_bytes: i64,
        content_type: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            node_id,
            tenant_id,
            storage,
            location,
            size_bytes,
            content_type,
            created_at: Utc::now(),
        }
    }
}
```

- [ ] **Step 3: Create queue.rs**

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueStatus {
    Pending,
    Locked,
    Done,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueEntry {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub run_id: Uuid,
    pub node_id: Uuid,
    pub priority: i32,
    pub status: QueueStatus,
    pub available_at: DateTime<Utc>,
    pub locked_at: Option<DateTime<Utc>>,
    pub locked_by: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl QueueEntry {
    pub fn new(
        tenant_id: Uuid,
        run_id: Uuid,
        node_id: Uuid,
        priority: i32,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            tenant_id,
            run_id,
            node_id,
            priority,
            status: QueueStatus::Pending,
            available_at: Utc::now(),
            locked_at: None,
            locked_by: None,
            created_at: Utc::now(),
        }
    }
}
```

- [ ] **Step 4: Create tenant.rs**

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tenant {
    pub id: Uuid,
    pub name: String,
    pub weight: i32,
    pub max_concurrency: i32,
    pub monthly_token_quota: Option<i64>,
    pub created_at: DateTime<Utc>,
}

impl Tenant {
    pub fn new(name: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            weight: 1,
            max_concurrency: 10,
            monthly_token_quota: None,
            created_at: Utc::now(),
        }
    }
}
```

- [ ] **Step 5: Create agent_type.rs**

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentType {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub system_prompt: String,
    pub tools: Vec<String>,
    pub max_tokens: i32,
    pub timeout_secs: i32,
    pub created_at: DateTime<Utc>,
}

impl AgentType {
    pub fn new(name: String, system_prompt: String, tools: Vec<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            description: None,
            system_prompt,
            tools,
            max_tokens: 4096,
            timeout_secs: 300,
            created_at: Utc::now(),
        }
    }
}
```

- [ ] **Step 6: Create error.rs**

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Serialization error: {0}")]
    Serialization(String),
    
    #[error("Validation error: {0}")]
    Validation(String),
    
    #[error("Database error: {0}")]
    Database(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Serialization,
    Validation,
    Database,
}
```

- [ ] **Step 7: Run cargo check**

```bash
cd crates/types && cargo check
```

- [ ] **Step 8: Commit**

```bash
git add crates/types/src/edge.rs crates/types/src/artifact.rs crates/types/src/queue.rs crates/types/src/tenant.rs crates/types/src/agent_type.rs crates/types/src/error.rs
git commit -m "feat(types): define Edge, Artifact, QueueEntry, Tenant, AgentType, Error types"
```

---

### Task 5: Add serialization tests

**Files:**
- Create: `crates/types/tests/serialization.rs`

- [ ] **Step 1: Create serialization tests**

```rust
use types::{Run, RunStatus, Node, NodeStatus, Edge, Artifact, StorageType, QueueEntry, QueueStatus, Tenant, AgentType};

#[test]
fn test_run_serialization() {
    let run = Run::new(
        uuid::Uuid::new_v4(),
        "Test instruction".to_string(),
        "abort".to_string(),
    );
    
    let json = serde_json::to_string(&run).unwrap();
    let parsed: Run = serde_json::from_str(&json).unwrap();
    
    assert_eq!(run.id, parsed.id);
    assert_eq!(run.status, RunStatus::Planning);
}

#[test]
fn test_node_serialization() {
    let node = Node::new(
        uuid::Uuid::new_v4(),
        uuid::Uuid::new_v4(),
        "researcher".to_string(),
        "Search for X".to_string(),
    );
    
    let json = serde_json::to_string(&node).unwrap();
    let parsed: Node = serde_json::from_str(&json).unwrap();
    
    assert_eq!(node.id, parsed.id);
    assert_eq!(node.status, NodeStatus::Pending);
}

#[test]
fn test_queue_entry_serialization() {
    let entry = QueueEntry::new(
        uuid::Uuid::new_v4(),
        uuid::Uuid::new_v4(),
        uuid::Uuid::new_v4(),
        0,
    );
    
    let json = serde_json::to_string(&entry).unwrap();
    let parsed: QueueEntry = serde_json::from_str(&json).unwrap();
    
    assert_eq!(entry.id, parsed.id);
    assert_eq!(entry.status, QueueStatus::Pending);
}
```

- [ ] **Step 2: Run tests**

```bash
cd crates/types && cargo test
```

- [ ] **Step 3: Commit**

```bash
git add crates/types/tests/serialization.rs
git commit -m "test(types): add serialization tests"
```

---

## Phase 2: DAG Crate (Day 2)

### Task 6: Create dag crate scaffold

**Files:**
- Create: `crates/dag/Cargo.toml`
- Create: `crates/dag/src/lib.rs`

- [ ] **Step 1: Create Cargo.toml**

```toml
[package]
name = "dag"
version = "0.1.0"
edition = "2021"

[dependencies]
types = { path = "../types" }
thiserror = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

- [ ] **Step 2: Create lib.rs with public exports**

```rust
pub mod validate;
pub mod topo_sort;
pub mod layers;
pub mod error;

pub use validate::validate_dag;
pub use topo_sort::topological_sort;
pub use layers::compute_layers;
pub use error::{DagError, DagErrorKind};
```

- [ ] **Step 3: Create error.rs**

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DagError {
    #[error("Cycle detected in DAG")]
    CycleDetected,
    
    #[error("Invalid edge reference: {0}")]
    InvalidEdgeReference(String),
    
    #[error("Orphan node detected: {0}")]
    OrphanNode(String),
    
    #[error("Empty node list")]
    EmptyNodeList,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DagErrorKind {
    CycleDetected,
    InvalidEdgeReference,
    OrphanNode,
    EmptyNodeList,
}
```

- [ ] **Step 4: Commit**

```bash
git add crates/dag/
git commit -m "feat(dag): create dag crate scaffold"
```

---

### Task 7: Implement DAG validation

**Files:**
- Create: `crates/dag/src/validate.rs`

- [ ] **Step 1: Create validate.rs with Kahn's algorithm**

```rust
use std::collections::{HashMap, HashSet};
use types::{Edge, Node};
use crate::error::{DagError, DagErrorKind};
use crate::DagError;

pub fn validate_dag(nodes: &[Node], edges: &[Edge]) -> Result<(), DagError> {
    if nodes.is_empty() {
        return Err(DagError::EmptyNodeList);
    }
    
    let node_ids: HashSet<_> = nodes.iter().map(|n| n.id).collect();
    
    let mut in_degree: HashMap<_, usize> = nodes.iter().map(|n| (n.id, 0)).collect();
    let mut adjacency: HashMap<_, Vec<_>> = nodes.iter().map(|n| (n.id, vec![])).collect();
    
    for edge in edges {
        if !node_ids.contains(&edge.source_node) {
            return Err(DagError::InvalidEdgeReference(format!(
                "source node {} not found", edge.source_node
            )));
        }
        if !node_ids.contains(&edge.target_node) {
            return Err(DagError::InvalidEdgeReference(format!(
                "target node {} not found", edge.target_node
            )));
        }
        
        adjacency.get_mut(&edge.source_node).unwrap().push(edge.target_node);
        *in_degree.get_mut(&edge.target_node).unwrap() += 1;
    }
    
    let mut queue: Vec<_> = in_degree
        .iter()
        .filter(|(_, &d)| d == 0)
        .map(|(id, _)| *id)
        .collect();
    let mut visited = 0;
    
    while let Some(node_id) = queue.pop() {
        visited += 1;
        for &neighbor in adjacency.get(&node_id).unwrap() {
            *in_degree.get_mut(&neighbor).unwrap() -= 1;
            if in_degree[&neighbor] == 0 {
                queue.push(neighbor);
            }
        }
    }
    
    if visited != nodes.len() {
        return Err(DagError::CycleDetected);
    }
    
    let root_ids: HashSet<_> = edges.iter().map(|e| e.target_node).collect();
    let has_roots = nodes.iter().any(|n| !root_ids.contains(&n.id));
    if !has_roots && !nodes.is_empty() {
        return Err(DagError::OrphanNode("no root nodes found".to_string()));
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use types::{Node, Edge};
    use uuid::Uuid;

    #[test]
    fn test_valid_dag() {
        let node1 = Node::new(Uuid::new_v4(), Uuid::new_v4(), "a".to_string(), "".to_string());
        let node2 = Node::new(Uuid::new_v4(), Uuid::new_v4(), "b".to_string(), "".to_string());
        let nodes = vec![node1.clone(), node2.clone()];
        
        let edge = Edge::new(nodes[0].run_id, nodes[0].id, nodes[1].id);
        let edges = vec![edge];
        
        validate_dag(&nodes, &edges).unwrap();
    }
    
    #[test]
    fn test_cycle_detected() {
        let node1 = Node::new(Uuid::new_v4(), Uuid::new_v4(), "a".to_string(), "".to_string());
        let node2 = Node::new(Uuid::new_v4(), Uuid::new_v4(), "b".to_string(), "".to_string());
        let nodes = vec![node1.clone(), node2.clone()];
        
        let edge1 = Edge::new(nodes[0].run_id, nodes[0].id, nodes[1].id);
        let edge2 = Edge::new(nodes[0].run_id, nodes[1].id, nodes[0].id);
        let edges = vec![edge1, edge2];
        
        let result = validate_dag(&nodes, &edges);
        assert!(result.is_err());
    }
}
```

- [ ] **Step 2: Run cargo check and tests**

```bash
cd crates/dag && cargo test
```

- [ ] **Step 3: Commit**

```bash
git add crates/dag/src/validate.rs
git commit -m "feat(dag): implement DAG validation with cycle detection"
```

---

### Task 8: Implement topological sort

**Files:**
- Create: `crates/dag/src/topo_sort.rs`

- [ ] **Step 1: Create topo_sort.rs**

```rust
use std::collections::{HashMap, VecDeque};
use types::{Edge, Node};
use crate::DagError;

pub fn topological_sort(nodes: &[Node], edges: &[Edge]) -> Result<Vec<uuid::Uuid>, DagError> {
    let mut in_degree: HashMap<_, usize> = nodes.iter().map(|n| (n.id, 0)).collect();
    let mut adjacency: HashMap<_, Vec<_>> = nodes.iter().map(|n| (n.id, vec![])).collect();
    
    for edge in edges {
        adjacency.get_mut(&edge.source_node).unwrap().push(edge.target_node);
        *in_degree.get_mut(&edge.target_node).unwrap() += 1;
    }
    
    let mut queue: VecDeque<_> = in_degree
        .iter()
        .filter(|(_, &d)| d == 0)
        .map(|(id, _)| *id)
        .collect();
    
    let mut result = Vec::new();
    
    while let Some(node_id) = queue.pop_front() {
        result.push(node_id);
        for &neighbor in adjacency.get(&node_id).unwrap() {
            *in_degree.get_mut(&neighbor).unwrap() -= 1;
            if in_degree[&neighbor] == 0 {
                queue.push_back(neighbor);
            }
        }
    }
    
    if result.len() != nodes.len() {
        return Err(DagError::CycleDetected);
    }
    
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use types::{Node, Edge};
    use uuid::Uuid;

    #[test]
    fn test_topological_sort() {
        let node1 = Node::new(Uuid::new_v4(), Uuid::new_v4(), "a".to_string(), "".to_string());
        let node2 = Node::new(Uuid::new_v4(), Uuid::new_v4(), "b".to_string(), "".to_string());
        let node3 = Node::new(Uuid::new_v4(), Uuid::new_v4(), "c".to_string(), "".to_string());
        let nodes = vec![node1.clone(), node2.clone(), node3.clone()];
        
        let edge1 = Edge::new(nodes[0].run_id, nodes[0].id, nodes[1].id);
        let edge2 = Edge::new(nodes[0].run_id, nodes[1].id, nodes[2].id);
        let edges = vec![edge1, edge2];
        
        let sorted = topological_sort(&nodes, &edges).unwrap();
        assert_eq!(sorted.len(), 3);
        assert!(sorted[0] == nodes[0].id);
        assert!(sorted[2] == nodes[2].id);
    }
}
```

- [ ] **Step 2: Run cargo test**

```bash
cd crates/dag && cargo test
```

- [ ] **Step 3: Commit**

```bash
git add crates/dag/src/topo_sort.rs
git commit -m "feat(dag): implement topological sort"
```

---

### Task 9: Implement layer computation

**Files:**
- Create: `crates/dag/src/layers.rs`

- [ ] **Step 1: Create layers.rs**

```rust
use std::collections::HashMap;
use types::{Edge, Node};
use crate::DagError;

pub type Layer = i32;

pub fn compute_layers(nodes: &[Node], edges: &[Edge]) -> Result<HashMap<uuid::Uuid, Layer>, DagError> {
    let mut in_degree: HashMap<_, usize> = nodes.iter().map(|n| (n.id, 0)).collect();
    let mut outgoing: HashMap<_, Vec<_>> = nodes.iter().map(|n| (n.id, vec![])).collect();
    
    for edge in edges {
        *in_degree.get_mut(&edge.target_node).unwrap() += 1;
        outgoing.get_mut(&edge.source_node).unwrap().push(edge.target_node);
    }
    
    let mut layers: HashMap<uuid::Uuid, Layer> = HashMap::new();
    let mut queue: Vec<_> = in_degree
        .iter()
        .filter(|(_, &d)| d == 0)
        .map(|(id, _)| *id)
        .collect();
    
    for node_id in queue {
        layers.insert(node_id, 0);
    }
    
    while let Some(node_id) = queue.pop() {
        let current_layer = layers[&node_id];
        for &neighbor in outgoing.get(&node_id).unwrap() {
            let new_layer = current_layer + 1;
            let existing = layers.entry(neighbor).or_insert(new_layer);
            *existing = (*existing).max(new_layer);
            *in_degree.get_mut(&neighbor).unwrap() -= 1;
            if in_degree[&neighbor] == 0 {
                queue.push(neighbor);
            }
        }
    }
    
    Ok(layers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use types::{Node, Edge};
    use uuid::Uuid;

    #[test]
    fn test_compute_layers() {
        let node1 = Node::new(Uuid::new_v4(), Uuid::new_v4(), "a".to_string(), "".to_string());
        let node2 = Node::new(Uuid::new_v4(), Uuid::new_v4(), "b".to_string(), "".to_string());
        let node3 = Node::new(Uuid::new_v4(), Uuid::new_v4(), "c".to_string(), "".to_string());
        let nodes = vec![node1.clone(), node2.clone(), node3.clone()];
        
        let edge1 = Edge::new(nodes[0].run_id, nodes[0].id, nodes[1].id);
        let edge2 = Edge::new(nodes[0].run_id, nodes[1].id, nodes[2].id);
        let edges = vec![edge1, edge2];
        
        let layers = compute_layers(&nodes, &edges).unwrap();
        
        assert_eq!(layers[&nodes[0].id], 0);
        assert_eq!(layers[&nodes[1].id], 1);
        assert_eq!(layers[&nodes[2].id], 2);
    }
}
```

- [ ] **Step 2: Run cargo test**

```bash
cd crates/dag && cargo test
```

- [ ] **Step 3: Commit**

```bash
git add crates/dag/src/layers.rs
git commit -m "feat(dag): implement layer computation"
```

---

## Phase 3: DB Crate (Day 3)

### Task 10: Create db crate scaffold

**Files:**
- Create: `crates/db/Cargo.toml`
- Create: `crates/db/src/lib.rs`

- [ ] **Step 1: Create Cargo.toml**

```toml
[package]
name = "db"
version = "0.1.0"
edition = "2021"

[dependencies]
types = { path = "../types" }
sqlx = { version = "0.7", features = ["runtime-tokio", "postgres", "uuid", "chrono", "json"] }
thiserror = "1"
tokio = { version = "1", features = ["full"] }
```

- [ ] **Step 2: Create lib.rs**

```rust
pub mod runs;
pub mod nodes;
pub mod edges;
pub mod artifacts;
pub mod queue;
pub mod migrations;

pub use sqlx::PgPool;
```

- [ ] **Step 3: Commit**

```bash
git add crates/db/
git commit -m "feat(db): create db crate scaffold"
```

---

### Task 11: Create migrations

**Files:**
- Create: `crates/db/migrations/001_create_tenants.sql`
- Create: `crates/db/migrations/002_create_agent_types.sql`
- Create: `crates/db/migrations/003_create_runs.sql`
- Create: `crates/db/migrations/004_create_nodes.sql`
- Create: `crates/db/migrations/005_create_edges.sql`
- Create: `crates/db/migrations/006_create_artifacts.sql`
- Create: `crates/db/migrations/007_create_queue.sql`

- [ ] **Step 1: Create all migration files**

```sql
-- 001_create_tenants.sql
CREATE TABLE tenants (
    id            UUID PRIMARY KEY,
    name          TEXT NOT NULL,
    weight        INTEGER DEFAULT 1,
    max_concurrency INTEGER DEFAULT 10,
    monthly_token_quota BIGINT,
    created_at    TIMESTAMPTZ DEFAULT NOW()
);
```

```sql
-- 002_create_agent_types.sql
CREATE TABLE agent_types (
    id            UUID PRIMARY KEY,
    name          TEXT UNIQUE NOT NULL,
    description   TEXT,
    system_prompt TEXT NOT NULL,
    tools         JSONB DEFAULT '[]',
    max_tokens    INTEGER DEFAULT 4096,
    timeout_secs  INTEGER DEFAULT 300,
    created_at    TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_agent_types_name ON agent_types(name);
```

```sql
-- 003_create_runs.sql
CREATE TABLE runs (
    id            UUID PRIMARY KEY,
    tenant_id     UUID REFERENCES tenants,
    status        TEXT NOT NULL DEFAULT 'planning',
    instruction   TEXT NOT NULL,
    failure_policy TEXT DEFAULT 'abort',
    created_at    TIMESTAMPTZ DEFAULT NOW(),
    started_at    TIMESTAMPTZ,
    completed_at  TIMESTAMPTZ,
    error         TEXT
);

CREATE INDEX idx_runs_tenant_id ON runs(tenant_id);
CREATE INDEX idx_runs_status ON runs(status);
```

```sql
-- 004_create_nodes.sql
CREATE TABLE nodes (
    id                UUID PRIMARY KEY,
    run_id            UUID REFERENCES runs,
    tenant_id         UUID REFERENCES tenants,
    agent_type        TEXT REFERENCES agent_types(name),
    fallback_agent_type TEXT REFERENCES agent_types(name),
    instruction       TEXT NOT NULL,
    tools             JSONB,
    failure_policy    TEXT,
    requires_approval BOOLEAN DEFAULT false,
    status            TEXT NOT NULL DEFAULT 'pending',
    layer             INTEGER,
    created_at        TIMESTAMPTZ DEFAULT NOW(),
    started_at        TIMESTAMPTZ,
    completed_at      TIMESTAMPTZ,
    retry_count       INTEGER DEFAULT 0,
    error             TEXT,
    executor_id       TEXT
);

CREATE INDEX idx_nodes_run_id ON nodes(run_id);
CREATE INDEX idx_nodes_tenant_id ON nodes(tenant_id);
CREATE INDEX idx_nodes_status ON nodes(status);
CREATE INDEX idx_nodes_layer ON nodes(layer);
```

```sql
-- 005_create_edges.sql
CREATE TABLE edges (
    id            UUID PRIMARY KEY,
    run_id        UUID REFERENCES runs,
    source_node   UUID REFERENCES nodes,
    target_node   UUID REFERENCES nodes
);

CREATE INDEX idx_edges_run_id ON edges(run_id);
```

```sql
-- 006_create_artifacts.sql
CREATE TABLE artifacts (
    id            UUID PRIMARY KEY,
    node_id       UUID REFERENCES nodes,
    tenant_id     UUID REFERENCES tenants,
    storage       TEXT NOT NULL,
    location      TEXT NOT NULL,
    size_bytes    BIGINT,
    content_type  TEXT,
    created_at    TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_artifacts_node_id ON artifacts(node_id);
CREATE INDEX idx_artifacts_tenant_id ON artifacts(tenant_id);
```

```sql
-- 007_create_queue.sql
CREATE TABLE queue (
    id            UUID PRIMARY KEY,
    tenant_id     UUID REFERENCES tenants,
    run_id        UUID REFERENCES runs,
    node_id       UUID REFERENCES nodes UNIQUE,
    priority      INTEGER DEFAULT 0,
    status        TEXT NOT NULL DEFAULT 'pending',
    available_at  TIMESTAMPTZ DEFAULT NOW(),
    locked_at     TIMESTAMPTZ,
    locked_by     TEXT,
    created_at    TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_queue_tenant_id ON queue(tenant_id);
CREATE INDEX idx_queue_status ON queue(status);
CREATE INDEX idx_queue_available_at ON queue(available_at);
```

- [ ] **Step 2: Commit**

```bash
git add crates/db/migrations/
git commit -m "feat(db): add all migration files"
```

---

### Task 12: Implement runs CRUD

**Files:**
- Create: `crates/db/src/runs.rs`

- [ ] **Step 1: Create runs.rs**

```rust
use sqlx::PgPool;
use types::{Run, RunStatus};
use uuid::Uuid;

pub async fn create(pool: &PgPool, run: &Run) -> Result<Run, sqlx::Error> {
    sqlx::query_as!(
        Run,
        r#"
        INSERT INTO runs (id, tenant_id, status, instruction, failure_policy, created_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING *
        "#,
        run.id,
        run.tenant_id,
        run.status.to_string(),
        run.instruction,
        run.failure_policy,
        run.created_at
    )
    .fetch_one(pool)
    .await
}

pub async fn get(pool: &PgPool, id: Uuid) -> Result<Option<Run>, sqlx::Error> {
    sqlx::query_as!(
        Run,
        "SELECT * FROM runs WHERE id = $1",
        id
    )
    .fetch_optional(pool)
    .await
}

pub async fn update_status(pool: &PgPool, id: Uuid, status: RunStatus) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE runs SET status = $2 WHERE id = $1",
        id,
        status.to_string()
    )
    .execute(pool)
    .await?;
    Ok(())
}
```

- [ ] **Step 2: Run cargo check**

```bash
cd crates/db && cargo check
```

- [ ] **Step 3: Commit**

```bash
git add crates/db/src/runs.rs
git commit -m "feat(db): implement runs CRUD"
```

---

### Task 13: Implement nodes, edges, artifacts CRUD

**Files:**
- Create: `crates/db/src/nodes.rs`
- Create: `crates/db/src/edges.rs`
- Create: `crates/db/src/artifacts.rs`

- [ ] **Step 1: Create nodes.rs**

```rust
use sqlx::PgPool;
use types::{Node, NodeStatus};
use uuid::Uuid;

pub async fn create(pool: &PgPool, node: &Node) -> Result<Node, sqlx::Error> {
    sqlx::query_as!(
        Node,
        r#"
        INSERT INTO nodes (id, run_id, tenant_id, agent_type, instruction, status)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING *
        "#,
        node.id,
        node.run_id,
        node.tenant_id,
        node.agent_type,
        node.instruction,
        node.status.to_string()
    )
    .fetch_one(pool)
    .await
}

pub async fn get(pool: &PgPool, id: Uuid) -> Result<Option<Node>, sqlx::Error> {
    sqlx::query_as!(
        Node,
        "SELECT * FROM nodes WHERE id = $1",
        id
    )
    .fetch_optional(pool)
    .await
}

pub async fn update_status(pool: &PgPool, id: Uuid, status: NodeStatus) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE nodes SET status = $2 WHERE id = $1",
        id,
        status.to_string()
    )
    .execute(pool)
    .await?;
    Ok(())
}
```

- [ ] **Step 2: Create edges.rs**

```rust
use sqlx::PgPool;
use types::{Edge};
use uuid::Uuid;

pub async fn create(pool: &PgPool, edge: &Edge) -> Result<Edge, sqlx::Error> {
    sqlx::query_as!(
        Edge,
        r#"
        INSERT INTO edges (id, run_id, source_node, target_node)
        VALUES ($1, $2, $3, $4)
        RETURNING *
        "#,
        edge.id,
        edge.run_id,
        edge.source_node,
        edge.target_node
    )
    .fetch_one(pool)
    .await
}

pub async fn get_by_run(pool: &PgPool, run_id: Uuid) -> Result<Vec<Edge>, sqlx::Error> {
    sqlx::query_as!(
        Edge,
        "SELECT * FROM edges WHERE run_id = $1",
        run_id
    )
    .fetch_all(pool)
    .await
}
```

- [ ] **Step 3: Create artifacts.rs**

```rust
use sqlx::PgPool;
use types::{Artifact, StorageType};
use uuid::Uuid;

pub async fn create(pool: &PgPool, artifact: &Artifact) -> Result<Artifact, sqlx::Error> {
    sqlx::query_as!(
        Artifact,
        r#"
        INSERT INTO artifacts (id, node_id, tenant_id, storage, location, size_bytes, content_type, created_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING *
        "#,
        artifact.id,
        artifact.node_id,
        artifact.tenant_id,
        artifact.storage.to_string(),
        artifact.location,
        artifact.size_bytes,
        artifact.content_type,
        artifact.created_at
    )
    .fetch_one(pool)
    .await
}

pub async fn get_by_node(pool: &PgPool, node_id: Uuid) -> Result<Vec<Artifact>, sqlx::Error> {
    sqlx::query_as!(
        Artifact,
        "SELECT * FROM artifacts WHERE node_id = $1",
        node_id
    )
    .fetch_all(pool)
    .await
}
```

- [ ] **Step 4: Run cargo check**

```bash
cd crates/db && cargo check
```

- [ ] **Step 5: Commit**

```bash
git add crates/db/src/nodes.rs crates/db/src/edges.rs crates/db/src/artifacts.rs
git commit -m "feat(db): implement nodes, edges, artifacts CRUD"
```

---

## Phase 4: Queue Crate (Day 4)

### Task 14: Create queue crate scaffold

**Files:**
- Create: `crates/queue/Cargo.toml`
- Create: `crates/queue/src/lib.rs`
- Create: `crates/queue/src/error.rs`

- [ ] **Step 1: Create Cargo.toml**

```toml
[package]
name = "queue"
version = "0.1.0"
edition = "2021"

[dependencies]
types = { path = "../types" }
db = { path = "../db" }
sqlx = { version = "0.7", features = ["runtime-tokio", "postgres", "uuid", "chrono"] }
thiserror = "1"
tokio = { version = "1", features = ["full"] }
```

- [ ] **Step 2: Create lib.rs**

```rust
pub mod enqueue;
pub mod dequeue;
pub mod complete;
pub mod error;

pub use error::{QueueError, QueueErrorKind};
pub use enqueue::enqueue;
pub use dequeue::dequeue;
pub use complete::complete;
```

- [ ] **Step 3: Create error.rs**

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum QueueError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    
    #[error("Entry not found: {0}")]
    NotFound(String),
    
    #[error("Already locked: {0}")]
    AlreadyLocked(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueErrorKind {
    Database,
    NotFound,
    AlreadyLocked,
}
```

- [ ] **Step 4: Commit**

```bash
git add crates/queue/
git commit -m "feat(queue): create queue crate scaffold"
```

---

### Task 15: Implement enqueue operation

**Files:**
- Create: `crates/queue/src/enqueue.rs`

- [ ] **Step 1: Create enqueue.rs**

```rust
use sqlx::PgPool;
use types::{QueueEntry, QueueStatus};
use crate::error::QueueError;

pub async fn enqueue(
    pool: &PgPool,
    entry: &QueueEntry,
) -> Result<uuid::Uuid, QueueError> {
    let id = sqlx::query_scalar!(
        r#"
        INSERT INTO queue (id, tenant_id, run_id, node_id, priority, status, available_at, created_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (node_id) DO NOTHING
        RETURNING id
        "#,
        entry.id,
        entry.tenant_id,
        entry.run_id,
        entry.node_id,
        entry.priority,
        QueueStatus::Pending.to_string(),
        entry.available_at,
        entry.created_at
    )
    .fetch_optional(pool)
    .await?;
    
    Ok(id.unwrap_or(entry.id))
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/queue/src/enqueue.rs
git commit -m "feat(queue): implement enqueue with ON CONFLICT DO NOTHING"
```

---

### Task 16: Implement dequeue with SKIP LOCKED

**Files:**
- Create: `crates/queue/src/dequeue.rs`

- [ ] **Step 1: Create dequeue.rs**

```rust
use sqlx::PgPool;
use types::{QueueEntry, QueueStatus};
use crate::error::QueueError;

pub async fn dequeue(
    pool: &PgPool,
    tenant_id: uuid::Uuid,
    executor_id: &str,
) -> Result<Option<QueueEntry>, QueueError> {
    let entry = sqlx::query_as!(
        QueueEntry,
        r#"
        SELECT * FROM queue
        WHERE tenant_id = $1
          AND status = 'pending'
          AND available_at <= NOW()
        ORDER BY priority DESC, created_at ASC
        LIMIT 1
        FOR UPDATE SKIP LOCKED
        "#,
        tenant_id
    )
    .fetch_optional(pool)
    .await?;
    
    if let Some(ref e) = entry {
        sqlx::query!(
            r#"
            UPDATE queue
            SET status = 'locked', locked_at = NOW(), locked_by = $3
            WHERE id = $1
            "#,
            e.id,
            QueueStatus::Locked.to_string(),
            executor_id
        )
        .execute(pool)
        .await?;
    }
    
    Ok(entry)
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/queue/src/dequeue.rs
git commit -m "feat(queue): implement dequeue with SKIP LOCKED"
```

---

### Task 17: Implement complete and reset_to_pending

**Files:**
- Create: `crates/queue/src/complete.rs`

- [ ] **Step 1: Create complete.rs**

```rust
use sqlx::PgPool;
use types::QueueStatus;
use crate::error::QueueError;

pub async fn complete(pool: &PgPool, queue_id: uuid::Uuid) -> Result<(), QueueError> {
    sqlx::query!(
        r#"
        UPDATE queue SET status = $2 WHERE id = $1
        "#,
        queue_id,
        QueueStatus::Done.to_string()
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn reset_to_pending(pool: &PgPool, queue_id: uuid::Uuid) -> Result<(), QueueError> {
    sqlx::query!(
        r#"
        UPDATE queue
        SET status = 'pending', locked_at = NULL, locked_by = NULL, available_at = NOW()
        WHERE id = $1
        "#,
        queue_id
    )
    .execute(pool)
    .await?;
    Ok(())
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/queue/src/complete.rs
git commit -m "feat(queue): implement complete and reset_to_pending"
```

---

## Phase 5: Integration (Day 5)

### Task 18: Workspace verification

- [ ] **Step 1: Run cargo check --workspace**

```bash
cargo check --workspace
```

- [ ] **Step 2: Run cargo test --workspace**

```bash
cargo test --workspace
```

- [ ] **Step 3: Commit integration**

```bash
git add -A
git commit -m "feat: integrate all Phase 1 crates"
```

---

## Summary

| Phase | Tasks | Duration |
|-------|-------|----------|
| Phase 1: Types | Tasks 1-5 | Day 1 |
| Phase 2: DAG | Tasks 6-9 | Day 2 |
| Phase 3: DB | Tasks 10-13 | Day 3 |
| Phase 4: Queue | Tasks 14-17 | Day 4 |
| Phase 5: Integration | Task 18 | Day 5 |

**Total Estimated Time:** 5 days

---

**Plan complete and saved to** `docs/superpowers/plans/2025-04-05-torque-core-skeleton-plan.md`

**Two execution options:**

**1. Subagent-Driven (recommended)** - Dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints
