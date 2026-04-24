use crate::models::v1::run::RunStatus;
use crate::repository::RunRepository;
use crate::service::webhook_manager::{WebhookManager, WebhookPayload};
use chrono::Utc;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

pub struct AsyncRunner {
    run_repo: Arc<dyn RunRepository>,
    webhook_manager: WebhookManager,
}

impl AsyncRunner {
    pub fn new(run_repo: Arc<dyn RunRepository>) -> Self {
        Self {
            run_repo,
            webhook_manager: WebhookManager::new(3, Duration::from_secs(1)),
        }
    }

    pub async fn process_run(&self, run_id: Uuid) -> anyhow::Result<()> {
        self.run_repo
            .update_status(run_id, RunStatus::Running)
            .await?;

        let result = self.execute_run(run_id).await;

        let final_status = match &result {
            Ok(_) => RunStatus::Completed,
            Err(_) => RunStatus::Failed,
        };
        self.run_repo.update_status(run_id, final_status).await?;

        if let Some(run) = self.run_repo.get(run_id).await? {
            if let Some(webhook_url) = &run.webhook_url {
                let status = if result.is_ok() { "completed" } else { "failed" };
                let error = result.as_ref().err().map(|e| e.to_string());
                let payload = WebhookPayload::new(run_id, status, error);

                let webhook_result = self.webhook_manager.send_with_retry(webhook_url, &payload).await;
                let webhook_attempts = self.webhook_manager.attempts();

                self.run_repo
                    .update_webhook_status(run_id, Utc::now(), webhook_attempts as i32)
                    .await?;

                if let Err(e) = webhook_result {
                    tracing::warn!("Failed to send webhook for run {}: {}", run_id, e);
                }
            }
        }

        Ok(())
    }

    async fn execute_run(&self, run_id: Uuid) -> anyhow::Result<()> {
        let run = self
            .run_repo
            .get(run_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Run not found: {}", run_id))?;

        tracing::info!(
            "Executing async run {} (async_execution: {})",
            run_id,
            run.async_execution
        );

        Ok(())
    }
}