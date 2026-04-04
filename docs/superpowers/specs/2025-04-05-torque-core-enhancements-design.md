# Torque Core Enhancements Design

## Overview

This document describes the first batch of core enhancements for the Torque Agent Team System, incorporating lessons learned from LangGraph and DeepAgents. The focus is on three key areas: Checkpointer (state snapshots and crash recovery), Virtual File System (VFS), and Context Manager (intelligent context optimization).

**Date**: 2025-04-05  
**Status**: Design Approved  
**Scope**: Phase 1 - Core Functionality

---

## 1. Checkpointer: State Snapshots and Crash Recovery

### 1.1 Design Goals

- Support **time travel**: Resume execution from any historical checkpoint
- Replace current timeout-based crash recovery with a more reliable mechanism
- Pluggable storage backends for different use cases

### 1.2 Architecture

```
crates/
├── checkpointer/          # New crate
│   ├── trait Checkpointer
│   ├── PostgreSQLCheckpointer
│   ├── RedisCheckpointer  
   └── HybridCheckpointer # Default: PostgreSQL metadata + Redis snapshots
```

### 1.3 Core Trait

```rust
#[async_trait]
pub trait Checkpointer: Send + Sync {
    /// Save checkpoint, returns checkpoint_id
    async fn save(
        &self,
        run_id: Uuid,
        node_id: Uuid,
        state: CheckpointState,
    ) -> Result<CheckpointId>;
    
    /// Load specified checkpoint
    async fn load(&self, checkpoint_id: CheckpointId) -> Result<CheckpointState>;
    
    /// List all checkpoints for a Run (chronological order)
    async fn list_run_checkpoints(&self, run_id: Uuid) -> Result<Vec<CheckpointMeta>>;
    
    /// List all checkpoints for a Node
    async fn list_node_checkpoints(&self, node_id: Uuid) -> Result<Vec<CheckpointMeta>>;
    
    /// Delete specified checkpoint
    async fn delete(&self, checkpoint_id: CheckpointId) -> Result<()>;
}

pub struct CheckpointState {
    pub messages: Vec<Message>,           // Conversation history
    pub tool_call_count: u32,             // Executed tool call count
    pub intermediate_results: Vec<ArtifactPointer>, // Intermediate result references
    pub custom_state: Option<JsonValue>,  // Agent custom state
}

pub struct CheckpointMeta {
    pub id: CheckpointId,
    pub run_id: Uuid,
    pub node_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub state_hash: String,               // For deduplication
}
```

### 1.4 HybridCheckpointer Implementation (Default)

**Storage Strategy**:
- **PostgreSQL**: Checkpoint metadata (id, run_id, node_id, created_at, state_hash)
- **Redis**: State snapshots (TTL 24h), Key: `{tenant_id}:checkpoint:{checkpoint_id}`

**New Crash Recovery Flow**:
1. On Executor startup, query PostgreSQL for RUNNING nodes in last 24h
2. For each node, check Redis for latest checkpoint
3. **With checkpoint**: Restore state from checkpoint, resume execution
4. **Without checkpoint**: Reset to PENDING, follow original failure policy

### 1.5 When to Create Checkpoints

- **After tool calls**: Every time Tool Executor completes a call
- **Explicit request**: Agent calls the `create_checkpoint` tool
- **Configured interval**: Auto-create every N seconds (default 30s)

**Explicit Checkpoint Tool**:
```rust
{
    "name": "create_checkpoint",
    "description": "Create a checkpoint of current execution state for future recovery",
    "parameters": {
        "reason": "Optional string describing why checkpoint is being created"
    }
}
```

### 1.6 Database Schema Update

```sql
-- New checkpoints table
CREATE TABLE checkpoints (
    id            UUID PRIMARY KEY,
    run_id        UUID REFERENCES runs,
    node_id       UUID REFERENCES nodes,
    tenant_id     UUID REFERENCES tenants,
    state_hash    TEXT NOT NULL,          -- SHA256(state)
    storage       TEXT NOT NULL,          -- redis / s3
    location      TEXT NOT NULL,          -- Redis key or S3 path
    created_at    TIMESTAMPTZ,
    expires_at    TIMESTAMPTZ             -- For cleanup
);
```

---

## 2. Virtual File System (VFS): Unified File Abstraction

### 2.1 Design Goals

- Make Agent code more natural: work with file paths instead of Artifact Pointers
- Support collaborative workspace: multiple nodes sharing files
- Reuse ContextStore routing logic at the bottom layer

### 2.2 Architecture

```
crates/
├── context-store/         # Extended
│   ├── trait ContextStore (existing)
│   ├── trait VirtualFileSystem # New
│   ├── ContextStoreImpl (existing)
   └── VfsOverlay         # New: VFS implementation based on ContextStore
```

### 2.3 Core Trait

```rust
#[async_trait]
pub trait VirtualFileSystem: Send + Sync {
    /// Read file content
    async fn read(&self, path: &str) -> Result<Vec<u8>>;
    
    /// Write file, returns ArtifactPointer
    async fn write(&self, path: &str, content: &[u8]) -> Result<ArtifactPointer>;
    
    /// List directory contents
    async fn list(&self, dir: &str) -> Result<Vec<FileMeta>>;
    
    /// Check if file exists
    async fn exists(&self, path: &str) -> Result<bool>;
    
    /// Delete file
    async fn delete(&self, path: &str) -> Result<()>;
    
    /// Copy file (within same Run)
    async fn copy(&self, from: &str, to: &str) -> Result<ArtifactPointer>;
}

pub struct FileMeta {
    pub path: String,
    pub size_bytes: u64,
    pub content_type: String,
    pub modified_at: DateTime<Utc>,
    pub is_directory: bool,
}
```

### 2.4 Path Schema (Hybrid Mode)

**Absolute Paths** (full form):
```
/{tid}/{rid}/workspace/*     # Shared workspace, all nodes can read/write
/{tid}/{rid}/{nid}/*         # Node private space, only this node can write
/{tid}/{rid}/temp/*          # Temporary files, auto-cleanup after Run completion
```
Where: `tid` = tenant_id, `rid` = run_id, `nid` = node_id

**VFS Shortcuts** (resolved relative to current Run/Node context):
```
/workspace/file.json     → /{tid}/{rid}/workspace/file.json (shared)
/output/result.json      → /{tid}/{rid}/{nid}/output/result.json (node-private)
/temp/cache.json         → /{tid}/{rid}/temp/cache.json (temp)
```

**Path Resolution Examples**:
- `/workspace/shared_data.json` (shortcut) → `/123e4567-e89b-12d3-a456-426614174000/abc123/def456/workspace/shared_data.json`
- `/output/result.json` (shortcut) → `/123e4567-e89b-12d3-a456-426614174000/abc123/def456/output/result.json` (auto-mapped to current node)
- `temp/cache.json` (shortcut) → `/123e4567-e89b-12d3-a456-426614174000/abc123/def456/temp/cache.json`

**Note**: Agents use VFS shortcuts in code. The VfsOverlay resolves to absolute paths internally.

### 2.5 VFS to Artifact Mapping

```rust
impl VirtualFileSystem for VfsOverlay {
    async fn write(&self, path: &str, content: &[u8]) -> Result<ArtifactPointer> {
        // 1. Parse path, determine actual storage location
        let storage_path = self.resolve_path(path)?;
        
        // 2. Reuse ContextStore routing logic
        let pointer = self.context_store.write(content).await?;
        
        // 3. Update VFS metadata (stored in PostgreSQL)
        self.vfs_metadata.insert(storage_path, pointer.clone()).await?;
        
        Ok(pointer)
    }
    
    async fn read(&self, path: &str) -> Result<Vec<u8>> {
        // 1. Query VFS metadata to get ArtifactPointer
        let pointer = self.vfs_metadata.get(self.resolve_path(path)?).await?;
        
        // 2. Reuse ContextStore read
        self.context_store.read(&pointer).await
    }
}
```

### 2.6 Database Schema Update

```sql
-- New vfs_metadata table
CREATE TABLE vfs_metadata (
    id            UUID PRIMARY KEY,
    run_id        UUID REFERENCES runs,
    node_id       UUID REFERENCES nodes,    -- NULL means shared file
    path          TEXT NOT NULL,
    artifact_id   UUID REFERENCES artifacts,
    is_directory  BOOLEAN DEFAULT false,
    created_at    TIMESTAMPTZ,
    modified_at   TIMESTAMPTZ,
    UNIQUE(run_id, path)
);
```

### 2.7 Agent Code Example (After)

```rust
// Before (explicit Artifact Pointer handling)
let pointer = context_store.read_artifact(&upstream_pointer).await?;
let data: Value = serde_json::from_slice(&pointer.data)?;
let result = process(data);
let output_pointer = context_store.write_artifact(&result).await?;

// After (VFS abstraction)
let content = vfs.read_file("/workspace/input.json").await?;
let data: Value = serde_json::from_slice(&content)?;
let result = process(data);
vfs.write_file("/output/result.json", &result).await?;
```

---

## 3. Context Manager: Intelligent Context Optimization

### 3.1 Design Goals

- Automatically protect context window, prevent token overflow
- Dual-track storage: keep full audit logs, use compressed version for LLM
- Support explicit management tools

### 3.2 Architecture

```
crates/
├── agent-runtime/         # Extended
│   ├── AgentRuntime
│   ├── ContextManager     # New: context optimization management
   └── tools/
       └── context_tools   # New: context management tools
```

### 3.3 Core Components

```rust
pub struct ContextManager {
    max_tokens: usize,                    // Node's configured max_tokens
    warning_threshold: f64,               // Warning threshold (default 0.8)
    compression_strategy: CompressionStrategy,
    summarizer: Box<dyn Summarizer>,      // Summarizer (LLM-based)
}

pub enum CompressionStrategy {
    KeepLastN(usize),                    // Keep last N messages
    SummarizeOlder(SummaryConfig),       // Summarize early messages
    ExtractiveCompression,               // Extract key information
}

pub struct ContextState {
    pub full_history: Vec<Message>,       // Full history (for audit)
    pub compressed_context: Vec<Message>, // Compressed (for LLM prompt)
    pub summary_chain: Vec<Summary>,      // Summary chain
    pub token_count: usize,               // Current token count
}

pub struct Summary {
    pub covers_range: (usize, usize),     // Covered message range
    pub content: String,                  // Summary content
    pub created_at: DateTime<Utc>,
}
```

### 3.4 Trigger Strategy (Hybrid Mode)

**Automatic Trigger (Token Threshold)**:
```rust
impl ContextManager {
    pub async fn add_message(&mut self, msg: Message) -> Result<()> {
        self.full_history.push(msg.clone());
        
        let token_count = self.count_tokens(&self.full_history);
        
        // Check if exceeds threshold
        if token_count as f64 > self.max_tokens as f64 * self.warning_threshold {
            // Auto-compress
            self.compress().await?;
        }
        
        Ok(())
    }
}
```

**Explicit Tools** (Agent can call):
```rust
// context.compress tool
{
    "name": "context_compress",
    "description": "Actively compress context, retain key information",
    "parameters": {
        "strategy": "summarize",  // or "keep_last_n", "extractive"
        "keep_recent": 5          // Keep last N full messages
    }
}

// context.summarize_range tool
{
    "name": "context_summarize_range",
    "description": "Summarize messages in specified range",
    "parameters": {
        "start_idx": 0,
        "end_idx": 10
    }
}
```

### 3.5 Compression Implementation

```rust
impl ContextManager {
    async fn compress(&mut self) -> Result<()> {
        match &self.compression_strategy {
            CompressionStrategy::SummarizeOlder(config) => {
                // 1. Select messages to summarize (early ones)
                let to_summarize = &self.full_history[0..config.summarize_count];
                
                // 2. Generate summary using LLM
                let summary = self.summarizer.summarize(to_summarize).await?;
                
                // 3. Update compressed context
                self.compressed_context = vec![
                    Message::system(format!("Previous conversation summary: {}", summary))
                ];
                self.compressed_context.extend(
                    self.full_history[config.summarize_count..].to_vec()
                );
                
                // 4. Record summary chain (for audit)
                self.summary_chain.push(Summary {
                    covers_range: (0, config.summarize_count),
                    content: summary,
                    created_at: Utc::now(),
                });
            }
            // ... other strategies
        }
        
        Ok(())
    }
}
```

### 3.6 Dual-Track Storage (Option B)

- **full_history**: Complete messages, written to `node_logs` (permanent retention)
- **compressed_context**: Compressed version for LLM prompt, not persisted (in-memory only)
- **summary_chain**: Summary chain, written to `tool_calls` field in `node_logs`

### 3.7 Agent Runtime Integration

```rust
impl AgentRuntime {
    async fn execute(&self, node: Node) -> Result<AgentOutput> {
        // 1. Initialize ContextManager
        let mut context_mgr = ContextManager::new(
            node.max_tokens,
            self.config.compression_strategy.clone(),
        );
        
        // 2. Load upstream dependencies (via VFS)
        let upstream_files = self.load_upstream_files(&node).await?;
        
        // 3. Build initial context
        context_mgr.add_message(Message::system(node.system_prompt)).await?;
        context_mgr.add_message(Message::user(node.instruction)).await?;
        
        // 4. Tool call loop
        loop {
            // Use compressed_context to call LLM
            let response = self.llm
                .chat(context_mgr.get_compressed_context())
                .await?;
            
            // Add response to context (auto-triggers compression)
            context_mgr.add_message(response.clone()).await?;
            
            // Handle tool calls...
        }
    }
}
```

---

## 4. Updated Architecture Overview

```
Planner Service
├── Receive user task
├── Call LLM to generate DAG (structured JSON)
├── DAG validation (structural + semantic)
├── Persist to PostgreSQL
└── Push root nodes to task queue

PostgreSQL Queue (SKIP LOCKED)

Executor Service (multi-instance, horizontal scaling)
├── Scheduler (weighted round-robin, quota check)
└── Worker Pool
    └── Agent Runtime
        ├── Checkpointer (New: state snapshots + crash recovery)
        │   ├── HybridCheckpointer (PostgreSQL + Redis)
        │   └── Auto/explicit checkpoint creation
        ├── ContextManager (New: intelligent context optimization)
        │   ├── Token threshold auto-compression
        │   ├── Explicit context tools
        │   └── Dual-track storage (full logs + compressed prompt)
        ├── VirtualFileSystem (New: based on ContextStore)
        │   ├── Path abstraction: /workspace/ (shared), /output/ (private)
        │   └── Auto-routing to Redis/S3
        ├── ContextStore (existing: transparent routing Redis / S3)
        ├── LLM calls + tool call loop
        └── Tool Executor (permission check + execution log)

Storage Layer
├── Redis     Hot state, small results (< 256KB), concurrency counters, checkpoint snapshots
├── S3        Large results (>= 256KB), VFS large files
└── PostgreSQL DAG definitions, task states, artifact references, execution logs, usage records, checkpoint metadata, VFS metadata
```

---

## 5. Updated Crate Structure

```
agent-team/
├── crates/
│   ├── types/              Shared type definitions
│   ├── dag/                DAG data structures, validation, topological sorting
│   ├── checkpointer/       【New】State snapshots and crash recovery
│   │   ├── src/
│   │   │   ├── trait.rs
│   │   │   ├── hybrid.rs   # Default implementation
│   │   │   ├── postgres.rs
│   │   │   └── redis.rs
│   │   └── Cargo.toml
│   ├── db/                 PostgreSQL operations
│   ├── queue/              Task queue SKIP LOCKED
│   ├── context-store/      【Extended】ContextStore + VFS
│   │   ├── src/
│   │   │   ├── trait.rs
│   │   │   ├── store.rs    # Existing implementation
│   │   │   └── vfs.rs      # 【New】VFS implementation
│   │   └── Cargo.toml
│   ├── tool-executor/      Tool calls and permission checks
│   ├── agent-runtime/      【Extended】Agent execution lifecycle
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── runtime.rs
│   │   │   ├── context_mgr.rs  # 【New】Context management
│   │   │   └── tools/
│   │   │       └── context_tools.rs  # 【New】Context tools
│   │   └── Cargo.toml
│   ├── executor/           Executor Service entry
│   └── planner/            Planner Service entry
```

---

## 6. Dependency Updates

### 6.1 New Crates

**checkpointer** dependencies:
- `types` (local)
- `db` (local)
- `redis` (async Redis client)
- `async-trait`
- `serde`
- `serde_json`
- `chrono`
- `uuid`
- `sha2` (for state_hash)

### 6.2 Updated Crates

**context-store** new dependencies:
- `checkpointer` (optional, for checkpoint persistence)

**agent-runtime** new dependencies:
- `checkpointer`
- Extended `context-store` with VFS

---

## 7. Implementation Phases

### Phase 1: Checkpointer (Week 1)
1. Create `crates/checkpointer/` with trait definition
2. Implement `HybridCheckpointer`
3. Add `checkpoints` table migration
4. Integrate into `AgentRuntime`
5. Update crash recovery logic in `Executor`

### Phase 2: Virtual File System (Week 2)
1. Extend `context-store` with `VirtualFileSystem` trait
2. Implement `VfsOverlay`
3. Add `vfs_metadata` table migration
4. Create VFS tools (`read_file`, `write_file`, `list`, etc.)
5. Update Agent system prompts with VFS examples

### Phase 3: Context Manager (Week 3)
1. Create `ContextManager` in `agent-runtime`
2. Implement compression strategies
3. Create context management tools
4. Integrate into `AgentRuntime` execution loop
5. Add dual-track logging to `node_logs`

### Phase 4: Integration & Testing (Week 4)
1. End-to-end testing of all three components
2. Performance benchmarking
3. Documentation updates
4. Migration guide for existing code

---

## 8. Success Criteria

### Checkpointer
- [ ] Can save and load checkpoints from both PostgreSQL and Redis
- [ ] Crash recovery uses checkpoints when available
- [ ] Time travel: Can resume execution from any historical checkpoint
- [ ] State hash deduplication works correctly

### Virtual File System
- [ ] Agents can read/write files using natural paths
- [ ] Shared workspace (`/workspace/`) works across nodes
- [ ] Private output (`/output/`) automatically mapped to node
- [ ] Large files correctly routed to S3
- [ ] VFS metadata persisted in PostgreSQL

### Context Manager
- [ ] Auto-compression triggers at 80% token threshold
- [ ] Explicit compression tools work correctly
- [ ] Full history preserved in `node_logs`
- [ ] Compressed context used for LLM prompts
- [ ] Summary chain tracked for audit

---

## 9. Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Checkpoint frequency too high | Redis memory pressure | Configurable interval, auto-cleanup old checkpoints |
| VFS concurrent writes | Data corruption | Advisory locking with Redis (see Section 10) |
| Context compression loses critical info | Poor LLM responses | Multiple compression strategies, Agent can override |
| Migration complexity | Breaking changes | Maintain backward compatibility, gradual adoption |

---

## 10. VFS Concurrent Write Handling

**Design Decision**: Use advisory locking with Redis for concurrent write protection.

**Mechanism**:
- When writing to `/workspace/` (shared path), acquire a Redis lock: `{tid}:vfs:lock:{path}`
- Lock TTL: 30 seconds (prevents deadlocks if holder crashes)
- Lock acquisition uses `SET NX EX` (atomic set-if-not-exists with expiry)
- If lock cannot be acquired, return `Err(VfsError::ConcurrentWriteConflict)` to Agent
- Agent can retry after backoff or choose different filename

**Lock Key Format**: `{tenant_id}:vfs:lock:{resolved_absolute_path}`

**Example Flow**:
```rust
async fn write(&self, path: &str, content: &[u8]) -> Result<ArtifactPointer> {
    let resolved = self.resolve_path(path)?;
    
    // Acquire lock for shared paths
    if resolved.contains("/workspace/") {
        let lock_key = format!("{}:vfs:lock:{}", self.tenant_id, resolved);
        self.redis.set_nx_ex(&lock_key, self.executor_id, 30).await?
            .ok_or(VfsError::ConcurrentWriteConflict)?;
    }
    
    // ... write logic ...
    
    // Release lock on success
    if resolved.contains("/workspace/") {
        self.redis.del(&lock_key).await?;
    }
    
    Ok(pointer)
}
```

---

## 11. Open Questions

1. ~~How to handle VFS file locking for concurrent writes?~~ **Answered: Advisory locking with Redis**
2. Should we support checkpoint branching (multiple paths from one checkpoint)?
3. Should compressed context be cached in Redis for faster retrieval?
4. Do we need a web UI for checkpoint visualization (time travel)?

---

**Next Steps**: After design approval, proceed to implementation planning using the `writing-plans` skill.
