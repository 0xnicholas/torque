use crate::models::v1::memory::{CompactionRecommendation, CompactionStrategy, MemoryCategory, MemoryEntry, MemoryWriteCandidate, MemoryWriteCandidateStatus};
use crate::repository::MemoryRepositoryV1;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

pub struct MemoryCompactionJob {
    memory_repo: Arc<dyn MemoryRepositoryV1>,
    batch_size: i64,
    max_age_days: i64,
}

impl MemoryCompactionJob {
    pub fn new(
        memory_repo: Arc<dyn MemoryRepositoryV1>,
    ) -> Self {
        Self {
            memory_repo,
            batch_size: 10,
            max_age_days: 30,
        }
    }

    pub fn with_batch_size(mut self, batch_size: i64) -> Self {
        self.batch_size = batch_size;
        self
    }

    pub fn with_max_age_days(mut self, days: i64) -> Self {
        self.max_age_days = days;
        self
    }

    pub async fn run(&self) -> anyhow::Result<CompactionResult> {
        let cutoff_date = Utc::now() - chrono::Duration::days(self.max_age_days);
        let entries = self.get_entries_by_age_and_category(cutoff_date).await?;
        let grouped = self.group_entries(entries);

        let mut recommendations = Vec::new();
        for (category, group_entries) in grouped {
            let recommendation = self.evaluate_group_for_compaction(&category, group_entries)?;
            recommendations.push(recommendation);
        }

        let mut candidates_created = 0;
        let mut entries_processed = 0;
        let mut errors = 0;

        for rec in recommendations {
            entries_processed += 1;
            match self.create_compaction_candidate(&rec).await {
                Ok(_) => candidates_created += 1,
                Err(e) => {
                    tracing::warn!("Failed to create compaction candidate: {}", e);
                    errors += 1;
                }
            }
        }

        Ok(CompactionResult {
            entries_processed,
            candidates_created,
            errors,
        })
    }

    async fn get_entries_by_age_and_category(
        &self,
        cutoff_date: DateTime<Utc>,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        let all_entries = self.memory_repo.list_entries(1000, 0).await?;
        Ok(all_entries
            .into_iter()
            .filter(|e| e.created_at < cutoff_date)
            .collect())
    }

    fn group_entries(&self, entries: Vec<MemoryEntry>) -> HashMap<MemoryCategory, Vec<MemoryEntry>> {
        let mut groups: HashMap<MemoryCategory, Vec<MemoryEntry>> = HashMap::new();
        for entry in entries {
            groups.entry(entry.category.clone()).or_default().push(entry);
        }
        groups
    }

    fn evaluate_group_for_compaction(
        &self,
        category: &MemoryCategory,
        entries: Vec<MemoryEntry>,
    ) -> anyhow::Result<CompactionRecommendation> {
        let total_entries = entries.len();
        let oldest = entries.iter().min_by_key(|e| e.created_at);
        let newest = entries.iter().max_by_key(|e| e.created_at);

        let strategy = if total_entries > 10 {
            CompactionStrategy::Summarize
        } else if total_entries > 3 {
            CompactionStrategy::Merge
        } else {
            CompactionStrategy::Archive
        };

        let reason = format!(
            "Category {:?} has {} entries (oldest: {:?}, newest: {:?})",
            category,
            total_entries,
            oldest.map(|e| e.created_at),
            newest.map(|e| e.created_at)
        );

        let entry_id = entries.first().map(|e| e.id).unwrap_or_else(Uuid::new_v4);
        let supersedes = if entries.len() > 1 {
            entries.get(1).map(|e| e.id)
        } else {
            None
        };

        let entry_ids: Vec<Uuid> = entries.iter().map(|e| e.id).collect();

        Ok(CompactionRecommendation {
            entry_id,
            entry_ids,
            strategy,
            reason,
            supersedes,
        })
    }

    async fn create_compaction_candidate(
        &self,
        recommendation: &CompactionRecommendation,
    ) -> anyhow::Result<()> {
        let entries = self
            .memory_repo
            .get_entries_by_ids(vec![recommendation.entry_id])
            .await?;

        let entry = entries.first().ok_or_else(|| anyhow::anyhow!("Entry not found"))?;

        let strategy_str = match recommendation.strategy {
            CompactionStrategy::Summarize => "summarize",
            CompactionStrategy::Merge => "merge",
            CompactionStrategy::Archive => "archive",
            CompactionStrategy::Drop => "drop",
        };

        let candidate = MemoryWriteCandidate {
            id: Uuid::new_v4(),
            agent_instance_id: entry.agent_instance_id.unwrap_or(Uuid::nil()),
            team_instance_id: entry.team_instance_id,
            content: serde_json::json!({
                "category": entry.category.to_env_suffix(),
                "key": format!("compaction_{}", strategy_str),
                "value": serde_json::json!({
                    "action": strategy_str,
                    "reason": recommendation.reason,
                    "supersedes": recommendation.supersedes,
                    "original_entry_id": recommendation.entry_id,
                }),
            }),
            reasoning: Some(recommendation.reason.clone()),
            status: MemoryWriteCandidateStatus::Pending,
            memory_entry_id: None,
            reviewed_by: None,
            created_at: chrono::Utc::now(),
            reviewed_at: None,
            updated_at: chrono::Utc::now(),
        };

        self.memory_repo.create_candidate(&candidate).await?;

        if recommendation.strategy == CompactionStrategy::Summarize {
            self.summarize_entries(recommendation.entry_ids.clone()).await?;
        }

        Ok(())
    }

    async fn summarize_entries(&self, entry_ids: Vec<Uuid>) -> anyhow::Result<MemoryEntry> {
        let entries = self.memory_repo.get_entries_by_ids(entry_ids).await?;
        if entries.is_empty() {
            anyhow::bail!("No entries found");
        }

        let summary_text = entries
            .iter()
            .map(|e| format!("[{:?}] {}: {}", e.category, e.key, e.value))
            .collect::<Vec<_>>()
            .join("\n---\n");

        let summarized = MemoryEntry {
            id: Uuid::new_v4(),
            key: format!("_compacted_{}", Uuid::new_v4()),
            value: serde_json::json!(summary_text),
            category: MemoryCategory::Session,
            agent_instance_id: entries.first().and_then(|e| e.agent_instance_id),
            team_instance_id: entries.first().and_then(|e| e.team_instance_id),
            source_candidate_id: None,
            superseded_by: None,
            embedding_model: None,
            access_count: 0,
            last_accessed_at: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        self.memory_repo.create_entry(&summarized).await
    }
}

#[derive(Debug, Default)]
pub struct CompactionResult {
    pub entries_processed: usize,
    pub candidates_created: usize,
    pub errors: usize,
}
