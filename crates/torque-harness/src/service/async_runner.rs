use crate::models::v1::run::RunStatus;
use crate::repository::RunRepository;
use reqwest::Client;
use std::sync::Arc;
use uuid::Uuid;

pub struct AsyncRunner {
    run_repo: Arc<dyn RunRepository>,
    http_client: Client,
}

impl AsyncRunner {
    pub fn new(run_repo: Arc<dyn RunRepository>) -> Self {
        Self {
            run_repo,
            http_client: Client::new(),
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
                if let Err(e) = self.send_webhook(webhook_url, run_id, &result).await {
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

    async fn send_webhook(
        &self,
        url: &str,
        run_id: Uuid,
        result: &anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        let payload = serde_json::json!({
            "run_id": run_id.to_string(),
            "status": if result.is_ok() { "completed" } else { "failed" },
            "error": result.as_ref().err().map(|e| e.to_string()),
        });

        self.http_client
            .post(url)
            .json(&payload)
            .send()
            .await?;

        Ok(())
    }
}