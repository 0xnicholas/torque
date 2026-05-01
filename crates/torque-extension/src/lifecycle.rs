use serde::{Deserialize, Serialize};

/// Lifecycle states for an Extension.
///
/// ```text
/// Loaded ──► Registered ──► Initialized ──► Running
///   │            │               │             │
///   │            │               │             ▼
///   │            │               │         Suspended
///   │            │               │             │
///   │            │               │             ▼
///   │            │               │          Resumed
///   │            │               │             │
///   │            │               │             ▼
///   │            │               │          Stopped
///   │            │               │             │
///   ▼            ▼               ▼             ▼
/// Unloaded    Unregistered    Failed       Cleanup
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExtensionLifecycle {
    /// Loaded but not yet registered with the runtime.
    Loaded,
    /// Registered with the runtime, awaiting initialization.
    Registered,
    /// Initialized, ready to enter running state.
    Initialized,
    /// Running normally, handling hooks and messages.
    Running,
    /// Suspended (paused), can be resumed.
    Suspended,
    /// Stopped, awaiting cleanup.
    Stopped,
    /// Error state, awaiting cleanup.
    Failed,
    /// Cleanup complete — terminal.
    Cleanup,
    /// Removed from the runtime registry — terminal.
    Unregistered,
}

impl ExtensionLifecycle {
    /// Returns `true` if this is a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Cleanup | Self::Unregistered)
    }

    /// Returns `true` if the extension can process messages / hooks.
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Running)
    }

    /// Validate a transition from `self` to `next`.
    pub fn can_transition_to(&self, next: ExtensionLifecycle) -> bool {
        use ExtensionLifecycle::*;
        matches!(
            (self, next),
            (Loaded, Registered)
                | (Registered, Initialized)
                | (Registered, Unregistered)
                | (Initialized, Running)
                | (Initialized, Failed)
                | (Running, Suspended)
                | (Running, Stopped)
                | (Running, Failed)
                | (Suspended, Running)
                | (Stopped, Cleanup)
                | (Failed, Cleanup)
                | (Cleanup, Unregistered)
        )
    }
}

impl std::fmt::Display for ExtensionLifecycle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Loaded => write!(f, "loaded"),
            Self::Registered => write!(f, "registered"),
            Self::Initialized => write!(f, "initialized"),
            Self::Running => write!(f, "running"),
            Self::Suspended => write!(f, "suspended"),
            Self::Stopped => write!(f, "stopped"),
            Self::Failed => write!(f, "failed"),
            Self::Cleanup => write!(f, "cleanup"),
            Self::Unregistered => write!(f, "unregistered"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_state_is_loaded() {
        assert_eq!(ExtensionLifecycle::Loaded.to_string(), "loaded");
    }

    #[test]
    fn is_terminal() {
        assert!(ExtensionLifecycle::Cleanup.is_terminal());
        assert!(ExtensionLifecycle::Unregistered.is_terminal());
        assert!(!ExtensionLifecycle::Running.is_terminal());
        assert!(!ExtensionLifecycle::Loaded.is_terminal());
        assert!(!ExtensionLifecycle::Failed.is_terminal());
    }

    #[test]
    fn is_active() {
        assert!(ExtensionLifecycle::Running.is_active());
        assert!(!ExtensionLifecycle::Loaded.is_active());
        assert!(!ExtensionLifecycle::Suspended.is_active());
        assert!(!ExtensionLifecycle::Stopped.is_active());
        assert!(!ExtensionLifecycle::Failed.is_active());
    }

    #[test]
    fn valid_transitions() {
        let transitions = [
            (ExtensionLifecycle::Loaded, ExtensionLifecycle::Registered),
            (ExtensionLifecycle::Registered, ExtensionLifecycle::Initialized),
            (ExtensionLifecycle::Registered, ExtensionLifecycle::Unregistered),
            (ExtensionLifecycle::Initialized, ExtensionLifecycle::Running),
            (ExtensionLifecycle::Initialized, ExtensionLifecycle::Failed),
            (ExtensionLifecycle::Running, ExtensionLifecycle::Suspended),
            (ExtensionLifecycle::Running, ExtensionLifecycle::Stopped),
            (ExtensionLifecycle::Running, ExtensionLifecycle::Failed),
            (ExtensionLifecycle::Suspended, ExtensionLifecycle::Running),
            (ExtensionLifecycle::Stopped, ExtensionLifecycle::Cleanup),
            (ExtensionLifecycle::Failed, ExtensionLifecycle::Cleanup),
            (ExtensionLifecycle::Cleanup, ExtensionLifecycle::Unregistered),
        ];
        for (from, to) in &transitions {
            assert!(
                from.can_transition_to(*to),
                "expected valid transition: {:?} -> {:?}",
                from,
                to
            );
        }
    }

    #[test]
    fn invalid_transitions() {
        let invalid = [
            // Can't skip states
            (ExtensionLifecycle::Loaded, ExtensionLifecycle::Running),
            (ExtensionLifecycle::Loaded, ExtensionLifecycle::Cleanup),
            // Can't go backwards
            (ExtensionLifecycle::Running, ExtensionLifecycle::Initialized),
            (ExtensionLifecycle::Suspended, ExtensionLifecycle::Initialized),
            // Terminal is terminal
            (ExtensionLifecycle::Cleanup, ExtensionLifecycle::Running),
            (ExtensionLifecycle::Unregistered, ExtensionLifecycle::Loaded),
            // Missing intermediate steps
            (ExtensionLifecycle::Initialized, ExtensionLifecycle::Unregistered),
            (ExtensionLifecycle::Suspended, ExtensionLifecycle::Failed),
            (ExtensionLifecycle::Suspended, ExtensionLifecycle::Stopped),
        ];
        for (from, to) in &invalid {
            assert!(
                !from.can_transition_to(*to),
                "expected invalid transition: {:?} -> {:?}",
                from,
                to
            );
        }
    }

    #[test]
    fn display_all_states() {
        let cases = vec![
            (ExtensionLifecycle::Loaded, "loaded"),
            (ExtensionLifecycle::Registered, "registered"),
            (ExtensionLifecycle::Initialized, "initialized"),
            (ExtensionLifecycle::Running, "running"),
            (ExtensionLifecycle::Suspended, "suspended"),
            (ExtensionLifecycle::Stopped, "stopped"),
            (ExtensionLifecycle::Failed, "failed"),
            (ExtensionLifecycle::Cleanup, "cleanup"),
            (ExtensionLifecycle::Unregistered, "unregistered"),
        ];
        for (state, expected) in cases {
            assert_eq!(state.to_string(), expected);
        }
    }

    #[test]
    fn serde_roundtrip() {
        let state = ExtensionLifecycle::Running;
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: ExtensionLifecycle = serde_json::from_str(&json).unwrap();
        assert_eq!(state, deserialized);
    }

    #[test]
    fn equality() {
        assert_eq!(ExtensionLifecycle::Loaded, ExtensionLifecycle::Loaded);
        assert_ne!(ExtensionLifecycle::Loaded, ExtensionLifecycle::Running);
    }
}
