use anyhow::Result;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Serialize;
use std::time::Duration;
use uuid::Uuid;

pub struct WebhookManager {
    http_client: Client,
    max_retries: u32,
    base_delay: Duration,
}

impl WebhookManager {
    pub fn new(max_retries: u32, base_delay: Duration) -> Self {
        Self {
            http_client: Client::new(),
            max_retries,
            base_delay,
        }
    }

    pub fn attempts(&self) -> u32 {
        self.max_retries + 1
    }

    pub async fn send_with_retry(&self, url: &str, payload: &WebhookPayload) -> Result<()> {
        let mut attempts = 0;
        let mut delay = self.base_delay;

        loop {
            attempts += 1;
            if attempts > 1 {
                tracing::debug!("Webhook retry attempt {} for URL: {}", attempts - 1, url);
            }

            match self.send(url, payload).await {
                Ok(_) => return Ok(()),
                Err(e) if attempts > self.max_retries => return Err(e),
                Err(_) => {
                    tokio::time::sleep(delay).await;
                    delay *= 2;
                }
            }
        }
    }

    async fn send(&self, url: &str, payload: &WebhookPayload) -> Result<()> {
        let response = self
            .http_client
            .post(url)
            .json(payload)
            .send()
            .await?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!("Webhook failed with status: {}", response.status()))
        }
    }
}

#[derive(Serialize)]
pub struct WebhookPayload {
    pub run_id: Uuid,
    pub status: String,
    pub error: Option<String>,
    pub timestamp: DateTime<Utc>,
}

impl WebhookPayload {
    pub fn new(run_id: Uuid, status: &str, error: Option<String>) -> Self {
        Self {
            run_id,
            status: status.to_string(),
            error,
            timestamp: Utc::now(),
        }
    }
}