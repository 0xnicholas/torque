use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;
use tokio::sync::mpsc;
use uuid::Uuid;
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

pub struct RedisStreamEventListener;

impl RedisStreamEventListener {
    pub async fn new(_redis_url: &str) -> anyhow::Result<Self> {
        Ok(Self)
    }
}

#[async_trait]
impl EventListener for RedisStreamEventListener {
    async fn subscribe_delegation(
        &self,
        _delegation_id: Uuid,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = DelegationEvent> + Send>>> {
        let (_, rx) = mpsc::channel::<DelegationEvent>(100);
        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        Ok(Box::pin(stream))
    }

    async fn subscribe_team(
        &self,
        _team_id: Uuid,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = DelegationEvent> + Send>>> {
        let (_, rx) = mpsc::channel::<DelegationEvent>(100);
        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        Ok(Box::pin(stream))
    }

    async fn subscribe_member(
        &self,
        _member_id: Uuid,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = DelegationEvent> + Send>>> {
        let (_, rx) = mpsc::channel::<DelegationEvent>(100);
        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        Ok(Box::pin(stream))
    }
}