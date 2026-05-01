use std::sync::Arc;

use async_trait::async_trait;

use super::event::BusEvent;

/// Handler for EventBus events.
#[async_trait]
pub trait BusEventHandler: Send + Sync {
    /// Handle a bus event.
    async fn handle(&self, event: &BusEvent);
}

/// A type-erased boxed handler for storage.
pub(crate) type BoxedBusHandler = Arc<dyn BusEventHandler>;

#[cfg(test)]
pub(crate) struct TestBusHandler;

#[cfg(test)]
#[async_trait]
impl BusEventHandler for TestBusHandler {
    async fn handle(&self, _event: &BusEvent) {}
}

#[cfg(test)]
impl Default for TestBusHandler {
    fn default() -> Self {
        Self
    }
}
