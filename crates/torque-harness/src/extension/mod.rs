//! # Harness Extension Integration
//!
//! Bridges the `torque-extension` Actor runtime into the Harness layer.
//!
//! This module is only compiled when the `extension` feature is enabled.
//!
//! ## Structure
//!
//! - [`config`] — Configuration types for loading Extensions at startup
//! - [`runtime_handle`] — Wraps `InMemoryExtensionRuntime` with name-based
//!   lookup, suspend/resume, and lifecycle querying
//! - [`service`] — High-level service that manages registration, startup,
//!   and built-in extension loading

pub mod config;
pub mod runtime_handle;
pub mod service;

pub use config::HarnessExtensionConfig;
pub use runtime_handle::HarnessExtensionRuntimeHandle;
pub use service::ExtensionService;
