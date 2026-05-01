use serde::{
    Deserialize, Deserializer,
    Serialize, Serializer,
};
use std::fmt;
use std::sync::Arc;

/// A topic identifier for the EventBus.
///
/// Topics are namespaced with the convention `{namespace}:{name}`.
/// Subscribers match exact topic strings (no wildcards in Phase 1).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BusTopic(Arc<str>);

impl BusTopic {
    /// Predefined: Extension registered.
    pub const EXT_REGISTERED: &'static str = "ext:registered";
    /// Predefined: Extension unregistered.
    pub const EXT_UNREGISTERED: &'static str = "ext:unregistered";
    /// Predefined: Extension error.
    pub const EXT_ERROR: &'static str = "ext:error";

    /// Create a new topic from a namespace and name.
    pub fn new(ns: &str, name: &str) -> Self {
        Self(Arc::from(format!("{}:{}", ns, name)))
    }

    /// Create a topic from a raw string.
    pub fn from_str(s: &str) -> Self {
        Self(Arc::from(s.to_owned()))
    }

    /// Return the topic as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for BusTopic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// Custom serde implementation — Arc<str> doesn't support derive
impl Serialize for BusTopic {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for BusTopic {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(BusTopic(Arc::from(s)))
    }
}

impl From<&str> for BusTopic {
    fn from(s: &str) -> Self {
        BusTopic::from_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bus_topic_new() {
        let topic = BusTopic::new("ext", "test");
        assert_eq!(topic.as_str(), "ext:test");
    }

    #[test]
    fn test_bus_topic_from_str() {
        let topic = BusTopic::from_str("my:topic");
        assert_eq!(topic.as_str(), "my:topic");
    }

    #[test]
    fn test_bus_topic_from_str_ref() {
        let topic: BusTopic = "ext:registered".into();
        assert_eq!(topic.as_str(), "ext:registered");
    }

    #[test]
    fn test_bus_topic_display() {
        let topic = BusTopic::new("ns", "name");
        assert_eq!(format!("{}", topic), "ns:name");
    }

    #[test]
    fn test_bus_topic_debug() {
        let topic = BusTopic::from_str("ext:error");
        let debug = format!("{:?}", topic);
        assert!(debug.contains("ext:error"));
    }

    #[test]
    fn test_bus_topic_eq() {
        let a = BusTopic::from_str("same:topic");
        let b = BusTopic::from_str("same:topic");
        assert_eq!(a, b);
    }

    #[test]
    fn test_bus_topic_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(BusTopic::from_str("unique:topic"));
        assert!(set.contains(&BusTopic::from_str("unique:topic")));
        assert!(!set.contains(&BusTopic::from_str("other:topic")));
    }

    #[test]
    fn test_bus_topic_predefined_constants() {
        assert_eq!(BusTopic::EXT_REGISTERED, "ext:registered");
        assert_eq!(BusTopic::EXT_UNREGISTERED, "ext:unregistered");
        assert_eq!(BusTopic::EXT_ERROR, "ext:error");
    }

    #[test]
    fn test_bus_topic_serde_roundtrip() {
        let topic = BusTopic::new("serde", "test");
        let json = serde_json::to_string(&topic).unwrap();
        assert_eq!(json, "\"serde:test\"");
        let deserialized: BusTopic = serde_json::from_str(&json).unwrap();
        assert_eq!(topic, deserialized);
    }

    #[test]
    fn test_bus_topic_clone() {
        let topic = BusTopic::from_str("clone:me");
        let cloned = topic.clone();
        assert_eq!(topic, cloned);
    }
}
