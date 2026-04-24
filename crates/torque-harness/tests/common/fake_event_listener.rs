use async_stream::stream as async_stream;
use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;
use torque_harness::models::v1::delegation_event::DelegationEvent;
use torque_harness::service::team::event_listener::EventListener;
use uuid::Uuid;

pub struct MockEventListener {
    delegation_events: Arc<TokioMutex<Vec<DelegationEvent>>>,
}

impl MockEventListener {
    pub fn new() -> Self {
        Self {
            delegation_events: Arc::new(TokioMutex::new(Vec::new())),
        }
    }

    pub fn with_delegation_events(events: Vec<DelegationEvent>) -> Self {
        Self {
            delegation_events: Arc::new(TokioMutex::new(events)),
        }
    }

    pub async fn push_delegation_event(&self, event: DelegationEvent) {
        let mut guard = self.delegation_events.lock().await;
        guard.push(event);
    }

    pub async fn get_delegation_events(&self) -> Vec<DelegationEvent> {
        self.delegation_events
            .lock()
            .await
            .clone()
    }
}

impl Default for MockEventListener {
    fn default() -> Self {
        Self::new()
    }
}

fn get_delegation_id_from_event(event: &DelegationEvent) -> Option<Uuid> {
    match event {
        DelegationEvent::Created { delegation_id, .. } => Some(*delegation_id),
        DelegationEvent::Accepted { delegation_id, .. } => Some(*delegation_id),
        DelegationEvent::Rejected { delegation_id, .. } => Some(*delegation_id),
        DelegationEvent::Completed { delegation_id, .. } => Some(*delegation_id),
        DelegationEvent::Failed { delegation_id, .. } => Some(*delegation_id),
        DelegationEvent::TimeoutPartial { delegation_id, .. } => Some(*delegation_id),
        DelegationEvent::ExtensionRequested { delegation_id, .. } => Some(*delegation_id),
        DelegationEvent::ExtensionGranted { .. } => None,
    }
}

#[async_trait]
impl EventListener for MockEventListener {
    async fn subscribe_delegation(
        &self,
        delegation_id: Uuid,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = DelegationEvent> + Send>>> {
        let events = self.delegation_events.clone();
        let stream = async_stream! {
            let events_clone = events.clone();
            let mut guard = events_clone.lock().await;
            let events_vec: Vec<DelegationEvent> = guard.drain(0..).collect();
            for event in events_vec {
                if get_delegation_id_from_event(&event) == Some(delegation_id) {
                    yield event;
                }
            }
        };
        Ok(Box::pin(stream))
    }

    async fn subscribe_team(
        &self,
        _team_id: Uuid,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = DelegationEvent> + Send>>> {
        let events = self.delegation_events.clone();
        let stream = async_stream! {
            let events_clone = events.clone();
            let mut guard = events_clone.lock().await;
            let events_vec: Vec<DelegationEvent> = guard.drain(0..).collect();
            for event in events_vec {
                yield event;
            }
        };
        Ok(Box::pin(stream))
    }

    async fn subscribe_member(
        &self,
        _member_id: Uuid,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = DelegationEvent> + Send>>> {
        let events = self.delegation_events.clone();
        let stream = async_stream! {
            let events_clone = events.clone();
            let mut guard = events_clone.lock().await;
            let events_vec: Vec<DelegationEvent> = guard.drain(0..).collect();
            for event in events_vec {
                yield event;
            }
        };
        Ok(Box::pin(stream))
    }
}