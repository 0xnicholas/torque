use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, RwLock,
};

use crate::id::ExtensionId;

/// Context provided to a hook handler at invocation time.
#[derive(Debug, Clone)]
pub struct HookContext {
    /// The Extension that registered this handler.
    pub extension_id: ExtensionId,
    /// Hook point name (e.g. "tool_call", "turn_start").
    pub hook_name: &'static str,
    /// The Agent instance associated with this event, if any.
    pub agent_id: Option<torque_kernel::AgentInstanceId>,
    /// Abort signal — check `is_cancelled()` to stop early.
    pub signal: AbortSignal,
}

impl HookContext {
    /// Convenience check for early cancellation.
    pub fn is_cancelled(&self) -> bool {
        self.signal.is_aborted()
    }
}

/// A shared cancellation signal.
///
/// When the runtime or a higher-priority handler decides to abort,
/// it calls `abort()`. Handlers should check `is_aborted()` periodically
/// and return `HookResult::Continue` (or a custom early-stop value)
/// if they wish to honour the cancellation.
#[derive(Clone)]
pub struct AbortSignal {
    inner: Arc<AbortSignalInner>,
}

struct AbortSignalInner {
    aborted: AtomicBool,
    listeners: RwLock<Vec<Box<dyn Fn() + Send + Sync>>>,
}

impl std::fmt::Debug for AbortSignalInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AbortSignalInner")
            .field("aborted", &self.aborted.load(Ordering::Relaxed))
            .field("listener_count", &self.listeners.read().map(|l| l.len()).unwrap_or(0))
            .finish()
    }
}

impl AbortSignal {
    /// Create a new, un-aborted signal.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(AbortSignalInner {
                aborted: AtomicBool::new(false),
                listeners: RwLock::new(Vec::new()),
            }),
        }
    }

    /// Returns `true` if this signal has been aborted.
    pub fn is_aborted(&self) -> bool {
        self.inner.aborted.load(Ordering::SeqCst)
    }

    /// Abort the signal and notify all registered listeners.
    pub fn abort(&self) {
        self.inner.aborted.store(true, Ordering::SeqCst);
        if let Ok(listeners) = self.inner.listeners.read() {
            for listener in listeners.iter() {
                listener();
            }
        }
    }

    /// Register a callback to be invoked when the signal is aborted.
    ///
    /// If the signal is already aborted the callback fires immediately.
    pub fn on_abort(&self, listener: Box<dyn Fn() + Send + Sync>) {
        if self.is_aborted() {
            listener();
            return;
        }
        if let Ok(mut listeners) = self.inner.listeners.write() {
            listeners.push(listener);
        }
    }
}

impl std::fmt::Debug for AbortSignal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AbortSignal")
            .field("aborted", &self.is_aborted())
            .finish()
    }
}

impl Default for AbortSignal {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn test_abort_signal_new_not_aborted() {
        let signal = AbortSignal::new();
        assert!(!signal.is_aborted());
    }

    #[test]
    fn test_abort_signal_abort() {
        let signal = AbortSignal::new();
        signal.abort();
        assert!(signal.is_aborted());
    }

    #[test]
    fn test_abort_signal_default_not_aborted() {
        let signal = AbortSignal::default();
        assert!(!signal.is_aborted());
    }

    #[test]
    fn test_abort_signal_on_abort_fires() {
        let signal = AbortSignal::new();
        let fired = Arc::new(AtomicBool::new(false));
        let fired_clone = fired.clone();
        signal.on_abort(Box::new(move || {
            fired_clone.store(true, Ordering::SeqCst);
        }));
        assert!(!fired.load(Ordering::SeqCst));
        signal.abort();
        assert!(fired.load(Ordering::SeqCst));
    }

    #[test]
    fn test_abort_signal_on_abort_already_aborted() {
        let signal = AbortSignal::new();
        signal.abort();
        let fired = Arc::new(AtomicBool::new(false));
        let fired_clone = fired.clone();
        signal.on_abort(Box::new(move || {
            fired_clone.store(true, Ordering::SeqCst);
        }));
        assert!(fired.load(Ordering::SeqCst));
    }

    #[test]
    fn test_abort_signal_clone_shares_state() {
        let signal1 = AbortSignal::new();
        let signal2 = signal1.clone();
        assert!(!signal2.is_aborted());
        signal1.abort();
        assert!(signal2.is_aborted());
    }

    #[test]
    fn test_abort_signal_debug() {
        let signal = AbortSignal::new();
        let debug = format!("{:?}", signal);
        assert!(debug.contains("AbortSignal"));
    }

    #[test]
    fn test_hook_context_construction() {
        let signal = AbortSignal::new();
        let ctx = HookContext {
            extension_id: ExtensionId::from_uuid(uuid::Uuid::nil()),
            hook_name: "tool_call",
            agent_id: None,
            signal: signal.clone(),
        };
        assert!(!ctx.is_cancelled());
        assert_eq!(ctx.hook_name, "tool_call");
        assert!(ctx.agent_id.is_none());
    }

    #[test]
    fn test_hook_context_is_cancelled() {
        let signal = AbortSignal::new();
        let ctx = HookContext {
            extension_id: ExtensionId::from_uuid(uuid::Uuid::nil()),
            hook_name: "test",
            agent_id: None,
            signal: signal.clone(),
        };
        assert!(!ctx.is_cancelled());
        signal.abort();
        assert!(ctx.is_cancelled());
    }
}
