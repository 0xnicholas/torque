use crate::models::v1::run::{RunRequest, RunStatus};
use crate::repository::RunRepository;
use crate::service::webhook_manager::{WebhookManager, WebhookPayload};
use crate::service::RunService;
use chrono::Utc;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use uuid::Uuid;

pub struct AsyncRunner {
    run_repo: Arc<dyn RunRepository>,
    run_service: Arc<RunService>,
    webhook_manager: WebhookManager,
}

impl AsyncRunner {
    pub fn new(run_repo: Arc<dyn RunRepository>, run_service: Arc<RunService>) -> Self {
        Self {
            run_repo,
            run_service,
            webhook_manager: WebhookManager::new(3, Duration::from_secs(1)),
        }
    }

    pub async fn process_run(&self, run_id: Uuid) -> anyhow::Result<()> {
        let run = self
            .run_repo
            .get(run_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Run not found: {}", run_id))?;

        let started_at = Utc::now();
        self.run_repo
            .update_result(run_id, RunStatus::Running, Some(started_at), None, None)
            .await?;

        let request = serde_json::from_value::<RunRequest>(run.request_payload.clone())?;

        let (tx, _rx) = mpsc::channel::<crate::agent::stream::StreamEvent>(1);
        let result = self
            .run_service
            .execute(run.agent_instance_id, request, tx)
            .await;

        match result {
            Ok(()) => {
                let completed_at = Utc::now();
                self.run_repo
                    .update_result(
                        run_id,
                        RunStatus::Completed,
                        None,
                        Some(completed_at),
                        None,
                    )
                    .await?;

                if let Some(webhook_url) = &run.webhook_url {
                    let payload = WebhookPayload::new(run_id, "completed", None);
                    self.send_webhook(run_id, webhook_url, &payload).await;
                }
            }
            Err(e) => {
                let error_msg = e.to_string();
                self.run_repo
                    .update_result(
                        run_id,
                        RunStatus::Failed,
                        None,
                        Some(Utc::now()),
                        Some(error_msg.clone()),
                    )
                    .await?;

                if let Some(webhook_url) = &run.webhook_url {
                    let payload =
                        WebhookPayload::new(run_id, "failed", Some(error_msg));
                    self.send_webhook(run_id, webhook_url, &payload).await;
                }

                return Err(e);
            }
        }

        Ok(())
    }

    async fn send_webhook(&self, run_id: Uuid, webhook_url: &str, payload: &WebhookPayload) {
        let webhook_result = self
            .webhook_manager
            .send_with_retry(webhook_url, payload)
            .await;
        let webhook_attempts = self.webhook_manager.attempts();

        if let Err(e) = self
            .run_repo
            .update_webhook_status(run_id, Utc::now(), webhook_attempts as i32)
            .await
        {
            tracing::warn!("Failed to update webhook status for run {}: {}", run_id, e);
        }

        if let Err(e) = webhook_result {
            tracing::warn!("Failed to send webhook for run {}: {}", run_id, e);
        }
    }
}
