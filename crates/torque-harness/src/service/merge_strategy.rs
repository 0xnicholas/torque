use crate::models::v1::gating::MergeStrategy;
use crate::models::v1::memory::{MemoryContent, MemoryEntry};
use anyhow::Result;
use async_trait::async_trait;
use llm::{ChatRequest, LlmClient};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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
            obj.insert(
                "_provenance".to_string(),
                serde_json::to_value(&provenance)?,
            );
        }

        Ok(MergedMemoryEntry {
            key: existing.key.clone(),
            value: new_value,
            provenance,
        })
    }
}

pub struct SummarizeStrategy {
    llm: Arc<dyn LlmClient>,
    model: String,
}

impl SummarizeStrategy {
    pub fn new(llm: Arc<dyn LlmClient>) -> Self {
        let model = crate::config::extraction_model();
        Self { llm, model }
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

        let request = ChatRequest::new(
            &self.model,
            vec![
                llm::Message::system("You are a memory consolidation assistant."),
                llm::Message::user(&prompt),
            ],
        )
        .with_max_tokens(500)
        .with_temperature(0.3);

        let response = self.llm.chat(request).await?;
        let response_text = response.message.content;

        let consolidated: serde_json::Value =
            serde_json::from_str(&response_text).unwrap_or_else(|_| {
                serde_json::json!({
                    "key": candidate.key.clone(),
                    "value": {
                        "original": existing.value,
                        "new": candidate.value.clone(),
                        "summary": response_text
                    }
                })
            });

        Ok(MergedMemoryEntry {
            key: candidate.key.clone(),
            value: consolidated,
            provenance: vec![
                ProvenanceEntry {
                    source: existing.id.to_string(),
                    method: "original".to_string(),
                    timestamp: None,
                },
                ProvenanceEntry {
                    source: candidate.key.clone(),
                    method: "summarized".to_string(),
                    timestamp: None,
                },
            ],
        })
    }
}

pub struct MergeStrategyExecutor {
    summarize: SummarizeStrategy,
    append: AppendStrategy,
    keep_separate: KeepSeparateStrategy,
    with_provenance: WithProvenanceStrategy,
}

impl MergeStrategyExecutor {
    pub fn new(llm: Arc<dyn LlmClient>) -> Self {
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
