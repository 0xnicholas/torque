# Torque Memory System Design

**Date**: 2026-04-18  
**Status**: Final Draft  
**Scope**: Memory storage, retrieval, governance, and audit pipeline

---

## 1. Overview

Torque's memory system is designed as a **semantic retention plane** — not an archive of all outputs, but a selective, policy-governed pipeline that captures durable, recall-worthy knowledge.

### 1.1 Core Principles

1. **Candidate → Gating → Write → Audit**: Memory is not a binary write/no-write decision, but a complete pipeline
2. **Policy-governed**: All long-term writes pass through selective gating with policy evaluation
3. **Semantic retrieval**: Vector-based similarity search with hybrid keyword fallback
4. **Auditability**: Complete decision log for transparency and continuous improvement
5. **Layered storage**: Session memory (ephemeral) vs durable memory (long-term)

### 1.2 Architecture

```
Execution Output
      ↓
[Step 1: Candidate Generation]
      ↓
[Step 2: Memory Gating] ← Policy evaluation, dedup, equivalence check
      ↓
[Step 3: Write Policy]  ← Embedding, normalization, provenance
      ↓
[Memory Storage]        ← pgvector + PostgreSQL
      ↓
[Decision Log]          ← Audit trail (30-day retention)
```

---

## 2. Data Model

### 2.1 Core Entities

#### MemoryCategory (Enum)

```rust
pub enum MemoryCategory {
    AgentProfileMemory,      // Agent behavior preferences
    UserPreferenceMemory,    // User preferences
    TaskOrDomainMemory,      // Domain knowledge, task patterns
    EpisodicMemory,          // Event/experience memory (NEW)
    ExternalContextMemory,   // External context references
}
```

**Future evolution**: May upgrade to `Category + Kind` structure:
```rust
pub struct MemoryKind {
    category: MemoryCategory,
    sub_kind: String,  // e.g., "conversation", "tool_usage", "decision"
}
```

#### MemoryContent

```rust
pub struct MemoryContent {
    pub category: MemoryCategory,
    pub key: String,        // Semantic key for lookup
    pub value: serde_json::Value,  // Structured content
    pub metadata: MemoryMetadata,
}

pub struct MemoryMetadata {
    pub source_type: String,     // "execution", "manual", "nomination"
    pub source_ref: Option<Uuid>, // Task/agent instance ID
    pub confidence: f64,         // LLM confidence (0-1)
    pub timestamp: DateTime<Utc>,
    pub tags: Vec<String>,
}
```

### 2.2 Database Schema

#### v1_memory_entries (Extended)

```sql
CREATE TABLE v1_memory_entries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    agent_instance_id UUID,
    team_instance_id UUID,
    category VARCHAR(50) NOT NULL,
    key TEXT NOT NULL,
    value JSONB NOT NULL,
    source_candidate_id UUID,
    
    -- NEW: Embedding fields
    embedding vector(1536),           -- pgvector embedding
    embedding_model TEXT DEFAULT 'text-embedding-3-small',
    
    -- NEW: Usage tracking
    access_count INTEGER DEFAULT 0,
    last_accessed_at TIMESTAMPTZ,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes
CREATE INDEX idx_v1_memory_entries_category ON v1_memory_entries(category);
CREATE INDEX idx_v1_memory_entries_agent ON v1_memory_entries(agent_instance_id);

-- NEW: pgvector HNSW index for semantic search
CREATE INDEX idx_v1_memory_entries_embedding 
ON v1_memory_entries 
USING hnsw (embedding vector_cosine_ops);

-- NEW: Composite index for category-filtered search
CREATE INDEX idx_v1_memory_entries_category_embedding 
ON v1_memory_entries 
USING hnsw (embedding vector_cosine_ops) 
WHERE category = 'agent_profile_memory';
```

#### memory_candidates (Extended)

```sql
-- Reuse existing table, add status values
-- Status enum:
--   pending          → Initial state
--   review_required  → Needs human review
--   auto_approved    → Approved by gating automatically
--   approved         → Approved (by human or system)
--   rejected         → Rejected
--   merged           → Merged into existing memory

-- No schema changes needed if table already has status TEXT
```

#### session_memory (NEW)

```sql
CREATE TABLE session_memory (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID NOT NULL,
    key TEXT NOT NULL,
    value JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ,           -- TTL support
    
    UNIQUE(session_id, key)
);

CREATE INDEX idx_session_memory_session ON session_memory(session_id);
CREATE INDEX idx_session_memory_expires ON session_memory(expires_at) 
WHERE expires_at IS NOT NULL;
```

**Design rationale**: Session memory is KV/JSONB with TTL. Not vectorized in P0/P1. Used for temporary preferences and session state.

#### memory_decision_log (NEW)

```sql
CREATE TABLE memory_decision_log (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- References
    candidate_id UUID REFERENCES memory_candidates(id),
    entry_id UUID REFERENCES v1_memory_entries(id),
    
    -- Decision
    decision_type VARCHAR(20) NOT NULL,  -- approve / reject / merge / review
    decision_reason TEXT,
    
    -- Factors (structured)
    factors JSONB NOT NULL DEFAULT '{}',
    /* Example:
    {
        "quality_score": 0.92,
        "confidence": 0.88,
        "similarity_to_existing": 0.85,
        "equivalence_result": "mergeable",
        "risk_level": "low",
        "has_conflict": false,
        "consent_required": false
    }
    */
    
    -- Processing metadata
    processed_by VARCHAR(50) NOT NULL,  -- "auto" / "user:xxx" / "policy:xxx"
    processed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_decision_log_candidate ON memory_decision_log(candidate_id);
CREATE INDEX idx_decision_log_type ON memory_decision_log(decision_type);
CREATE INDEX idx_decision_log_processed_at ON memory_decision_log(processed_at);
```

---

## 3. Pipeline Design

### 3.1 Step 1: Candidate Generation

**Trigger sources**:
- `TaskCompleted` → Auto-extract from execution summary
- `ToolCallCompleted` → Selective extraction from tool results
- `Manual` → User/API explicitly creates candidate
- `ScheduledCompaction` → Periodic historical review

**Extraction strategy**:
```rust
pub struct CandidateGenerationConfig {
    pub enabled: bool,
    pub extraction_model: String,           // "gpt-4o-mini"
    pub max_candidates_per_execution: usize, // 5
    pub min_content_length: usize,          // 20 chars
    pub excluded_tools: Vec<String>,        // ["echo", "ping"]
}
```

**Output**: `MemoryCandidate` with confidence score (0-1)

### 3.2 Step 2: Memory Gating (Core)

#### 2.1 Quality Assessment

Four dimensions (weighted):
- Information density (30%)
- Specificity (30%)
- Timelessness (20%)
- Reusability (20%)

Overall score: weighted average → must be ≥ 0.88 for auto-approve

#### 2.2 Deduplication (Dynamic Thresholds)

| Memory Type | Duplicate | Mergeable | New |
|------------|-----------|-----------|-----|
| AgentProfile | ≥ 0.96 | 0.88 – 0.96 | < 0.88 |
| UserPreference | ≥ 0.96 | 0.88 – 0.96 | < 0.88 |
| TaskDomain | ≥ 0.95 | 0.85 – 0.95 | < 0.85 |
| Episodic | ≥ 0.94 | 0.85 – 0.94 | < 0.85 |
| ExternalContext | ≥ 0.93 | 0.80 – 0.93 | < 0.80 |

**Algorithm**:
1. Vector search (Top-5) to find closest existing memory
2. Compute cosine similarity
3. Apply type-specific threshold
4. If ≥ duplicate threshold → Equivalence check
5. If ≥ merge threshold → Mergeable
6. Else → Distinct

#### 2.3 Equivalence Check (On-Demand)

**Not field comparison — semantic judgment**:

```rust
enum EquivalenceResult {
    Equivalent,   // Semantic equivalence (dedup)
    Mergeable,    // Complementary (can merge)
    Conflict,     // Same key, different value
    Distinct,     // Clearly different
}
```

**Process**:
1. **Rules engine first** (fast path):
   - Same task + < 5 min + similarity > 0.96 → Equivalent
   - Same key + different value → Conflict
2. **LLM fallback** (slow path, < 10% cases):
   - Input: candidate + existing memory + metadata
   - Output: EquivalenceResult
   - Only invoked when rules are inconclusive

**Metadata considered**:
- time_delta
- same_session / same_task / same_agent
- content_similarity
- context_overlap

#### 2.4 Risk / Conflict / Consent

```rust
enum RiskLevel {
    Low,     // General facts → Auto process
    Medium,  // User preferences → Conditional
    High,    // Security/policy → Must review
}
```

**User preference risk-based handling**:
- Low-risk long-term preferences → Auto write
- High-impact preferences (DB config, API keys) → Review
- Temporary preferences → Session memory only

#### 2.5 Gate Decision

```rust
enum GateDecision {
    Approve { write_mode: WriteMode },
    Review { reason: String, priority: ReviewPriority },
    Merge { target_id: Uuid, strategy: MergeStrategy },
    Reject { reason: String, category: RejectionCategory },
}
```

**Auto-approve conditions**:
```
quality.overall >= 0.88
&& candidate.confidence >= 0.85
&& risk_level == Low
&& !has_conflict
&& !consent_required
&& category != ExternalContext
```

**Review trigger conditions**:
- Quality 0.75-0.88 or confidence 0.70-0.85
- Risk == Medium
- Potential conflict detected
- Consent required
- User preference with medium+ impact

### 3.3 Step 3: Write Policy

#### 3.1 Normalization

Content formatted by category:
- **AgentProfile**: Behavior patterns, preferences
- **UserPreference**: User choices, settings
- **TaskDomain**: Domain knowledge, patterns
- **Episodic**: Events with context, evidence chain
- **ExternalContext**: References with source links

#### 3.2 Embedding Generation

```rust
let text = format!("{}: {} - {}", 
    content.category, 
    content.key, 
    serde_json::to_string(&content.value)?
);
let embedding = embedding_generator.generate(&text).await?;
```

#### 3.3 Write Modes

```rust
enum WriteMode {
    Insert,                              // New entry
    Merge { target_id, strategy },       // Merge with existing
    Replace { target_id, reason },       // Replace existing
}

enum MergeStrategy {
    Summarize,      // Profile/Preference: generate new summary
    Append,         // Episodic: add to timeline
    KeepSeparate,   // Keep as distinct entries
    WithProvenance { preserve_evidence: bool },
}
```

#### 3.4 Provenance

```rust
struct ProvenanceLog {
    source_candidate_id: Uuid,
    source_execution_id: Option<Uuid>,
    source_task_id: Option<Uuid>,
    
    raw_evidence: Vec<String>,
    extraction_confidence: f64,
    
    gating_decision: GateDecision,
    decision_factors: DecisionFactors,
    
    merge_history: Vec<MergeRecord>,
}
```

---

## 4. API Design

### 4.1 Semantic Search

```http
POST /v1/memory-entries/search
Content-Type: application/json

{
    "query": "user prefers dark mode",
    "category": "user_preference_memory",
    "limit": 10,
    "hybrid": true,
    "vector_weight": 0.7,
    "keyword_weight": 0.3
}

Response:
{
    "data": [
        {
            "id": "uuid",
            "content": {...},
            "similarity_score": 0.92,
            "search_method": "hybrid"
        }
    ]
}
```

### 4.2 Manual Trigger

```http
POST /v1/memory-write-candidates
Content-Type: application/json

{
    "agent_instance_id": "uuid",
    "content": {
        "category": "user_preference_memory",
        "key": "theme_preference",
        "value": {"theme": "dark"}
    },
    "reasoning": "User explicitly stated preference",
    "bypass_gating": false
}
```

```http
POST /v1/memory-entries/force
Content-Type: application/json

{
    "content": {...},
    "write_mode": "Insert",
    "reason": "User forced write",
    "user_id": "user_xxx"
}
```

### 4.3 Review Lifecycle

```http
GET /v1/memory-write-candidates?status=review_required&limit=20

POST /v1/memory-write-candidates/{id}/approve
POST /v1/memory-write-candidates/{id}/reject
{
    "reason": "Duplicate of existing memory"
}
POST /v1/memory-write-candidates/{id}/merge
{
    "target_id": "existing_uuid",
    "strategy": "Summarize"
}
```

---

## 5. Configuration

```yaml
memory_system:
  # Storage
  embedding:
    provider: "openai"                    # or "local"
    model: "text-embedding-3-small"
    dimensions: 1536
    api_key: "${OPENAI_API_KEY}"
  
  # Search
  search:
    default_limit: 10
    hybrid_search: true
    vector_weight: 0.7
    keyword_weight: 0.3
  
  # Pipeline
  candidate_generation:
    enabled: true
    extraction_model: "gpt-4o-mini"
    max_candidates_per_execution: 5
    min_content_length: 20
  
  gating:
    auto_approve:
      quality_threshold: 0.88
      confidence_threshold: 0.85
    
    dedup_thresholds:
      agent_profile: { duplicate: 0.96, merge: 0.88 }
      user_preference: { duplicate: 0.96, merge: 0.88 }
      task_domain: { duplicate: 0.95, merge: 0.85 }
      episodic: { duplicate: 0.94, merge: 0.85 }
      external_context: { duplicate: 0.93, merge: 0.80 }
    
    merge_strategies:
      agent_profile: "Summarize"
      user_preference: "Summarize"
      task_domain: "Append"
      episodic: "Append"
      external_context: "KeepSeparate"
    
    risk_rules:
      user_preference:
        high_impact_fields: ["database_config", "api_keys", "security_policy"]
        auto_approve: "low_impact && long_term"
  
  # Session memory
  session_memory:
    enabled: true
    default_ttl_seconds: 3600              # 1 hour
  
  # Audit
  decision_log:
    retention_days: 30
    aggregation_enabled: true
```

---

## 6. Implementation Phases

### P0: Foundation (Week 1-2)

**Goal**: Semantic storage and retrieval online

- [ ] **P0.1** Memory table + pgvector
  - Install pgvector extension
  - Add embedding fields to v1_memory_entries
  - Create HNSW indexes
  - Add pgvector dependency

- [ ] **P0.2** Embedding write path
  - Define `EmbeddingGenerator` trait
  - Implement `OpenAIEmbeddingGenerator`
  - Integrate into MemoryService

- [ ] **P0.3** Semantic retrieval
  - `semantic_search()` repository method
  - Hybrid search (RRF fusion)
  - `POST /v1/memory-entries/search` API

- [ ] **P0.4** Session memory minimal store
  - `session_memory` table (KV + TTL)
  - `SessionMemoryRepository`
  - Internal service (no public API)

- [ ] **P0.5** EpisodicMemory enum support
  - Add to `MemoryCategory`
  - Update constraints
  - API handler support

- [ ] **P0.6** Embedding backfill job
  - Batch script for existing entries
  - Configurable batch size
  - Non-blocking to main path

- [ ] **P0.7** Category backfill plan
  - Script to infer category for historical data
  - Gradual labeling, no P0 full migration required

### P1: Pipeline Core (Week 3-5)

**Goal**: Three-stage pipeline operational

- [ ] **P1.1** Candidate generation
  - `CandidateGenerator` trait
  - `LLMCandidateGenerator` implementation
  - Integration with RunService (post-task)

- [ ] **P1.2** Gating framework
  - `MemoryGatingService`
  - Quality assessment
  - Risk/conflict/consent rules
  - Auto-approve logic

- [ ] **P1.3** Dedup
  - Dynamic thresholds by type
  - Vector search Top-5
  - Similarity computation

- [ ] **P1.4** Equivalence check (on-demand)
  - Rules engine (fast path)
  - LLM fallback (slow path, < 10%)
  - Metadata-aware judgment

### P2: Governance & Audit (Week 6-7)

**Goal**: Complete audit and governance capability

- [ ] **P2.1** Decision log
  - `memory_decision_log` table
  - `DecisionLogService`
  - Write on every gating decision

- [ ] **P2.2** Manual trigger APIs
  - `POST /v1/memory-write-candidates`
  - `POST /v1/memory-entries/force`
  - Permission checks

- [ ] **P2.3** Review lifecycle
  - Query review queue
  - Approve / reject / merge endpoints
  - Notification hooks (SSE/webhook)

### P3: Advanced Features (Future)

**Goal**: Analytics, visualization, auto-governance

- [ ] **P3.1** Analytics
  - Approval/rejection rates
  - Distribution by category/source
  - Search hit rates

- [ ] **P3.2** Provenance UI
  - Full source chain API
  - Pipeline visualization
  - Diff view for merges

- [ ] **P3.3** Compaction / summarization / governance
  - Automated similarity merge
  - Long-memory summarization
  - Category-specific TTL
  - Compliance scanning

---

## 7. Key Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Vector storage | pgvector | Zero additional dependencies, transactional consistency |
| Embedding | OpenAI (default) | High quality, swappable to local models |
| Dedup thresholds | Dynamic by type | Profile/Preference need stricter control than ExternalContext |
| Auto-approve | quality ≥ 0.88, confidence ≥ 0.85 | Balance precision and recall |
| Session memory | KV/JSONB + TTL | Minimal viable, not vectorized in P0/P1 |
| Decision log | Separate table, 30 days | Audit requirement without polluting main store |
| EpisodicMemory | New enum value | Explicitly separate experiences from knowledge |
| Equivalence check | Rules first, LLM fallback | Performance: 90%+ resolved by rules |
| Manual triggers | Must preserve | Human oversight and edge cases always supported |

---

## 8. Dependencies

### Rust Crates
```toml
[dependencies]
pgvector = { version = "0.4", features = ["sqlx"] }
reqwest = { version = "0.12", features = ["json"] }  # For OpenAI API
```

### PostgreSQL
- PostgreSQL ≥ 14
- pgvector extension

### External Services
- OpenAI API (embedding generation)
- Configurable to local models

---

## 9. Risk Mitigation

| Risk | Mitigation |
|------|-----------|
| OpenAI API latency | Batch generation, async write path |
| pgvector performance at scale | HNSW index, monitor query latency |
| LLM extraction hallucination | Confidence threshold, human review for low confidence |
| Duplicate memory growth | Dedup + compaction pipeline |
| Sensitive data in memory | Policy gating, consent checks, audit log |

---

## 10. Success Criteria

**P0 Complete**:
- [ ] Semantic search returns relevant results (top-5 accuracy > 80%)
- [ ] All new memory entries have embeddings
- [ ] Session memory works for temporary preferences
- [ ] No performance regression on existing APIs

**P1 Complete**:
- [ ] Auto-nomination creates candidates on task completion
- [ ] Gating correctly routes: approve / review / merge / reject
- [ ] Dedup catches > 90% of duplicate nominations
- [ ] Decision log records every gating outcome

**P2 Complete**:
- [ ] Manual trigger APIs functional
- [ ] Review queue manageable (< 24h turnaround)
- [ ] Full audit trail for compliance

---

## Appendix A: Memory Type Examples

| Type | Example Content | Retention |
|------|----------------|-----------|
| AgentProfile | "Agent prefers concise responses" | Long-term |
| UserPreference | "User likes dark mode" | Long-term |
| TaskDomain | "DB migration best practices" | Medium-term |
| Episodic | "2026-04-18: Fixed connection bug by changing timeout" | Medium-term |
| ExternalContext | "Reference: RFC 7231" | Short-term |

## Appendix B: Session Memory Examples

| Key | Value | TTL |
|-----|-------|-----|
| current_language | "zh-CN" | 1 hour |
| draft_email_recipient | "boss@company.com" | 30 min |
| last_query_context | {"topic": "memory_system", "depth": "technical"} | 15 min |

---

*This spec is the authoritative reference for Torque Memory System implementation.*
