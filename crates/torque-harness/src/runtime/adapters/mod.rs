//! Adapters that bridge the `torque-runtime` port traits to harness-specific
//! implementations.
//!
//! Five of seven runtime ports are implemented:
//! - [`HarnessCheckpointSink`] → [`RuntimeCheckpointSink`]
//! - [`HarnessEventSink`] → [`RuntimeEventSink`]
//! - [`HarnessModelDriver`] → [`RuntimeModelDriver`]
//! - [`StreamEventSinkAdapter`] → [`RuntimeOutputSink`]
//! - [`HarnessToolExecutor`] → [`RuntimeToolExecutor`]
//!
//! Two ports are intentionally not implemented:
//! - [`RuntimeHydrationSource`]: the harness's [`RecoveryService`] handles
//!   state recovery directly via its own repository queries, bypassing the
//!   runtime hydration port. Implementing this adapter would require
//!   refactoring the recovery service to route through the runtime layer.
//! - [`ApprovalGateway`]: the runtime port is defined in the spec but not
//!   yet wired into [`RuntimeHost`]. The harness has an [`ApprovalService`]
//!   that could implement this trait once the host supports approval
//!   notifications.

pub mod checkpoint_sink;
pub mod event_sink;
pub mod model_driver;
pub mod output_sink;
pub mod tool_executor;

pub use checkpoint_sink::HarnessCheckpointSink;
pub use event_sink::HarnessEventSink;
pub use model_driver::HarnessModelDriver;
pub use output_sink::StreamEventSinkAdapter;
pub use tool_executor::HarnessToolExecutor;
