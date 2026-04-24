use crate::config;
use crate::embedding::EmbeddingGenerator;
use anyhow::Context;
use crate::models::v1::gating::{
    CandidateGenerationConfig, ConflictResult, ConflictType, DecisionFactors, DedupAction,
    DedupResult, DedupThresholds, EquivalenceCheckInput, EquivalenceResult, ExecutionSummary,
    GateDecision, GateDecisionType, GatingConfig, MergeStrategy, QualityScore, RiskAssessment,
    RiskLevel, ReviewPriority, SimilarMemoryResult, WriteMode,
};
use crate::models::v1::memory::{
    MemoryCategory, MemoryContent, MemoryWriteCandidate, MemoryWriteCandidateStatus,
};
use crate::repository::MemoryRepositoryV1;
use llm::OpenAiClient;
use std::sync::Arc;
use uuid::Uuid;

pub struct MemoryGatingService {
    repo: Arc<dyn MemoryRepositoryV1>,
    embedding: Option<Arc<dyn EmbeddingGenerator>>,
    llm: Option<Arc<OpenAiClient>>,
    gating_config: GatingConfig,
    candidate_config: CandidateGenerationConfig,
}

impl MemoryGatingService {
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

    pub async fn assess_quality(&self, content: &MemoryContent) -> QualityScore {
        let value_str = match &content.value {
            serde_json::Value::String(s) => s.clone(),
            _ => content.value.to_string(),
        };

        let information_density = self.assess_information_density(&value_str);
        let specificity = self.assess_specificity(&value_str);
        let timelessness = self.assess_timelessness(&value_str);
        let reusability = self.assess_reusability(&value_str);

        QualityScore::calculate(information_density, specificity, timelessness, reusability)
    }

    fn assess_information_density(&self, text: &str) -> f64 {
        let words: Vec<&str> = text.split_whitespace().collect();
        if words.len() < 3 {
            return 0.0;
        }
        let unique_ratio = words.iter().collect::<std::collections::HashSet<_>>().len() as f64
            / words.len() as f64;
        let length_score = (text.len() as f64 / 100.0).clamp(0.0, 1.0);
        (unique_ratio * 0.6 + length_score * 0.4).clamp(0.0, 1.0)
    }

    fn assess_specificity(&self, text: &str) -> f64 {
        let has_specific_values = text.chars().any(|c| c.is_numeric());
        let has_specific_terms = [
            "uuid",
            "id",
            "timestamp",
            "config",
            "setting",
            "value",
            "mode",
            "format",
        ]
        .iter()
        .any(|term| text.to_lowercase().contains(term));
        let specificity_score = if has_specific_values || has_specific_terms {
            0.8
        } else {
            0.5
        };
        let length_score = (text.len() as f64 / 50.0).clamp(0.0, 1.0);
        (specificity_score * 0.7 + length_score * 0.3).clamp(0.0, 1.0)
    }

    fn assess_timelessness(&self, text: &str) -> f64 {
        let temporal_terms = [
            "yesterday",
            "today",
            "tomorrow",
            "recently",
            "last week",
            "currently",
        ]
        .iter()
        .any(|term| text.to_lowercase().contains(term));
        if temporal_terms {
            return 0.4;
        }
        let timeless_terms = [
            "always",
            "never",
            "typically",
            "usually",
            "best practice",
            "convention",
        ]
        .iter()
        .any(|term| text.to_lowercase().contains(term));
        if timeless_terms {
            return 0.9;
        }
        0.7
    }

    fn assess_reusability(&self, text: &str) -> f64 {
        let general_terms = [
            "agent",
            "user",
            "preference",
            "setting",
            "configuration",
            "pattern",
        ]
        .iter()
        .any(|term| text.to_lowercase().contains(term));
        if general_terms {
            return 0.8;
        }
        let specific_terms = ["session", "temporary", "one-time", "ephemeral"]
            .iter()
            .any(|term| text.to_lowercase().contains(term));
        if specific_terms {
            return 0.3;
        }
        0.6
    }

    pub async fn evaluate_risk(&self, content: &MemoryContent) -> RiskAssessment {
        let high_impact_fields = [
            "database_config",
            "api_keys",
            "security_policy",
            "password",
            "secret",
            "credential",
        ];
        let value_str = content.value.to_string().to_lowercase();

        let has_high_impact = high_impact_fields
            .iter()
            .any(|field| value_str.contains(field));

        if has_high_impact {
            return RiskAssessment {
                level: RiskLevel::High,
                consent_required: true,
                review_reason: Some("High-impact field detected".to_string()),
            };
        }

        let medium_impact_fields = ["preference", "setting", "theme", "language", "timezone"];
        let has_medium_impact = medium_impact_fields
            .iter()
            .any(|field| value_str.contains(field));

        if has_medium_impact {
            return RiskAssessment {
                level: RiskLevel::Medium,
                consent_required: false,
                review_reason: Some("Medium-impact preference detected".to_string()),
            };
        }

        let consent_terms = ["personal", "private", "sensitive", "confidential"];
        let needs_consent = consent_terms.iter().any(|term| value_str.contains(term));

        RiskAssessment {
            level: RiskLevel::Low,
            consent_required: needs_consent,
            review_reason: None,
        }
    }

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

    pub async fn check_equivalence_for_candidate(
        &self,
        dedup_result: &DedupResult,
        category: &MemoryCategory,
        candidate_content: &MemoryContent,
    ) -> anyhow::Result<Option<EquivalenceResult>> {
        let thresholds = DedupThresholds::from_config(&self.gating_config, category);
        let similarity = dedup_result.similarity;

        let should_check = match dedup_result.action {
            DedupAction::New => similarity >= thresholds.merge - 0.05,
            DedupAction::Mergeable => true,
            DedupAction::Duplicate => similarity < 0.98,
        };

        if !should_check {
            return Ok(None);
        }

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

    pub async fn resolve_with_rules(
        &self,
        dedup_result: &DedupResult,
        equivalence_result: Option<&EquivalenceResult>,
    ) -> anyhow::Result<GateDecision> {
        let equiv = equivalence_result.cloned().unwrap_or(EquivalenceResult::Distinct);

        match (&dedup_result.action, &equiv) {
            (DedupAction::Duplicate, EquivalenceResult::Distinct) if dedup_result.similarity < 0.98 => {
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
                    target_entry_id: None,
                    priority: None,
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
                    target_entry_id: None,
                    priority: None,
                })
            }
            (DedupAction::Mergeable, EquivalenceResult::Conflict) => {
                Ok(GateDecision {
                    decision: GateDecisionType::Review,
                    priority: Some(ReviewPriority::High),
                    reason: "Mergeable but semantic conflict detected".to_string(),
                    target_entry_id: None,
                    write_mode: None,
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
                    target_entry_id: None,
                    priority: None,
                })
            }
            (DedupAction::New, EquivalenceResult::Distinct) => {
                Ok(GateDecision {
                    decision: GateDecisionType::Approve,
                    write_mode: Some(WriteMode::Insert),
                    reason: "New distinct entry".to_string(),
                    target_entry_id: None,
                    priority: None,
                })
            }
            _ => {
                Ok(GateDecision {
                    decision: GateDecisionType::Review,
                    priority: Some(ReviewPriority::Low),
                    reason: format!("Default review: dedup={:?}, equiv={:?}", dedup_result.action, equiv),
                    target_entry_id: None,
                    write_mode: None,
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
                    target_entry_id: None,
                    priority: None,
                })
            }
            Some(EquivalenceResult::Conflict) => {
                Ok(GateDecision {
                    decision: GateDecisionType::Review,
                    priority: Some(ReviewPriority::High),
                    reason: "LLM detected conflict".to_string(),
                    target_entry_id: None,
                    write_mode: None,
                })
            }
            _ => {
                Ok(GateDecision {
                    decision: GateDecisionType::Review,
                    priority: Some(ReviewPriority::Medium),
                    reason: "Ambiguous dedup result - LLM fallback inconclusive".to_string(),
                    target_entry_id: None,
                    write_mode: None,
                })
            }
        }
    }

    pub async fn check_equivalence(
        &self,
        input: &EquivalenceCheckInput,
    ) -> anyhow::Result<EquivalenceResult> {
        if let Some(time_delta) = input.time_delta_seconds {
            if input.same_task && time_delta < 300 && input.content_similarity > 0.96 {
                return Ok(EquivalenceResult::Equivalent);
            }
        }

        if input.same_task && input.content_similarity > 0.96 {
            return Ok(EquivalenceResult::Equivalent);
        }

        if input.same_agent && input.content_similarity > 0.92 {
            return Ok(EquivalenceResult::Mergeable);
        }

        if input.content_similarity < 0.80 {
            return Ok(EquivalenceResult::Distinct);
        }

        Ok(EquivalenceResult::Mergeable)
    }

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
        tracing::warn!("LLM equivalence check failed after {} retries: {:?}", max_retries, last_error);
        Ok(EquivalenceResult::Distinct)
    }

    pub async fn make_decision(
        &self,
        candidate: &MemoryWriteCandidate,
        quality: &QualityScore,
        risk: &RiskAssessment,
        dedup: &DedupResult,
        candidate_embedding: Option<&[f32]>,
    ) -> anyhow::Result<GateDecision> {
        if candidate.status != MemoryWriteCandidateStatus::Pending {
            return Ok(GateDecision {
                decision: GateDecisionType::Review,
                write_mode: None,
                target_entry_id: None,
                reason: "Candidate not in pending status".to_string(),
                priority: Some(crate::models::v1::gating::ReviewPriority::Low),
            });
        }

        if risk.level == RiskLevel::High || risk.consent_required {
            return Ok(GateDecision {
                decision: GateDecisionType::Review,
                write_mode: None,
                target_entry_id: None,
                reason: risk
                    .review_reason
                    .clone()
                    .unwrap_or_else(|| "High risk or consent required".to_string()),
                priority: Some(crate::models::v1::gating::ReviewPriority::High),
            });
        }

        if dedup.action == DedupAction::Duplicate {
            return Ok(GateDecision {
                decision: GateDecisionType::Reject,
                write_mode: None,
                target_entry_id: None,
                reason: format!(
                    "Duplicate of existing memory (similarity: {:.2})",
                    dedup.similarity
                ),
                priority: None,
            });
        }

        let content: MemoryContent = serde_json::from_value(candidate.content.clone())
            .unwrap_or_else(|_| MemoryContent {
                category: MemoryCategory::TaskOrDomainMemory,
                key: "unknown".to_string(),
                value: candidate.content.clone(),
            });

        if content.category == MemoryCategory::ExternalContextMemory {
            return Ok(GateDecision {
                decision: GateDecisionType::Review,
                write_mode: None,
                target_entry_id: None,
                reason: "External context memory requires review".to_string(),
                priority: Some(crate::models::v1::gating::ReviewPriority::Medium),
            });
        }

        if quality.overall >= self.gating_config.auto_approve_quality_threshold
            && dedup.action != DedupAction::Duplicate
            && dedup.action != DedupAction::Mergeable
        {
            return Ok(GateDecision {
                decision: GateDecisionType::Approve,
                write_mode: Some(WriteMode::Insert),
                target_entry_id: None,
                reason: "Auto-approved: high quality, low risk, no duplicates".to_string(),
                priority: None,
            });
        }

        if dedup.action == DedupAction::Mergeable {
            return Ok(GateDecision {
                decision: GateDecisionType::Merge,
                write_mode: Some(WriteMode::Merge {
                    target_id: dedup.similar_entry_id.unwrap_or(Uuid::nil()),
                    strategy: MergeStrategy::Summarize,
                }),
                target_entry_id: dedup.similar_entry_id,
                reason: format!(
                    "Mergeable with existing memory (similarity: {:.2})",
                    dedup.similarity
                ),
                priority: None,
            });
        }

        let priority = if quality.overall >= 0.75 {
            crate::models::v1::gating::ReviewPriority::Low
        } else if quality.overall >= 0.60 {
            crate::models::v1::gating::ReviewPriority::Medium
        } else {
            crate::models::v1::gating::ReviewPriority::High
        };

        Ok(GateDecision {
            decision: GateDecisionType::Review,
            write_mode: None,
            target_entry_id: None,
            reason: format!(
                "Quality {:.2} below auto-approve threshold {:.2}",
                quality.overall, self.gating_config.auto_approve_quality_threshold
            ),
            priority: Some(priority),
        })
    }

    pub async fn gate_candidate(
        &self,
        candidate: &MemoryWriteCandidate,
    ) -> anyhow::Result<GateDecision> {
        let content: MemoryContent = serde_json::from_value(candidate.content.clone())
            .context("Failed to parse memory content")?;

        let quality = self.assess_quality(&content).await;
        let risk = self.evaluate_risk(&content).await;

        let embedding = match &self.embedding {
            Some(emb) => Some(emb.generate(&Self::content_to_text(&content)).await?),
            None => None,
        };

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

        let equivalence_result = if embedding.is_some() {
            self.check_equivalence_for_candidate(&dedup_result, &content.category, &content).await?
        } else {
            None
        };

        let conflict_result = if let Some(EquivalenceResult::Conflict) = &equivalence_result {
            Some(self.detect_conflict(&content, &dedup_result).await?)
        } else {
            None
        };

        let decision = if conflict_result.as_ref().map(|c| c.has_conflict).unwrap_or(false) {
            Self::make_conflict_decision(conflict_result.unwrap())
        } else if embedding.is_some() {
            self.resolve_with_rules(&dedup_result, equivalence_result.as_ref()).await?
        } else {
            self.make_decision(candidate, &quality, &risk, &dedup_result, embedding.as_deref()).await?
        };

        self.log_decision(candidate, &quality, &risk, &dedup_result, &decision).await?;

        Ok(decision)
    }

    async fn log_decision(
        &self,
        candidate: &MemoryWriteCandidate,
        quality: &QualityScore,
        risk: &RiskAssessment,
        dedup: &DedupResult,
        decision: &GateDecision,
    ) -> anyhow::Result<()> {
        let factors = DecisionFactors {
            quality_score: quality.overall,
            confidence: 0.85,
            similarity_to_existing: Some(dedup.similarity),
            equivalence_result: Some(format!("{:?}", dedup.action)),
            risk_level: format!("{:?}", risk.level),
            has_conflict: dedup.action == DedupAction::Duplicate,
            consent_required: risk.consent_required,
        };

        let processed_by = match decision.decision {
            GateDecisionType::Approve | GateDecisionType::Merge => "auto",
            GateDecisionType::Review => "policy",
            GateDecisionType::Reject => "dedup",
        };

        self.repo
            .log_decision(
                Some(candidate.id),
                decision.target_entry_id,
                format!("{:?}", decision.decision).to_lowercase().as_str(),
                Some(&decision.reason),
                serde_json::to_value(&factors).unwrap_or_default(),
                processed_by,
            )
            .await?;

        Ok(())
    }

    pub fn gating_config(&self) -> &GatingConfig {
        &self.gating_config
    }

    pub fn candidate_config(&self) -> &CandidateGenerationConfig {
        &self.candidate_config
    }
}
