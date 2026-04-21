use async_stream::stream as async_stream;
use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};
use uuid::Uuid;
use crate::message_bus::stream_bus::{RedisStreamBus, StreamBus};
use crate::models::v1::delegation_event::DelegationEvent;

#[async_trait]
pub trait EventListener: Send + Sync {
    async fn subscribe_delegation(
        &self,
        delegation_id: Uuid,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = DelegationEvent> + Send>>>;
    async fn subscribe_team(
        &self,
        team_id: Uuid,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = DelegationEvent> + Send>>>;
    async fn subscribe_member(
        &self,
        member_id: Uuid,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = DelegationEvent> + Send>>>;
}

pub struct RedisStreamEventListener {
    stream_bus: Arc<RedisStreamBus>,
    consumer_id: String,
}

impl RedisStreamEventListener {
    pub async fn new(redis_url: &str) -> anyhow::Result<Self> {
        let client = redis::Client::open(redis_url)?;
        let conn = redis::aio::ConnectionManager::new(client).await?;
        let stream_bus = Arc::new(RedisStreamBus::new(conn));
        Ok(Self {
            stream_bus,
            consumer_id: format!("consumer-{}", Uuid::new_v4()),
        })
    }
}

fn parse_delegation_event(data: &serde_json::Value, delegation_id: Uuid) -> Option<DelegationEvent> {
    let type_field = data.get("type")?.as_str()?;
    let event_data = data.get("data")?;

    match type_field {
        "created" => {
            Some(DelegationEvent::Created {
                delegation_id,
                task_id: event_data.get("task_id")?.as_str()?.parse().ok()?,
                member_id: event_data.get("member_id")?.as_str()?.parse().ok()?,
                created_at: chrono::Utc::now(),
            })
        }
        "accepted" => {
            Some(DelegationEvent::Accepted {
                delegation_id,
                member_id: event_data.get("member_id")?.as_str()?.parse().ok()?,
                accepted_at: chrono::Utc::now(),
            })
        }
        "completed" => {
            Some(DelegationEvent::Completed {
                delegation_id,
                member_id: event_data.get("member_id")?.as_str()?.parse().ok()?,
                artifact_id: event_data.get("artifact_id")?.as_str()?.parse().ok()?,
                completed_at: chrono::Utc::now(),
            })
        }
        "failed" => {
            Some(DelegationEvent::Failed {
                delegation_id,
                member_id: event_data.get("member_id")?.as_str()?.parse().ok()?,
                error: event_data.get("error")?.as_str()?.to_string(),
                failed_at: chrono::Utc::now(),
            })
        }
        "timeout_partial" => {
            let completeness = event_data
                .get("completeness")?
                .as_f64()? as f32;
            let correctness_confidence = event_data
                .get("correctness_confidence")?
                .as_f64()? as f32;
            let usable_as_is = event_data
                .get("usable_as_is")?
                .as_bool()?;
            let requires_repair: Vec<String> = event_data
                .get("requires_repair")?
                .as_array()?
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            let estimated_remaining_work = event_data
                .get("estimated_remaining_work")?
                .as_str()
                .map(String::from);

            Some(DelegationEvent::TimeoutPartial {
                delegation_id,
                member_id: event_data.get("member_id")?.as_str()?.parse().ok()?,
                partial_quality: crate::models::v1::PartialQuality {
                    completeness,
                    correctness_confidence,
                    usable_as_is,
                    requires_repair,
                    estimated_remaining_work,
                },
                timed_out_at: chrono::Utc::now(),
            })
        }
        _ => None,
    }
}

#[async_trait]
impl EventListener for RedisStreamEventListener {
    async fn subscribe_delegation(
        &self,
        delegation_id: Uuid,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = DelegationEvent> + Send>>> {
        let stream_key = format!("delegation:{}:status", delegation_id);
        let bus = self.stream_bus.clone();
        let consumer_id = self.consumer_id.clone();
        let dg_id = delegation_id;

        let _ = bus.create_consumer_group(&stream_key, "delegation-group", "0").await;

        let stream = async_stream! {
            let mut last_id = "0".to_string();

            loop {
                match timeout(
                    Duration::from_secs(1),
                    bus.xreadgroup(
                        "delegation-group",
                        &consumer_id,
                        &[(stream_key.as_str(), &last_id)],
                        10,
                    )
                ).await {
                    Ok(Ok(results)) => {
                        for result in results {
                            if let Some(event) = parse_delegation_event(&result.data, dg_id) {
                                yield event;
                            }
                            last_id = result.id.clone();
                            let _ = bus.xack(&stream_key, "delegation-group", &[&result.id]).await;
                        }
                    }
                    Ok(Err(e)) => {
                        tracing::warn!("Redis read error: {}", e);
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                    Err(_) => {
                        // Timeout - continue polling
                    }
                }
            }
        };

        Ok(Box::pin(stream))
    }

    async fn subscribe_team(
        &self,
        team_id: Uuid,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = DelegationEvent> + Send>>> {
        let stream_key = format!("team:{}:tasks:shared", team_id);
        let bus = self.stream_bus.clone();
        let consumer_id = self.consumer_id.clone();
        let team_id_val = team_id;

        let _ = bus.create_consumer_group(&stream_key, &format!("team-{}", team_id), "0").await;

        let stream = async_stream! {
            let mut last_id = "0".to_string();

            loop {
                match timeout(
                    Duration::from_secs(1),
                    bus.xreadgroup(
                        &format!("team-{}", team_id),
                        &consumer_id,
                        &[(stream_key.as_str(), &last_id)],
                        10,
                    )
                ).await {
                    Ok(Ok(results)) => {
                        for result in results {
                            // Parse as delegation event or create generic event
                            if let Some(event) = parse_delegation_event(&result.data, team_id_val) {
                                yield event;
                            }
                            last_id = result.id.clone();
                            let _ = bus.xack(&stream_key, &format!("team-{}", team_id), &[&result.id]).await;
                        }
                    }
                    Ok(Err(e)) => {
                        tracing::warn!("Redis read error: {}", e);
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                    Err(_) => {
                        // Timeout - continue polling
                    }
                }
            }
        };

        Ok(Box::pin(stream))
    }

    async fn subscribe_member(
        &self,
        member_id: Uuid,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = DelegationEvent> + Send>>> {
        let stream_key = format!("member:{}:tasks", member_id);
        let bus = self.stream_bus.clone();
        let consumer_id = self.consumer_id.clone();
        let member_id_val = member_id;

        let _ = bus.create_consumer_group(&stream_key, &format!("member-{}", member_id), "0").await;

        let stream = async_stream! {
            let mut last_id = "0".to_string();

            loop {
                match timeout(
                    Duration::from_secs(1),
                    bus.xreadgroup(
                        &format!("member-{}", member_id),
                        &consumer_id,
                        &[(stream_key.as_str(), &last_id)],
                        10,
                    )
                ).await {
                    Ok(Ok(results)) => {
                        for result in results {
                            if let Some(event) = parse_delegation_event(&result.data, member_id_val) {
                                yield event;
                            }
                            last_id = result.id.clone();
                            let _ = bus.xack(&stream_key, &format!("member-{}", member_id), &[&result.id]).await;
                        }
                    }
                    Ok(Err(e)) => {
                        tracing::warn!("Redis read error: {}", e);
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                    Err(_) => {
                        // Timeout - continue polling
                    }
                }
            }
        };

        Ok(Box::pin(stream))
    }
}