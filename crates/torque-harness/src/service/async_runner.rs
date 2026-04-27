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
        let run = self
            .run_repo
            .get(run_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Run not found: {}", run_id))?;

        self.run_repo
            .update_status(run_id, RunStatus::Running)
            .await?;

        // TODO: Wire RuntimeFactory execution when Run schema includes
        // agent_definition_id and stores the original goal.
        // For now, runs created with async_execution=true complete
        // without executing — the caller handles execution separately.
        tracing::info!(
            "Async run {} accepted (async_execution={}, instruction='{}')",
            run_id,
            run.async_execution,
            run.instruction,
        );

        self.run_repo
            .update_status(run_id, RunStatus::Completed)
            .await?;

        if let Some(webhook_url) = &run.webhook_url {
            let payload = WebhookPayload::new(run_id, "completed", None);
            let webhook_result = self
                .webhook_manager
                .send_with_retry(webhook_url, &payload)
                .await;
            let webhook_attempts = self.webhook_manager.attempts();

            self.run_repo
                .update_webhook_status(run_id, Utc::now(), webhook_attempts as i32)
                .await?;

            if let Err(e) = webhook_result {
                tracing::warn!("Failed to send webhook for run {}: {}", run_id, e);
            }
        }

        Ok(())
    }
}
