use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::id::ExtensionId;

/// An event published on the EventBus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusEvent {
    pub id: Uuid,
    pub topic: super::topic::BusTopic,
    pub source: ExtensionId,
    pub timestamp: DateTime<Utc>,
    pub payload: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bus_event_construction() {
        let topic = crate::bus::BusTopic::from_str("ext:registered");
        let ext_id = crate::id::ExtensionId::new();
        let event = BusEvent {
            id: Uuid::new_v4(),
            topic: topic.clone(),
            source: ext_id,
            timestamp: Utc::now(),
            payload: serde_json::json!({ "key": "value" }),
        };
        assert_eq!(event.topic.as_str(), "ext:registered");
        assert_eq!(event.source, ext_id);
        assert_eq!(event.payload["key"], "value");
    }

    #[test]
    fn test_bus_event_clone() {
        let event = BusEvent {
            id: Uuid::new_v4(),
            topic: crate::bus::BusTopic::from_str("test:clone"),
            source: crate::id::ExtensionId::new(),
            timestamp: Utc::now(),
            payload: serde_json::json!(42),
        };
        let cloned = event.clone();
        assert_eq!(event.id, cloned.id);
        assert_eq!(event.topic, cloned.topic);
        assert_eq!(event.source, cloned.source);
        assert_eq!(event.payload, cloned.payload);
    }

    #[test]
    fn test_bus_event_serde_roundtrip() {
        let event = BusEvent {
            id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            topic: crate::bus::BusTopic::from_str("test:serde"),
            source: crate::id::ExtensionId::from_uuid(Uuid::parse_str("550e8400-e29b-41d4-a716-446655440001").unwrap()),
            timestamp: Utc::now(),
            payload: serde_json::json!({ "event": "test" }),
        };
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: BusEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event.id, deserialized.id);
        assert_eq!(event.topic, deserialized.topic);
        assert_eq!(event.source, deserialized.source);
        assert_eq!(event.payload, deserialized.payload);
    }
}
