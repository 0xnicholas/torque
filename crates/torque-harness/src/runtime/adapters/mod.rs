//! Adapters that bridge the `torque-runtime` port traits to harness-specific
//! implementations.
//!
//! Five of seven runtime ports are implemented:
//! - [`HarnessEventSink`] → [`RuntimeEventSink`]
//! - [`HarnessModelDriver`] → [`RuntimeModelDriver`]
//! - [`StreamEventSinkAdapter`] → [`RuntimeOutputSink`]
//! - [`HarnessToolExecutor`] → [`RuntimeToolExecutor`]
//!
//! [`RuntimeCheckpointSink`] is implemented directly by
//! [`PostgresCheckpointer`] in [`crate::runtime::checkpoint`] — no adapter needed.
//!
//! Two ports are intentionally not implemented:
//! - [`RuntimeHydrationSource`]: the harness's [`RecoveryService`] handles
//!   state recovery directly via its own repository queries.
//! - [`ApprovalGateway`]: the runtime port is defined but not yet wired into
//!   [`RuntimeHost`]. The harness has an [`ApprovalService`] that could
//!   implement this trait once the host supports approval notifications.

pub mod event_sink;
pub mod model_driver;
pub mod output_sink;
pub mod tool_executor;

pub use event_sink::HarnessEventSink;
pub use model_driver::HarnessModelDriver;
pub use output_sink::StreamEventSinkAdapter;
pub use tool_executor::HarnessToolExecutor;
