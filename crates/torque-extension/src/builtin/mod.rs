//! Built-in Extension implementations for Torque.
//!
//! These extensions ship with the `torque-extension` crate and are
//! available out-of-the-box when the `extension` feature is enabled
//! in `torque-harness`.
//!
//! ## Available Extensions
//!
//! - **LoggingExtension** — records lifecycle and execution events via `tracing`.
//! - **MetricsExtension** — collects counters (tool calls, errors, latency)
//!   and exposes them through the Actor message channel.

pub mod logging;
pub mod metrics;

pub use logging::LoggingExtension;
pub use metrics::MetricsExtension;
