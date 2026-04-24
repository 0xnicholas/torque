# P1: Deduplication Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement dynamic deduplication thresholds and semantic equivalence checking to improve Memory Gating accuracy.

**Architecture:** Extend MemoryGatingService with dynamic thresholds via GatingConfig, integrate equivalence checking serially after dedup, implement 4 merge strategies, and add comprehensive decision logging.

**Tech Stack:** Rust (tokio, sqlx, async-trait), PostgreSQL (pgvector), OpenAI API

---

## Task 1: Dynamic Threshold Configuration

### Files
- Modify: `crates/torque-harness/src/models/v1/gating.rs`
- Modify: `crates/torque-harness/src/config/memory.rs`
- Test: `crates/torque-harness/tests/dedup_thresholds_tests.rs`

- [ ] **Step 1: Add HashMap import and extend DedupThresholds in gating.rs**

Read `crates/torque-harness/src/models/v1/gating.rs` first.

Add to imports:
```rust
use std::collections::HashMap;
```

Add `minimum_content_length` field to DedupThresholds:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DedupThresholds {
    pub duplicate: f64,           // >= 此值视为重复
    pub merge: f64,              // >= 此值视为可合并
    pub minimum_content_length: usize,  // 新增: 最小内容长度
}
```

Add `from_config` and `with_env_override` methods:
```rust
impl DedupThresholds {
    pub fn for_category(category: &MemoryCategory) -> Self {
        match category {
            MemoryCategory::AgentProfileMemory | MemoryCategory::UserPreferenceMemory => {
                DedupThresholds { duplicate: 0.96, merge: 0.88, minimum_content_length: 10 }
            }
            MemoryCategory::TaskOrDomainMemory => DedupThresholds {
                duplicate: 0.95, merge: 0.85, minimum_content_length: 20,
            },
            MemoryCategory::EpisodicMemory => DedupThresholds {
                duplicate: 0.94, merge: 0.85, minimum_content_length: 30,
            },
            MemoryCategory::ExternalContextMemory => DedupThresholds {
                duplicate: 0.93, merge: 0.80, minimum_content_length: 5,
            },
        }
    }

    pub fn from_config(config: &GatingConfig, category: &MemoryCategory) -> Self {
        config.dedup_thresholds.get(category).cloned().unwrap_or_else(|| Self::for_category(category))
    }

    pub fn with_env_override(mut self, category: &MemoryCategory) -> Self {
        let prefix = format!("MEMORY_DEDUP_{}", category.to_env_suffix());
        if let Ok(v) = std::env::var(&format!("{}_DUPLICATE", prefix)) {
            if let Ok(val) = v.parse::<f64>() { self.duplicate = val; }
        }
        if let Ok(v) = std::env::var(&format!("{}_MERGE", prefix)) {
            if let Ok(val) = v.parse::<f64>() { self.merge = val; }
        }
        self
    }
}
```

- [ ] **Step 2: Add to_env_suffix to MemoryCategory in memory.rs**

Read `crates/torque-harness/src/models/v1/memory.rs`.

Add to MemoryCategory impl:
```rust
impl MemoryCategory {
    pub fn to_env_suffix(&self) -> String {
        match self {
            MemoryCategory::AgentProfileMemory => "AGENT_PROFILE".to_string(),
            MemoryCategory::UserPreferenceMemory => "USER_PREFERENCE".to_string(),
            MemoryCategory::TaskOrDomainMemory => "TASK_DOMAIN".to_string(),
            MemoryCategory::EpisodicMemory => "EPISODIC".to_string(),
            MemoryCategory::ExternalContextMemory => "EXTERNAL_CONTEXT".to_string(),
        }
    }
}
```

- [ ] **Step 3: Extend GatingConfig in gating.rs**

Add dedup_thresholds to GatingConfig:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatingConfig {
    pub auto_approve_quality_threshold: f64,
    pub auto_approve_confidence_threshold: f64,
    pub dedup_thresholds: HashMap<MemoryCategory, DedupThresholds>,  // 新增
}
```

- [ ] **Step 4: Update gating_config() in memory.rs**

Read `crates/torque-harness/src/config/memory.rs` first.

Replace the function:
```rust
use std::collections::HashMap;
use crate::models::v1::gating::DedupThresholds;
use crate::models::v1::memory::MemoryCategory;

pub fn gating_config() -> GatingConfig {
    let mut thresholds: HashMap<MemoryCategory, DedupThresholds> = HashMap::new();

    thresholds.insert(MemoryCategory::AgentProfileMemory, DedupThresholds {
        duplicate: 0.96, merge: 0.88, minimum_content_length: 10
    });
    thresholds.insert(MemoryCategory::UserPreferenceMemory, DedupThresholds {
        duplicate: 0.96, merge: 0.88, minimum_content_length: 10
    });
    thresholds.insert(MemoryCategory::TaskOrDomainMemory, DedupThresholds {
        duplicate: 0.95, merge: 0.85, minimum_content_length: 20
    });
    thresholds.insert(MemoryCategory::EpisodicMemory, DedupThresholds {
        duplicate: 0.94, merge: 0.85, minimum_content_length: 30
    });
    thresholds.insert(MemoryCategory::ExternalContextMemory, DedupThresholds {
        duplicate: 0.93, merge: 0.80, minimum_content_length: 5
    });

    // 环境变量覆盖
    for (category, thresholds) in thresholds.iter_mut() {
        *thresholds = thresholds.clone().with_env_override(category);
    }

    GatingConfig {
        auto_approve_quality_threshold: std::env::var("MEMORY_QUALITY_THRESHOLD")
            .ok().and_then(|v| v.parse().ok()).unwrap_or(0.88),
        auto_approve_confidence_threshold: std::env::var("MEMORY_CONFIDENCE_THRESHOLD")
            .ok().and_then(|v| v.parse().ok()).unwrap_or(0.85),
        dedup_thresholds: thresholds,
    }
}
```

- [ ] **Step 5: Add unit tests for DedupThresholds**

Create `crates/torque-harness/tests/dedup_thresholds_tests.rs`:
```rust
use torque_harness::models::v1::gating::{DedupThresholds, GatingConfig};
use torque_harness::models::v1::memory::MemoryCategory;

#[test]
fn test_dedup_thresholds_default_values() {
    let thresholds = DedupThresholds::for_category(&MemoryCategory::AgentProfileMemory);
    assert_eq!(thresholds.duplicate, 0.96);
    assert_eq!(thresholds.merge, 0.88);
    assert_eq!(thresholds.minimum_content_length, 10);
}

#[test]
fn test_dedup_thresholds_from_config() {
    let mut config = GatingConfig::default();
    config.dedup_thresholds.insert(
        MemoryCategory::AgentProfileMemory,
        DedupThresholds { duplicate: 0.99, merge: 0.95, minimum_content_length: 5 },
    );

    let thresholds = DedupThresholds::from_config(&config, &MemoryCategory::AgentProfileMemory);
    assert_eq!(thresholds.duplicate, 0.99);
    assert_eq!(thresholds.merge, 0.95);
    assert_eq!(thresholds.minimum_content_length, 5);
}

#[test]
fn test_dedup_thresholds_category_defaults() {
    let task = DedupThresholds::for_category(&MemoryCategory::TaskOrDomainMemory);
    assert_eq!(task.duplicate, 0.95);
    assert_eq!(task.merge, 0.85);

    let episodic = DedupThresholds::for_category(&MemoryCategory::EpisodicMemory);
    assert_eq!(episodic.duplicate, 0.94);
    assert_eq!(episodic.merge, 0.85);
}
```

- [ ] **Step 6: Run tests to verify**

Run: `cargo test -p torque-harness dedup_thresholds`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/torque-harness/src/models/v1/gating.rs crates/torque-harness/src/models/v1/memory.rs crates/torque-harness/src/config/memory.rs crates/torque-harness/tests/dedup_thresholds_tests.rs
git commit -m "feat(gating): add dynamic dedup thresholds configuration

- Add minimum_content_length to DedupThresholds
- Add from_config() and with_env_override() methods
- Extend GatingConfig with per-category thresholds
- Add environment variable override support (MEMORY_DEDUP_{CATEGORY}_DUPLICATE/MERGE)
- Add unit tests for DedupThresholds"
```

---

## Task 2: Add LLM Client to MemoryGatingService

### Files
- Modify: `crates/torque-harness/src/service/gating.rs`

- [ ] **Step 1: Read gating.rs to understand current structure**

- [ ] **Step 2: Add llm field to MemoryGatingService**

Find the struct definition and add:
```rust
use crate::llm::OpenAiClient;

pub struct MemoryGatingService {
    repo: Arc<dyn MemoryRepositoryV1>,
    embedding: Option<Arc<dyn EmbeddingGenerator>>,
    llm: Option<Arc<OpenAiClient>>,  // 新增
    gating_config: GatingConfig,
    candidate_config: CandidateGenerationConfig,
}
```

- [ ] **Step 3: Update new() method**

```rust
pub fn new(
    repo: Arc<dyn MemoryRepositoryV1>,
    embedding: Option<Arc<dyn EmbeddingGenerator>>,
    llm: Option<Arc<OpenAiClient>>,
) -> Self {
    Self {
        repo,
        embedding,
        llm,
        gating_config: config::gating_config(),
        candidate_config: config::candidate_generation_config(),
    }
}
```

- [ ] **Step 4: Run cargo check to verify compilation**

Run: `cargo check -p torque-harness`
Expected: No errors related to gating service

- [ ] **Step 5: Commit**

```bash
git add crates/torque-harness/src/service/gating.rs
git commit -m "feat(gating): add LLM client field to MemoryGatingService

MemoryGatingService now holds optional OpenAiClient for LLM-based equivalence checking"
```

---

## Task 3: Implement Equivalence Check Integration

### Files
- Modify: `crates/torque-harness/src/service/gating.rs`

- [ ] **Step 1: Modify check_dedup to use dynamic thresholds**

Read current `check_dedup` method (around line 183).

Replace with:
```rust
pub async fn check_dedup(
    &self,
    embedding: &[f32],
    category: &MemoryCategory,
    candidate_content: &MemoryContent,
) -> anyhow::Result<DedupResult> {
    let vector = crate::vector_type::Vector::from(embedding.to_vec());
    let similar = self
        .repo
        .find_similar_entries(&vector, Some(category), 5)
        .await?;

    // Use dynamic thresholds from config
    let thresholds = DedupThresholds::from_config(&self.gating_config, category);

    if similar.is_empty() {
        return Ok(DedupResult {
            similarity: 0.0,
            threshold_category: format!("{:?}", category),
            action: DedupAction::New,
            similar_entry_id: None,
        });
    }

    let best = similar.first().unwrap();

    let action = if best.similarity >= thresholds.duplicate {
        DedupAction::Duplicate
    } else if best.similarity >= thresholds.merge {
        DedupAction::Mergeable
    } else {
        DedupAction::New
    };

    Ok(DedupResult {
        similarity: best.similarity,
        threshold_category: format!("{:?}", category),
        action,
        similar_entry_id: Some(best.entry_id),
    })
}
```

- [ ] **Step 2: Add check_equivalence_for_candidate method**

Add after `check_dedup`:
```rust
pub async fn check_equivalence_for_candidate(
    &self,
    dedup_result: &DedupResult,
    category: &MemoryCategory,
    candidate_content: &MemoryContent,
) -> anyhow::Result<Option<EquivalenceResult>> {
    let thresholds = DedupThresholds::from_config(&self.gating_config, category);
    let similarity = dedup_result.similarity;

    // Determine if equivalence check is needed
    let should_check = match dedup_result.action {
        DedupAction::New => similarity >= thresholds.merge - 0.05,
        DedupAction::Mergeable => true,
        DedupAction::Duplicate => similarity < 0.98,
    };

    if !should_check {
        return Ok(None);
    }

    // Get similar entry for equivalence check
    if let Some(entry_id) = dedup_result.similar_entry_id {
        if let Some(existing_entry) = self.repo.get_entry_by_id(entry_id).await? {
            let input = EquivalenceCheckInput {
                candidate_content: serde_json::json!({
                    "category": candidate_content.category,
                    "key": candidate_content.key,
                    "value": candidate_content.value,
                }),
                existing_entry_id: entry_id,
                existing_content: serde_json::json!({
                    "category": existing_entry.category,
                    "key": existing_entry.key,
                    "value": existing_entry.value,
                }),
                time_delta_seconds: None,
                same_session: false,
                same_task: false,
                same_agent: true,
                content_similarity: similarity,
            };

            return Ok(Some(self.check_equivalence(&input).await?));
        }
    }

    Ok(None)
}
```

- [ ] **Step 3: Run cargo check**

Run: `cargo check -p torque-harness`
Expected: No errors

- [ ] **Step 4: Commit**

```bash
git add crates/torque-harness/src/service/gating.rs
git commit -m "feat(gating): integrate equivalence checking into dedup flow

- check_dedup now uses dynamic thresholds from GatingConfig
- Add check_equivalence_for_candidate for serial equivalence checking
- Equivalence check triggered on boundary cases and mergeable results"
```

---

## Task 4: Add LLM Retry and Fallback

### Files
- Modify: `crates/torque-harness/src/service/gating.rs`

- [ ] **Step 1: Read current check_equivalence_via_llm method**

Located around line 248.

- [ ] **Step 2: Add retry logic to check_equivalence_via_llm**

Replace the method with:
```rust
pub async fn check_equivalence_via_llm(
    &self,
    input: &EquivalenceCheckInput,
) -> anyhow::Result<EquivalenceResult> {
    let Some(llm) = &self.llm else {
        return Ok(EquivalenceResult::Distinct);
    };

    let api_key = match config::extraction_api_key() {
        Some(key) => key,
        None => return Ok(EquivalenceResult::Distinct),
    };

    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let base_url = config::extraction_api_base();
    let model = config::extraction_model();

    let prompt = format!(
        "Compare these two memory entries and determine if they are semantically equivalent, mergeable, conflicting, or distinct.\n\n\
        Entry 1: {}\n\n\
        Entry 2: {}\n\n\
        Respond with ONLY one word: Equivalent, Mergeable, Conflict, or Distinct",
        input.candidate_content, input.existing_content
    );

    let body = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "system", "content": "You are a memory equivalence checker. Respond with exactly one word."},
            {"role": "user", "content": prompt}
        ],
        "temperature": 0.1,
        "max_tokens": 20
    });

    let url = format!("{}/chat/completions", base_url);

    // Retry logic
    let max_retries = 3;
    let mut last_error = None;

    for attempt in 0..max_retries {
        match http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
        {
            Ok(response) => {
                let status = response.status();
                if !status.is_success() {
                    last_error = Some(format!("HTTP {}", status));
                    continue;
                }

                #[derive(serde::Deserialize)]
                struct ChatResponse {
                    choices: Vec<Choice>,
                }
                #[derive(serde::Deserialize)]
                struct Choice {
                    message: MessageContent,
                }
                #[derive(serde::Deserialize)]
                struct MessageContent {
                    content: String,
                }

                match response.json::<ChatResponse>().await {
                    Ok(chat_response) => {
                        let content = chat_response
                            .choices
                            .first()
                            .map(|c| c.message.content.trim().to_lowercase())
                            .unwrap_or_default();

                        let result = match content.as_str() {
                            c if c.contains("equivalent") => EquivalenceResult::Equivalent,
                            c if c.contains("conflict") => EquivalenceResult::Conflict,
                            c if c.contains("mergeable") => EquivalenceResult::Mergeable,
                            _ => EquivalenceResult::Distinct,
                        };

                        return Ok(result);
                    }
                    Err(e) => {
                        last_error = Some(e.to_string());
                        continue;
                    }
                }
            }
            Err(e) => {
                last_error = Some(e.to_string());
                continue;
            }
        }
    }

    // All retries failed - fallback to Distinct with warning
    log::warn!("LLM equivalence check failed after {} retries: {:?}", max_retries, last_error);
    Ok(EquivalenceResult::Distinct)
}
```

- [ ] **Step 3: Run cargo check**

Run: `cargo check -p torque-harness`
Expected: No errors

- [ ] **Step 4: Commit**

```bash
git add crates/torque-harness/src/service/gating.rs
git commit -m "feat(gating): add retry logic to LLM equivalence check

- Add 3 retry attempts for LLM API calls
- Fallback to Distinct on all retries failing
- Log warnings for failed LLM calls"
```

---

## Task 5: Add Conflict Detection

### Files
- Modify: `crates/torque-harness/src/models/v1/gating.rs`
- Modify: `crates/torque-harness/src/service/gating.rs`

- [ ] **Step 1: Add ConflictResult and ConflictType to gating.rs**

Add after EquivalenceResult:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictResult {
    pub has_conflict: bool,
    pub conflict_type: ConflictType,
    pub resolution: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConflictType {
    Overwrite,
    KeepBoth,
    MergeRequired,
    Discard,
}
```

- [ ] **Step 2: Add detect_conflict method to gating.rs**

Add after `check_equivalence_for_candidate`:
```rust
pub async fn detect_conflict(
    &self,
    candidate: &MemoryContent,
    dedup_result: &DedupResult,
) -> anyhow::Result<ConflictResult> {
    if let Some(entry_id) = dedup_result.similar_entry_id {
        if let Some(existing) = self.repo.get_entry_by_id(entry_id).await? {
            if candidate.key != existing.key {
                return Ok(ConflictResult {
                    has_conflict: true,
                    conflict_type: ConflictType::MergeRequired,
                    resolution: format!(
                        "Similar content with different keys: '{}' vs '{}'",
                        candidate.key, existing.key
                    ),
                });
            }
        }
    }

    Ok(ConflictResult {
        has_conflict: false,
        conflict_type: ConflictType::MergeRequired,
        resolution: String::new(),
    })
}
```

- [ ] **Step 3: Run cargo check**

Run: `cargo check -p torque-harness`
Expected: No errors

- [ ] **Step 4: Commit**

```bash
git add crates/torque-harness/src/models/v1/gating.rs crates/torque-harness/src/service/gating.rs
git commit -m "feat(gating): add conflict detection

- Add ConflictResult and ConflictType types
- Add detect_conflict method for identifying semantic conflicts
- Conflict detected when keys differ but content is similar"
```

---

## Task 6: Create Merge Strategy Module

### Files
- Create: `crates/torque-harness/src/service/merge_strategy.rs`
- Test: `crates/torque-harness/tests/merge_strategy_tests.rs`

- [ ] **Step 1: Create merge_strategy.rs**

```rust
use async_trait::async_trait;
use crate::models::v1::gating::{MergeStrategy, MemoryContent, MemoryEntry};
use crate::models::v1::memory::MemoryEntry;
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvenanceEntry {
    pub source: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug)]
pub struct MergedMemoryEntry {
    pub key: String,
    pub value: serde_json::Value,
    pub provenance: Vec<ProvenanceEntry>,
}

#[async_trait]
pub trait MergeStrategyHandler: Send + Sync {
    async fn merge(
        &self,
        candidate: &MemoryContent,
        existing: &MemoryEntry,
    ) -> Result<MergedMemoryEntry>;
}

// ============ Append Strategy ============

pub struct AppendStrategy;

#[async_trait]
impl MergeStrategyHandler for AppendStrategy {
    async fn merge(
        &self,
        candidate: &MemoryContent,
        existing: &MemoryEntry,
    ) -> Result<MergedMemoryEntry> {
        let mut values = Vec::new();

        if let serde_json::Value::Array(arr) = &existing.value {
            values.extend(arr.clone());
        } else {
            values.push(existing.value.clone());
        }

        let new_value_str = candidate.value.to_string();
        if !values.iter().any(|v| v.to_string() == new_value_str) {
            values.push(candidate.value.clone());
        }

        Ok(MergedMemoryEntry {
            key: existing.key.clone(),
            value: serde_json::Value::Array(values),
            provenance: vec![ProvenanceEntry {
                source: existing.id.to_string(),
                method: "append".to_string(),
                timestamp: None,
            }],
        })
    }
}

// ============ KeepSeparate Strategy ============

pub struct KeepSeparateStrategy;

#[async_trait]
impl MergeStrategyHandler for KeepSeparateStrategy {
    async fn merge(
        &self,
        candidate: &MemoryContent,
        existing: &MemoryEntry,
    ) -> Result<MergedMemoryEntry> {
        Ok(MergedMemoryEntry {
            key: candidate.key.clone(),
            value: serde_json::json!({
                "_type": "separate_entries",
                "entries": [
                    { "id": existing.id.to_string(), "key": existing.key, "value": existing.value },
                    { "key": candidate.key, "value": candidate.value }
                ]
            }),
            provenance: vec![ProvenanceEntry {
                source: existing.id.to_string(),
                method: "keep_separate".to_string(),
                timestamp: None,
            }],
        })
    }
}

// ============ WithProvenance Strategy ============

pub struct WithProvenanceStrategy;

#[async_trait]
impl MergeStrategyHandler for WithProvenanceStrategy {
    async fn merge(
        &self,
        candidate: &MemoryContent,
        existing: &MemoryEntry,
    ) -> Result<MergedMemoryEntry> {
        let mut provenance = Vec::new();

        if let serde_json::Value::Object(obj) = &existing.value {
            if let Some(provenances) = obj.get("_provenance") {
                if let serde_json::Value::Array(arr) = provenances {
                    for p in arr {
                        provenance.push(serde_json::from_value(p.clone())?);
                    }
                }
            }
        }

        provenance.push(ProvenanceEntry {
            source: candidate.key.clone(),
            method: "merged".to_string(),
            timestamp: Some(chrono::Utc::now()),
        });

        let mut new_value = candidate.value.clone();
        if let serde_json::Value::Object(ref mut obj) = new_value {
            obj.insert("_provenance".to_string(), serde_json::to_value(&provenance)?);
        }

        Ok(MergedMemoryEntry {
            key: existing.key.clone(),
            value: new_value,
            provenance,
        })
    }
}

// ============ Summarize Strategy ============

use crate::llm::OpenAiClient;
use std::sync::Arc;

pub struct SummarizeStrategy {
    http_client: reqwest::Client,
    api_base: String,
    api_key: String,
    model: String,
}

impl SummarizeStrategy {
    pub fn new(llm: Arc<OpenAiClient>) -> Self {
        Self {
            http_client: reqwest::Client::new(),
            api_base: crate::config::extraction_api_base(),
            api_key: crate::config::extraction_api_key().unwrap_or_default(),
            model: crate::config::extraction_model(),
        }
    }
}

#[async_trait]
impl MergeStrategyHandler for SummarizeStrategy {
    async fn merge(
        &self,
        candidate: &MemoryContent,
        existing: &MemoryEntry,
    ) -> Result<MergedMemoryEntry> {
        let prompt = format!(
            "Given these two memory entries about the same topic, create a consolidated summary:\n\n\
            Existing: {} - {}\n\
            New: {} - {}\n\n\
            Create a single consolidated entry.",
            existing.key, existing.value,
            candidate.key, candidate.value
        );

        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": "You are a memory consolidation assistant."},
                {"role": "user", "content": prompt}
            ],
            "temperature": 0.3,
            "max_tokens": 500
        });

        let url = format!("{}/chat/completions", self.api_base);
        let response = self.http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?
            .text()
            .await?;

        let consolidated: serde_json::Value = serde_json::from_str(&response)
            .unwrap_or_else(|_| serde_json::json!({
                "key": candidate.key.clone(),
                "value": {
                    "original": existing.value,
                    "new": candidate.value.clone(),
                    "summary": response
                }
            }));

        Ok(MergedMemoryEntry {
            key: candidate.key.clone(),
            value: consolidated,
            provenance: vec![
                ProvenanceEntry { source: existing.id.to_string(), method: "original".to_string(), timestamp: None },
                ProvenanceEntry { source: candidate.key.clone(), method: "summarized".to_string(), timestamp: None },
            ],
        })
    }
}

// ============ MergeStrategyExecutor ============

pub struct MergeStrategyExecutor {
    summarize: SummarizeStrategy,
    append: AppendStrategy,
    keep_separate: KeepSeparateStrategy,
    with_provenance: WithProvenanceStrategy,
}

impl MergeStrategyExecutor {
    pub fn new(llm: Arc<OpenAiClient>) -> Self {
        Self {
            summarize: SummarizeStrategy::new(llm),
            append: AppendStrategy,
            keep_separate: KeepSeparateStrategy,
            with_provenance: WithProvenanceStrategy,
        }
    }

    pub async fn execute(
        &self,
        strategy: MergeStrategy,
        candidate: &MemoryContent,
        existing: &MemoryEntry,
    ) -> Result<MergedMemoryEntry> {
        match strategy {
            MergeStrategy::Summarize => self.summarize.merge(candidate, existing).await,
            MergeStrategy::Append => self.append.merge(candidate, existing).await,
            MergeStrategy::KeepSeparate => self.keep_separate.merge(candidate, existing).await,
            MergeStrategy::WithProvenance => self.with_provenance.merge(candidate, existing).await,
        }
    }
}
```

- [ ] **Step 2: Run cargo check**

Run: `cargo check -p torque-harness`
Expected: No errors

- [ ] **Step 3: Create unit tests**

Create `crates/torque-harness/tests/merge_strategy_tests.rs`:
```rust
use torque_harness::models::v1::gating::MergeStrategy;
use torque_harness::models::v1::memory::{MemoryCategory, MemoryEntry, MemoryContent};
use torque_harness::service::merge_strategy::{AppendStrategy, KeepSeparateStrategy, WithProvenanceStrategy, MergeStrategyExecutor, MergedMemoryEntry};
use torque_harness::llm::OpenAiClient;
use std::sync::Arc;
use serde_json::json;

fn create_test_existing() -> MemoryEntry {
    MemoryEntry {
        id: uuid::Uuid::new_v4(),
        agent_instance_id: Some(uuid::Uuid::new_v4()),
        team_instance_id: None,
        category: MemoryCategory::AgentProfileMemory,
        key: "test_key".to_string(),
        value: json!("original value"),
        source_candidate_id: None,
        embedding: None,
        embedding_model: None,
        access_count: 0,
        last_accessed_at: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}

fn create_test_candidate() -> MemoryContent {
    MemoryContent {
        category: MemoryCategory::AgentProfileMemory,
        key: "test_key".to_string(),
        value: json!("new value"),
    }
}

#[tokio::test]
async fn test_append_strategy_creates_array() {
    let strategy = AppendStrategy;
    let existing = create_test_existing();
    let candidate = create_test_candidate();

    let result = strategy.merge(&candidate, &existing).await.unwrap();

    assert!(matches!(result.value, serde_json::Value::Array(arr) if arr.len() == 2));
    assert_eq!(result.key, "test_key");
}

#[tokio::test]
async fn test_append_strategy_deduplicates() {
    let strategy = AppendStrategy;
    let mut existing = create_test_existing();
    existing.value = json!(["value1", "value2"]);
    let mut candidate = create_test_candidate();
    candidate.value = json!("value1"); // duplicate

    let result = strategy.merge(&candidate, &existing).await.unwrap();

    if let serde_json::Value::Array(arr) = result.value {
        assert_eq!(arr.len(), 2); // Should not add duplicate
    } else {
        panic!("Expected array");
    }
}

#[tokio::test]
async fn test_keep_separate_strategy() {
    let strategy = KeepSeparateStrategy;
    let existing = create_test_existing();
    let candidate = create_test_candidate();

    let result = strategy.merge(&candidate, &existing).await.unwrap();

    assert!(result.value.get("_type").is_some());
    assert_eq!(result.value["_type"], "separate_entries");
}

#[tokio::test]
async fn test_with_provenance_strategy() {
    let strategy = WithProvenanceStrategy;
    let existing = create_test_existing();
    let candidate = create_test_candidate();

    let result = strategy.merge(&candidate, &existing).await.unwrap();

    assert_eq!(result.provenance.len(), 1);
    assert_eq!(result.provenance[0].method, "merged");
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p torque-harness merge_strategy`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/torque-harness/src/service/merge_strategy.rs crates/torque-harness/tests/merge_strategy_tests.rs
git commit -m "feat(gating): implement four merge strategies

- AppendStrategy: combines values into array with dedup
- KeepSeparateStrategy: stores as separate entries
- WithProvenanceStrategy: tracks merge history
- SummarizeStrategy: uses LLM for consolidation
- Add MergeStrategyExecutor to route to appropriate handler"
```

---

## Task 7: Rewrite gate_candidate with Full Flow

### Files
- Modify: `crates/torque-harness/src/service/gating.rs`

- [ ] **Step 1: Read current gate_candidate method**

Located around line 334.

- [ ] **Step 2: Add helper methods for content_to_text and make_conflict_decision**

Add after `detect_conflict`:
```rust
fn content_to_text(content: &MemoryContent) -> String {
    format!("{}: {} - {}", content.category.to_env_suffix(), content.key, content.value)
}

fn make_conflict_decision(conflict: ConflictResult) -> GateDecision {
    GateDecision {
        decision: GateDecisionType::Review,
        priority: Some(ReviewPriority::High),
        reason: conflict.resolution,
        write_mode: None,
        target_entry_id: None,
    }
}
```

- [ ] **Step 3: Add resolve_with_rules method**

Add after `make_conflict_decision`:
```rust
async fn resolve_with_rules(
    &self,
    dedup_result: &DedupResult,
    equivalence_result: Option<&EquivalenceResult>,
) -> anyhow::Result<GateDecision> {
    let equiv = equivalence_result.cloned().unwrap_or(EquivalenceResult::Distinct);

    match (&dedup_result.action, &equiv) {
        (DedupAction::Duplicate, EquivalenceResult::Distinct) if dedup_result.similarity < 0.98 => {
            // Fallback to LLM
            let llm_result = self.check_equivalence_via_llm_with_fallback(dedup_result).await?;
            Ok(self.llm_result_to_decision(llm_result, dedup_result)?)
        }
        (DedupAction::Duplicate, EquivalenceResult::Mergeable) => {
            Ok(GateDecision {
                decision: GateDecisionType::Merge,
                write_mode: Some(WriteMode::Merge {
                    target_id: dedup_result.similar_entry_id.unwrap(),
                    strategy: MergeStrategy::Summarize,
                }),
                reason: "Duplicate content but semantically mergeable".to_string(),
                ..Default::default()
            })
        }
        (DedupAction::Mergeable, EquivalenceResult::Equivalent) => {
            Ok(GateDecision {
                decision: GateDecisionType::Merge,
                write_mode: Some(WriteMode::Merge {
                    target_id: dedup_result.similar_entry_id.unwrap(),
                    strategy: MergeStrategy::WithProvenance,
                }),
                reason: "Semantically equivalent entries".to_string(),
                ..Default::default()
            })
        }
        (DedupAction::Mergeable, EquivalenceResult::Conflict) => {
            Ok(GateDecision {
                decision: GateDecisionType::Review,
                priority: Some(ReviewPriority::High),
                reason: "Mergeable but semantic conflict detected".to_string(),
                ..Default::default()
            })
        }
        (DedupAction::New, EquivalenceResult::Mergeable) => {
            Ok(GateDecision {
                decision: GateDecisionType::Merge,
                write_mode: Some(WriteMode::Merge {
                    target_id: dedup_result.similar_entry_id.unwrap(),
                    strategy: MergeStrategy::Append,
                }),
                reason: "New entry but similar to existing - appending".to_string(),
                ..Default::default()
            })
        }
        (DedupAction::New, EquivalenceResult::Distinct) => {
            Ok(GateDecision {
                decision: GateDecisionType::Approve,
                write_mode: Some(WriteMode::Insert),
                reason: "New distinct entry".to_string(),
                ..Default::default()
            })
        }
        _ => {
            Ok(GateDecision {
                decision: GateDecisionType::Review,
                priority: Some(ReviewPriority::Low),
                reason: format!("Default review: dedup={:?}, equiv={:?}", dedup_result.action, equiv),
                ..Default::default()
            })
        }
    }
}

async fn check_equivalence_via_llm_with_fallback(
    &self,
    dedup_result: &DedupResult,
) -> anyhow::Result<Option<EquivalenceResult>> {
    if let Some(entry_id) = dedup_result.similar_entry_id {
        if let Some(existing) = self.repo.get_entry_by_id(entry_id).await? {
            let input = EquivalenceCheckInput {
                candidate_content: serde_json::json!({}),
                existing_entry_id: entry_id,
                existing_content: existing.value,
                time_delta_seconds: None,
                same_session: false,
                same_task: false,
                same_agent: true,
                content_similarity: dedup_result.similarity,
            };
            return Ok(Some(self.check_equivalence_via_llm(&input).await?));
        }
    }
    Ok(None)
}

fn llm_result_to_decision(
    &self,
    result: Option<EquivalenceResult>,
    dedup_result: &DedupResult,
) -> anyhow::Result<GateDecision> {
    match result {
        Some(EquivalenceResult::Equivalent) | Some(EquivalenceResult::Mergeable) => {
            Ok(GateDecision {
                decision: GateDecisionType::Merge,
                write_mode: Some(WriteMode::Merge {
                    target_id: dedup_result.similar_entry_id.unwrap(),
                    strategy: MergeStrategy::Append,
                }),
                reason: "LLM confirmed mergeable".to_string(),
                ..Default::default()
            })
        }
        Some(EquivalenceResult::Conflict) => {
            Ok(GateDecision {
                decision: GateDecisionType::Review,
                priority: Some(ReviewPriority::High),
                reason: "LLM detected conflict".to_string(),
                ..Default::default()
            })
        }
        _ => {
            Ok(GateDecision {
                decision: GateDecisionType::Review,
                priority: Some(ReviewPriority::Medium),
                reason: "Ambiguous dedup result - LLM fallback inconclusive".to_string(),
                ..Default::default()
            })
        }
    }
}
```

- [ ] **Step 4: Rewrite gate_candidate method**

Replace the existing `gate_candidate` method:
```rust
pub async fn gate_candidate(
    &self,
    candidate: &MemoryWriteCandidate,
) -> anyhow::Result<GateDecision> {
    // Step 1: Parse content
    let content: MemoryContent = serde_json::from_value(candidate.content.clone())
        .context("Failed to parse memory content")?;

    // Step 2: Quality assessment
    let quality = self.assess_quality(&content);

    // Step 3: Risk assessment
    let risk = self.evaluate_risk(&content).await;

    // Step 4: Generate embedding
    let embedding = match &self.embedding {
        Some(emb) => Some(emb.generate(&content_to_text(&content)).await?),
        None => None,
    };

    // Step 5: Dedup check (using dynamic thresholds)
    let dedup_result = if let Some(ref emb) = embedding {
        self.check_dedup(emb, &content.category, &content).await?
    } else {
        DedupResult {
            similarity: 0.0,
            threshold_category: format!("{:?}", content.category),
            action: DedupAction::New,
            similar_entry_id: None,
        }
    };

    // Step 6: Equivalence check (serial)
    let equivalence_result = if embedding.is_some() {
        self.check_equivalence_for_candidate(&dedup_result, &content.category, &content).await?
    } else {
        None
    };

    // Step 7: Conflict detection
    let conflict_result = if let Some(EquivalenceResult::Conflict) = &equivalence_result {
        Some(self.detect_conflict(&content, &dedup_result).await?)
    } else {
        None
    };

    // Step 8: Decision
    let decision = if conflict_result.as_ref().map(|c| c.has_conflict).unwrap_or(false) {
        make_conflict_decision(conflict_result.unwrap())
    } else if embedding.is_some() {
        self.resolve_with_rules(&dedup_result, equivalence_result.as_ref()).await?
    } else {
        self.make_decision(&content, &quality, &risk, &dedup_result).await?
    };

    // Step 9: Log decision
    self.log_decision(candidate, &quality, &risk, &dedup_result, &decision).await?;

    Ok(decision)
}
```

- [ ] **Step 5: Run cargo check**

Run: `cargo check -p torque-harness`
Expected: No errors (may have warnings)

- [ ] **Step 6: Run all gating tests**

Run: `cargo test -p torque-harness gating`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/torque-harness/src/service/gating.rs
git commit -m "feat(gating): rewrite gate_candidate with full dedup-equivalence flow

- gate_candidate now orchestrates: quality -> risk -> dedup -> equivalence -> conflict -> decision
- Add resolve_with_rules for decision matrix logic
- Add check_equivalence_via_llm_with_fallback for boundary cases
- Add llm_result_to_decision to convert LLM results to decisions
- Decision logging at end of gating flow"
```

---

## Task 8: Add Configuration Validation

### Files
- Modify: `crates/torque-harness/src/models/v1/gating.rs`

- [ ] **Step 1: Add ConfigError and GatingConfigValidator**

Add at the end of gating.rs:
```rust
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Invalid threshold for {category}.{field}: {value} (must be 0.0-1.0)")]
    InvalidThreshold {
        category: String,
        field: String,
        value: f64,
    },

    #[error("Invalid quality threshold: {0} (must be 0.0-1.0)")]
    InvalidQualityThreshold(f64),

    #[error("merge threshold must be <= duplicate threshold for {category}")]
    MergeGreaterThanDuplicate { category: String },
}

pub struct GatingConfigValidator;

impl GatingConfigValidator {
    pub fn validate(config: &GatingConfig) -> Result<(), ConfigError> {
        for (category, thresholds) in &config.dedup_thresholds {
            if thresholds.duplicate > 1.0 || thresholds.duplicate < 0.0 {
                return Err(ConfigError::InvalidThreshold {
                    category: format!("{:?}", category),
                    field: "duplicate".to_string(),
                    value: thresholds.duplicate,
                });
            }

            if thresholds.merge > thresholds.duplicate {
                return Err(ConfigError::MergeGreaterThanDuplicate {
                    category: format!("{:?}", category),
                });
            }

            if thresholds.merge < 0.0 || thresholds.merge > 1.0 {
                return Err(ConfigError::InvalidThreshold {
                    category: format!("{:?}", category),
                    field: "merge".to_string(),
                    value: thresholds.merge,
                });
            }
        }

        if config.auto_approve_quality_threshold > 1.0
            || config.auto_approve_quality_threshold < 0.0 {
            return Err(ConfigError::InvalidQualityThreshold(config.auto_approve_quality_threshold));
        }

        Ok(())
    }
}
```

- [ ] **Step 2: Add thiserror to Cargo.toml dependencies**

Check if thiserror is already a dependency, if not add:
```toml
thiserror = "1.0"
```

- [ ] **Step 3: Run cargo check**

Run: `cargo check -p torque-harness`
Expected: No errors

- [ ] **Step 4: Commit**

```bash
git add crates/torque-harness/src/models/v1/gating.rs crates/torque-harness/Cargo.toml
git commit -m "feat(gating): add configuration validation

- Add ConfigError enum for validation errors
- Add GatingConfigValidator with threshold and quality checks
- Validates merge <= duplicate for all categories"
```

---

## Task 9: Final Verification

- [ ] **Step 1: Run full test suite**

Run: `cargo test -p torque-harness 2>&1 | tail -50`
Expected: All tests pass

- [ ] **Step 2: Run cargo check for warnings**

Run: `cargo check -p torque-harness 2>&1 | grep -E "warning|error"`
Expected: Only existing warnings (not related to our changes)

- [ ] **Step 3: Update STATUS.md**

Add new section for P1: Deduplication

- [ ] **Step 4: Final commit**

```bash
git add STATUS.md
git commit -m "docs: mark P1 Deduplication complete

- Dynamic thresholds per MemoryCategory
- Equivalence checking integrated into gating flow
- Four merge strategies: Summarize, Append, KeepSeparate, WithProvenance
- LLM retry with fallback for equivalence checking
- Configuration validation for threshold consistency"
```

---

## Summary of Changes

### Modified Files
| File | Changes |
|------|---------|
| `src/models/v1/gating.rs` | Extended GatingConfig, DedupThresholds, added ConflictResult, ConfigError, Validator |
| `src/models/v1/memory.rs` | Added `to_env_suffix()` to MemoryCategory |
| `src/config/memory.rs` | Dynamic threshold loading with env override |
| `src/service/gating.rs` | LLM field, equivalence integration, gate_candidate rewrite |
| `Cargo.toml` | Added thiserror dependency |

### New Files
| File | Purpose |
|------|---------|
| `src/service/merge_strategy.rs` | Four merge strategy implementations |
| `tests/dedup_thresholds_tests.rs` | DedupThresholds unit tests |
| `tests/merge_strategy_tests.rs` | Merge strategy unit tests |

### New Types
- `ConflictResult`, `ConflictType` - conflict detection
- `ConfigError` - configuration validation errors
- `GatingConfigValidator` - threshold validation
- `ProvenanceEntry`, `MergedMemoryEntry` - merge result tracking
- `MergeStrategyHandler` trait + implementations

### Environment Variables
- `MEMORY_DEDUP_AGENT_PROFILE_DUPLICATE=0.95`
- `MEMORY_DEDUP_USER_PREFERENCE_MERGE=0.90`
- etc. (per category)
