use crate::models::v1::delegation_event::RejectionReason;
use chrono::{DateTime, Duration, Utc};
use tokio::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

pub struct CircuitBreaker {
    failure_threshold: usize,
    success_threshold: usize,
    recovery_timeout: Duration,
    state: RwLock<CircuitState>,
    failure_count: RwLock<usize>,
    success_count: RwLock<usize>,
    last_failure_time: RwLock<Option<DateTime<Utc>>>,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: usize, success_threshold: usize) -> Self {
        Self::with_recovery_timeout(failure_threshold, success_threshold, Duration::seconds(30))
    }

    pub fn with_recovery_timeout(
        failure_threshold: usize,
        success_threshold: usize,
        recovery_timeout: Duration,
    ) -> Self {
        Self {
            failure_threshold,
            success_threshold,
            recovery_timeout,
            state: RwLock::new(CircuitState::Closed),
            failure_count: RwLock::new(0),
            success_count: RwLock::new(0),
            last_failure_time: RwLock::new(None),
        }
    }

    pub async fn state(&self) -> CircuitState {
        *self.state.read().await
    }

    pub async fn record_failure(&self, _reason: &RejectionReason) {
        let current_state = *self.state.read().await;

        match current_state {
            CircuitState::Closed => {
                let mut count = self.failure_count.write().await;
                *count += 1;
                *self.last_failure_time.write().await = Some(Utc::now());

                if *count >= self.failure_threshold {
                    *self.state.write().await = CircuitState::Open;
                }
            }
            CircuitState::HalfOpen => {
                *self.state.write().await = CircuitState::Open;
                *self.failure_count.write().await = 0;
                *self.success_count.write().await = 0;
                *self.last_failure_time.write().await = Some(Utc::now());
            }
            CircuitState::Open => {
                *self.last_failure_time.write().await = Some(Utc::now());
            }
        }
    }

    pub async fn record_success(&self) {
        let current_state = *self.state.read().await;

        match current_state {
            CircuitState::Closed => {
                let mut count = self.success_count.write().await;
                *count += 1;
                if *count >= self.success_threshold {
                    *self.state.write().await = CircuitState::Closed;
                    *self.failure_count.write().await = 0;
                    *count = 0;
                }
            }
            CircuitState::HalfOpen => {
                let mut count = self.success_count.write().await;
                *count += 1;
                if *count >= self.success_threshold {
                    *self.state.write().await = CircuitState::Closed;
                    *self.failure_count.write().await = 0;
                    *count = 0;
                }
            }
            CircuitState::Open => {}
        }
    }

    pub async fn allow_request(&self) -> bool {
        let current_state = *self.state.read().await;

        match current_state {
            CircuitState::Closed => true,
            CircuitState::HalfOpen => true,
            CircuitState::Open => {
                let last_failure = *self.last_failure_time.read().await;
                if let Some(time) = last_failure {
                    if Utc::now() - time >= self.recovery_timeout {
                        *self.state.write().await = CircuitState::HalfOpen;
                        *self.success_count.write().await = 0;
                        return true;
                    }
                }
                false
            }
        }
    }

    pub async fn transition_to_half_open(&self) {
        *self.state.write().await = CircuitState::HalfOpen;
        *self.success_count.write().await = 0;
    }
}
