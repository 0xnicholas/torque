use crate::config;
use crate::models::v1::gating::CandidateGenerationConfig;
use crate::repository::MemoryRepositoryV1;
use crate::service::candidate_generator::CandidateGenerator;
use std::sync::Arc;
use uuid::Uuid;

pub struct MemoryCompactionJob {
    memory_repo: Arc<dyn MemoryRepositoryV1>,
    candidate_generator: Arc<dyn CandidateGenerator>,
    batch_size: i64,
}

impl MemoryCompactionJob {
    pub fn new(
        memory_repo: Arc<dyn MemoryRepositoryV1>,
        candidate_generator: Arc<dyn CandidateGenerator>,
    ) -> Self {
        Self {
            memory_repo,
            candidate_generator,
            batch_size: 10,
        }
    }

    pub fn with_batch_size(mut self, batch_size: i64) -> Self {
        self.batch_size = batch_size;
        self
    }

    pub async fn run(&self) -> anyhow::Result<CompactionResult> {
        let recent_entries = self
            .memory_repo
            .list_entries(self.batch_size, 0)
            .await?;

        let entries_count = recent_entries.len();
        let mut candidates_created = 0;
        let mut errors = 0;

        for entry in recent_entries {
            let summary_prompt = format!(
                "Analyze this memory entry and determine if it should be compacted, updated, or kept as-is.\n\nEntry Category: {:?}\nKey: {}\nValue: {}",
                entry.category, entry.key, entry.value
            );

            let exec_summary = crate::models::v1::gating::ExecutionSummary {
                task_id: Uuid::nil(),
                agent_instance_id: entry.agent_instance_id.unwrap_or(Uuid::nil()),
                goal: format!("Compaction review for memory entry {}", entry.id),
                output_summary: summary_prompt,
                tool_calls: vec![],
                duration_ms: None,
            };

            let candidate_config = config::candidate_generation_config();

            match self
                .candidate_generator
                .generate_candidates(&exec_summary, &candidate_config)
                .await
            {
                Ok(candidates) => {
                    for candidate in candidates {
                        if self.memory_repo.create_candidate(&candidate).await.is_ok() {
                            candidates_created += 1;
                        } else {
                            errors += 1;
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to generate compaction candidate for entry {}: {}",
                        entry.id,
                        e
                    );
                    errors += 1;
                }
            }
        }

        Ok(CompactionResult {
            entries_processed: entries_count,
            candidates_created,
            errors,
        })
    }
}

#[derive(Debug, Default)]
pub struct CompactionResult {
    pub entries_processed: usize,
    pub candidates_created: usize,
    pub errors: usize,
}