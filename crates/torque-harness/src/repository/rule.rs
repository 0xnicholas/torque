use crate::db::Database;
use crate::vector_type::Vector;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub struct Rule {
    pub id: Uuid,
    pub name: String,
    pub category: String,
    pub pattern: serde_json::Value,
    pub action: serde_json::Value,
    pub priority: i32,
    pub success_count: i32,
    pub failure_count: i32,
    pub confidence_score: f64,
    pub embedding: Option<Vector>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_accessed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RuleCreate {
    pub name: String,
    pub category: String,
    pub pattern: serde_json::Value,
    pub action: serde_json::Value,
    pub priority: Option<i32>,
    pub embedding: Option<Vector>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleQuery {
    pub category: Option<String>,
    pub pattern_match: Option<serde_json::Value>,
    pub min_confidence: Option<f64>,
    pub limit: i64,
}

#[async_trait]
pub trait RuleRepository: Send + Sync {
    async fn create(&self, rule: &RuleCreate) -> anyhow::Result<Rule>;
    async fn get_by_id(&self, id: Uuid) -> anyhow::Result<Option<Rule>>;
    async fn list_by_category(&self, category: &str, limit: i64) -> anyhow::Result<Vec<Rule>>;
    async fn search_similar(
        &self,
        embedding: &Vector,
        category: Option<&str>,
        limit: i64,
    ) -> anyhow::Result<Vec<Rule>>;
    async fn query(&self, query: &RuleQuery) -> anyhow::Result<Vec<Rule>>;
    async fn update_stats(&self, id: Uuid, success: bool) -> anyhow::Result<Option<Rule>>;
    async fn update_priority(&self, id: Uuid, priority: i32) -> anyhow::Result<Option<Rule>>;
    async fn delete(&self, id: Uuid) -> anyhow::Result<bool>;
    async fn increment_access(&self, id: Uuid) -> anyhow::Result<Option<Rule>>;
}

pub struct PostgresRuleRepository {
    db: Database,
}

impl PostgresRuleRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }
}

#[async_trait]
impl RuleRepository for PostgresRuleRepository {
    async fn create(&self, rule: &RuleCreate) -> anyhow::Result<Rule> {
        let row = sqlx::query_as::<_, Rule>(
            r#"
            INSERT INTO rules (name, category, pattern, action, priority, embedding)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING *
            "#,
        )
        .bind(&rule.name)
        .bind(&rule.category)
        .bind(&rule.pattern)
        .bind(&rule.action)
        .bind(rule.priority.unwrap_or(0))
        .bind(&rule.embedding)
        .fetch_one(self.db.pool())
        .await?;
        Ok(row)
    }

    async fn get_by_id(&self, id: Uuid) -> anyhow::Result<Option<Rule>> {
        let row = sqlx::query_as::<_, Rule>("SELECT * FROM rules WHERE id = $1")
            .bind(id)
            .fetch_optional(self.db.pool())
            .await?;
        Ok(row)
    }

    async fn list_by_category(&self, category: &str, limit: i64) -> anyhow::Result<Vec<Rule>> {
        let rows = sqlx::query_as::<_, Rule>(
            "SELECT * FROM rules WHERE category = $1 ORDER BY priority DESC, confidence_score DESC LIMIT $2",
        )
        .bind(category)
        .bind(limit)
        .fetch_all(self.db.pool())
        .await?;
        Ok(rows)
    }

    async fn search_similar(
        &self,
        embedding: &Vector,
        category: Option<&str>,
        limit: i64,
    ) -> anyhow::Result<Vec<Rule>> {
        let rows = if let Some(cat) = category {
            sqlx::query_as::<_, Rule>(
                r#"
                SELECT * FROM rules
                WHERE category = $2 AND embedding IS NOT NULL
                ORDER BY embedding <=> $1
                LIMIT $3
                "#,
            )
            .bind(embedding)
            .bind(cat)
            .bind(limit)
            .fetch_all(self.db.pool())
            .await?
        } else {
            sqlx::query_as::<_, Rule>(
                r#"
                SELECT * FROM rules
                WHERE embedding IS NOT NULL
                ORDER BY embedding <=> $1
                LIMIT $2
                "#,
            )
            .bind(embedding)
            .bind(limit)
            .fetch_all(self.db.pool())
            .await?
        };
        Ok(rows)
    }

    async fn query(&self, query: &RuleQuery) -> anyhow::Result<Vec<Rule>> {
        let min_confidence = query.min_confidence.unwrap_or(0.0);
        let limit = query.limit;

        let rows = if let Some(ref cat) = query.category {
            sqlx::query_as::<_, Rule>(
                r#"
                SELECT * FROM rules
                WHERE category = $1 AND confidence_score >= $2
                ORDER BY priority DESC, confidence_score DESC
                LIMIT $3
                "#,
            )
            .bind(cat)
            .bind(min_confidence)
            .bind(limit)
            .fetch_all(self.db.pool())
            .await?
        } else {
            sqlx::query_as::<_, Rule>(
                r#"
                SELECT * FROM rules
                WHERE confidence_score >= $1
                ORDER BY priority DESC, confidence_score DESC
                LIMIT $2
                "#,
            )
            .bind(min_confidence)
            .bind(limit)
            .fetch_all(self.db.pool())
            .await?
        };
        Ok(rows)
    }

    async fn update_stats(&self, id: Uuid, success: bool) -> anyhow::Result<Option<Rule>> {
        let row = if success {
            sqlx::query_as::<_, Rule>(
                r#"
                UPDATE rules
                SET success_count = success_count + 1,
                    confidence_score = CASE
                        WHEN success_count + failure_count > 0
                        THEN (success_count + 1.0) / (success_count + failure_count + 1.0)
                        ELSE 0.5
                    END,
                    updated_at = NOW()
                WHERE id = $1
                RETURNING *
                "#,
            )
            .bind(id)
            .fetch_optional(self.db.pool())
            .await?
        } else {
            sqlx::query_as::<_, Rule>(
                r#"
                UPDATE rules
                SET failure_count = failure_count + 1,
                    confidence_score = CASE
                        WHEN success_count + failure_count > 0
                        THEN success_count / (success_count + failure_count + 1.0)
                        ELSE 0.5
                    END,
                    updated_at = NOW()
                WHERE id = $1
                RETURNING *
                "#,
            )
            .bind(id)
            .fetch_optional(self.db.pool())
            .await?
        };
        Ok(row)
    }

    async fn update_priority(&self, id: Uuid, priority: i32) -> anyhow::Result<Option<Rule>> {
        let row = sqlx::query_as::<_, Rule>(
            r#"
            UPDATE rules SET priority = $2, updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(priority)
        .fetch_optional(self.db.pool())
        .await?;
        Ok(row)
    }

    async fn delete(&self, id: Uuid) -> anyhow::Result<bool> {
        let result = sqlx::query("DELETE FROM rules WHERE id = $1")
            .bind(id)
            .execute(self.db.pool())
            .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn increment_access(&self, id: Uuid) -> anyhow::Result<Option<Rule>> {
        let row = sqlx::query_as::<_, Rule>(
            r#"
            UPDATE rules
            SET last_accessed_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(id)
        .fetch_optional(self.db.pool())
        .await?;
        Ok(row)
    }
}
