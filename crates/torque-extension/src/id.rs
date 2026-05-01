use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for an Extension instance.
///
/// Extensions are independent of AgentInstanceId — each Extension
/// gets its own UUID at registration time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExtensionId(Uuid);

impl ExtensionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for ExtensionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ExtensionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Semantic version for an Extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ExtensionVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl ExtensionVersion {
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self { major, minor, patch }
    }
}

impl Default for ExtensionVersion {
    fn default() -> Self {
        Self { major: 0, minor: 0, patch: 0 }
    }
}

impl std::fmt::Display for ExtensionVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn extension_id_new() {
        let id = ExtensionId::new();
        // Should not be nil
        assert_ne!(id.as_uuid(), Uuid::nil());
    }

    #[test]
    fn extension_id_from_uuid() {
        let uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let id = ExtensionId::from_uuid(uuid);
        assert_eq!(id.as_uuid(), uuid);
    }

    #[test]
    fn extension_id_display() {
        let uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let id = ExtensionId::from_uuid(uuid);
        assert_eq!(id.to_string(), "550e8400-e29b-41d4-a716-446655440000");
    }

    #[test]
    fn extension_id_equality() {
        let uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let a = ExtensionId::from_uuid(uuid);
        let b = ExtensionId::from_uuid(uuid);
        assert_eq!(a, b);
    }

    #[test]
    fn extension_id_default_is_new() {
        let id = ExtensionId::default();
        assert_ne!(id.as_uuid(), Uuid::nil());
    }

    #[test]
    fn extension_id_serde_roundtrip() {
        let id = ExtensionId::from_uuid(
            Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
        );
        let json = serde_json::to_string(&id).unwrap();
        let deserialized: ExtensionId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, deserialized);
    }

    #[test]
    fn extension_version_new() {
        let v = ExtensionVersion::new(1, 2, 3);
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
    }

    #[test]
    fn extension_version_display() {
        let v = ExtensionVersion::new(2, 0, 1);
        assert_eq!(v.to_string(), "2.0.1");
    }

    #[test]
    fn extension_version_ordering() {
        let v1 = ExtensionVersion::new(1, 0, 0);
        let v2 = ExtensionVersion::new(2, 0, 0);
        assert!(v1 < v2);
    }

    #[test]
    fn extension_version_serde() {
        let v = ExtensionVersion::new(3, 2, 1);
        let json = serde_json::to_string(&v).unwrap();
        let deserialized: ExtensionVersion = serde_json::from_str(&json).unwrap();
        assert_eq!(v, deserialized);
    }
}
