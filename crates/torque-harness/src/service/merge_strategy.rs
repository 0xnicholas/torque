use crate::models::v1::gating::MergeStrategy;
use crate::models::v1::memory::{MemoryContent, MemoryEntry};
use anyhow::Result;
use async_trait::async_trait;
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

use llm::OpenAiClient;
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
        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?
            .text()
            .await?;

        let consolidated: serde_json::Value =
            serde_json::from_str(&response).unwrap_or_else(|_| {
                serde_json::json!({
                    "key": candidate.key.clone(),
                    "value": {
                        "original": existing.value,
                        "new": candidate.value.clone(),
                        "summary": response
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
