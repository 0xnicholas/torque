use crate::config;
use crate::embedding::EmbeddingGenerator;
use crate::models::v1::memory::MemoryCategory;
use crate::repository::ephemeral_log::{EphemeralLog, EphemeralLogCreate};
use crate::repository::rule::RuleRepository;
use crate::vector_type::Vector;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtaskResult {
    pub task_id: Uuid,
    pub plan_id: Uuid,
    pub input: Option<String>,
    pub output: Option<String>,
    pub duration_ms: Option<i32>,
    pub status: String,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReflectionResult {
    pub root_cause: String,
    pub lessons_learned: Vec<String>,
    pub suggested_fix: Option<String>,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperienceQuery {
    pub task_description: String,
    pub category: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievedExperience {
    pub source: String,
    pub content: String,
    pub relevance_score: f64,
    pub rule_id: Option<Uuid>,
    pub memory_id: Option<Uuid>,
}

pub struct ReflexionService {
    ephemeral_log_repo: Arc<dyn crate::repository::ephemeral_log::EphemeralLogRepository>,
    rule_repo: Arc<dyn RuleRepository>,
    memory_repo: Arc<dyn crate::repository::MemoryRepositoryV1>,
    embedding: Option<Arc<dyn EmbeddingGenerator>>,
    llm: Arc<dyn crate::infra::llm::LlmClient>,
}

impl ReflexionService {
    pub fn new(
        ephemeral_log_repo: Arc<dyn crate::repository::ephemeral_log::EphemeralLogRepository>,
        rule_repo: Arc<dyn RuleRepository>,
        memory_repo: Arc<dyn crate::repository::MemoryRepositoryV1>,
        embedding: Option<Arc<dyn EmbeddingGenerator>>,
        llm: Arc<dyn crate::infra::llm::LlmClient>,
    ) -> Self {
        Self {
            ephemeral_log_repo,
            rule_repo,
            memory_repo,
            embedding,
            llm,
        }
    }

    pub async fn log_subtask(&self, result: SubtaskResult) -> anyhow::Result<EphemeralLog> {
        let log_create = EphemeralLogCreate {
            plan_id: result.plan_id,
            task_id: result.task_id,
            input: result.input,
            output: result.output,
            duration_ms: result.duration_ms,
            status: result.status.clone(),
            error_message: result.error_message,
        };

        let log = self.ephemeral_log_repo.create(&log_create).await?;
        Ok(log)
    }

    pub async fn reflect_on_failure(
        &self,
        failed_task_id: Uuid,
        plan_id: Uuid,
        error_context: &str,
    ) -> anyhow::Result<ReflectionResult> {
        let logs = self.ephemeral_log_repo.list_by_plan(plan_id, 50).await?;

        let relevant_logs: Vec<_> = logs
            .into_iter()
            .filter(|l| l.task_id != failed_task_id)
            .collect();

        let reflection = self
            .perform_llm_reflection(error_context, &relevant_logs)
            .await?;

        if reflection.confidence > 0.7 {
            self.store_reflection_as_rule(&reflection, plan_id).await?;
        }

        Ok(reflection)
    }

    async fn perform_llm_reflection(
        &self,
        error_context: &str,
        context_logs: &[EphemeralLog],
    ) -> anyhow::Result<ReflectionResult> {
        let logs_summary = context_logs
            .iter()
            .map(|l| {
                format!(
                    "- Task {}: {} (status: {}, error: {:?})",
                    l.task_id,
                    l.input.as_deref().unwrap_or("N/A"),
                    l.status,
                    l.error_message
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let system_prompt = r#"You are a reflection agent analyzing task failures. Your job is to identify root causes and lessons learned.

Respond ONLY with valid JSON:
{
    "root_cause": "specific technical or procedural reason for failure",
    "lessons_learned": ["actionable insight 1", "actionable insight 2"],
    "suggested_fix": "concrete code or process change to prevent recurrence",
    "confidence": 0.0-1.0
}

Rules:
- root_cause should be specific and actionable (not "bug" or "error")
- lessons_learned should be generalizable
- suggested_fix should be concrete when possible
- confidence reflects how certain you are about the analysis"#;

        let user_prompt = format!(
            "Analyze this failure:\n\nError: {}\n\nRecent context:\n{}\n\nProvide a detailed reflection.",
            error_context, logs_summary
        );

        let response = self
            .llm
            .chat(llm::ChatRequest {
                model: config::extraction_model(),
                messages: vec![
                    llm::Message::system(system_prompt),
                    llm::Message::user(&user_prompt),
                ],
                temperature: Some(0.3),
                max_tokens: Some(1500),
                tools: None,
                stream: None,
            })
            .await
            .map_err(|e| anyhow::anyhow!("LLM reflection failed: {}", e))?;

        let content = response.message.content.as_ref();

        let parsed: ReflectionResult = serde_json::from_str(content).unwrap_or_else(|_| {
            serde_json::from_str::<serde_json::Value>(content)
                .ok()
                .and_then(|v| serde_json::from_value::<ReflectionResult>(v).ok())
                .unwrap_or_else(|| ReflectionResult {
                    root_cause: format!("Analysis inconclusive: {}", content),
                    lessons_learned: vec![],
                    suggested_fix: None,
                    confidence: 0.0,
                })
        });

        Ok(parsed)
    }

    async fn store_reflection_as_rule(
        &self,
        reflection: &ReflectionResult,
        plan_id: Uuid,
    ) -> anyhow::Result<()> {
        let rule_create = crate::repository::rule::RuleCreate {
            name: format!("reflection-{}", plan_id),
            category: "execution".to_string(),
            pattern: serde_json::json!({
                "root_cause": reflection.root_cause,
                "lessons": reflection.lessons_learned,
            }),
            action: serde_json::json!({
                "suggested_fix": reflection.suggested_fix,
            }),
            priority: Some(5),
            embedding: None,
        };

        if let Some(ref embedding_gen) = self.embedding {
            let text = format!(
                "{}: {}",
                reflection.root_cause,
                reflection.lessons_learned.join(" ")
            );
            if let Ok(embedding) = embedding_gen.generate(&text).await {
                let rule_create = crate::repository::rule::RuleCreate {
                    name: rule_create.name,
                    category: rule_create.category,
                    pattern: rule_create.pattern,
                    action: rule_create.action,
                    priority: rule_create.priority,
                    embedding: Some(Vector(embedding)),
                };
                let _ = self.rule_repo.create(&rule_create).await;
                return Ok(());
            }
        }

        let _ = self.rule_repo.create(&rule_create).await;
        Ok(())
    }

    pub async fn retrieve_experience(
        &self,
        query: ExperienceQuery,
    ) -> anyhow::Result<Vec<RetrievedExperience>> {
        let mut experiences = Vec::new();

        if let Some(ref embedding_gen) = self.embedding {
            if let Ok(query_embedding) = embedding_gen.generate(&query.task_description).await {
                let vector = Vector(query_embedding);

                let similar_rules = self
                    .rule_repo
                    .search_similar(&vector, query.category.as_deref(), query.limit as i64)
                    .await?;

                for rule in similar_rules {
                    experiences.push(RetrievedExperience {
                        source: "rule".to_string(),
                        content: format!("{}: {}", rule.pattern, rule.action),
                        relevance_score: 1.0 - rule.confidence_score,
                        rule_id: Some(rule.id),
                        memory_id: None,
                    });
                }

                let similar_memories = self
                    .memory_repo
                    .find_similar_entries(
                        &vector,
                        Some(&MemoryCategory::EpisodicMemory),
                        query.limit as i64,
                    )
                    .await?;

                for memory in similar_memories {
                    experiences.push(RetrievedExperience {
                        source: "memory".to_string(),
                        content: format!("{}: {}", memory.key, memory.value),
                        relevance_score: memory.similarity,
                        rule_id: None,
                        memory_id: Some(memory.entry_id),
                    });
                }
            }
        }

        let rules = self
            .rule_repo
            .query(&crate::repository::rule::RuleQuery {
                category: query.category.clone(),
                pattern_match: None,
                min_confidence: Some(0.5),
                limit: query.limit as i64,
            })
            .await?;

        for rule in rules {
            if !experiences.iter().any(|e| e.rule_id == Some(rule.id)) {
                experiences.push(RetrievedExperience {
                    source: "rule".to_string(),
                    content: format!("{}: {}", rule.pattern, rule.action),
                    relevance_score: rule.confidence_score,
                    rule_id: Some(rule.id),
                    memory_id: None,
                });
            }
        }

        experiences.sort_by(|a, b| {
            b.relevance_score
                .partial_cmp(&a.relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        experiences.truncate(query.limit);

        Ok(experiences)
    }

    pub async fn record_success(&self, rule_id: Uuid) -> anyhow::Result<()> {
        let _ = self.rule_repo.update_stats(rule_id, true).await?;
        Ok(())
    }

    pub async fn record_failure(&self, rule_id: Uuid) -> anyhow::Result<()> {
        let _ = self.rule_repo.update_stats(rule_id, false).await?;
        Ok(())
    }
}
