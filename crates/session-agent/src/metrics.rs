use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};

static SESSION_GATE_CONTENTION_TOTAL: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, Serialize)]
pub struct RuntimeMetricsSnapshot {
    pub session_gate_contention_total: u64,
}

pub fn increment_session_gate_contention_total() -> u64 {
    SESSION_GATE_CONTENTION_TOTAL.fetch_add(1, Ordering::Relaxed) + 1
}

pub fn session_gate_contention_total() -> u64 {
    SESSION_GATE_CONTENTION_TOTAL.load(Ordering::Relaxed)
}

pub fn snapshot() -> RuntimeMetricsSnapshot {
    RuntimeMetricsSnapshot {
        session_gate_contention_total: session_gate_contention_total(),
    }
}

pub fn reset_session_gate_contention_total_for_tests() {
    SESSION_GATE_CONTENTION_TOTAL.store(0, Ordering::Relaxed);
}
