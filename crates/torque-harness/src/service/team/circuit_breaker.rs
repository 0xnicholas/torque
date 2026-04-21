use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};
use crate::models::v1::delegation_event::RejectionReason;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

pub struct CircuitBreaker {
    failure_threshold: usize,
    success_threshold: usize,
    state: RwLock<CircuitState>,
    failure_count: RwLock<usize>,
    success_count: RwLock<usize>,
    last_failure_time: RwLock<Option<DateTime<Utc>>>,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: usize, success_threshold: usize) -> Self {
        Self {
            failure_threshold,
            success_threshold,
            state: RwLock::new(CircuitState::Closed),
            failure_count: RwLock::new(0),
            success_count: RwLock::new(0),
            last_failure_time: RwLock::new(None),
        }
    }

    pub async fn state(&self) -> CircuitState {
        *self.state.read().await
    }

    pub async fn record_failure(&self, reason: &RejectionReason) {
        let mut count = self.failure_count.write().await;
        *count += 1;
        *self.last_failure_time.write().await = Some(Utc::now());

        if *count >= self.failure_threshold {
            *self.state.write().await = CircuitState::Open;
        }
    }

    pub async fn record_success(&self) {
        let mut count = self.success_count.write().await;
        *count += 1;

        if *count >= self.success_threshold {
            *self.state.write().await = CircuitState::Closed;
            *self.failure_count.write().await = 0;
            *count = 0;
        }
    }

    pub async fn allow_request(&self) -> bool {
        let state = self.state.read().await;
        match *state {
            CircuitState::Closed => true,
            CircuitState::HalfOpen => true,
            CircuitState::Open => false,
        }
    }

    pub async fn transition_to_half_open(&self) {
        *self.state.write().await = CircuitState::HalfOpen;
        *self.success_count.write().await = 0;
    }
}