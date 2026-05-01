pub mod checkpoint;
pub mod context;
pub mod environment;
pub mod events;
pub mod host;
pub use context::{
    CompactSummary, ContextCompactionPolicy, ContextCompactionService, CancellationToken,
    CompactionJob, CompactionJobStatus,
};
pub mod message;
pub mod message_queue;
pub mod offload;
pub mod tools;
pub mod vfs;

pub use environment::{
    ApprovalGateway, RuntimeCheckpointSink, RuntimeEventSink, RuntimeExecutionContext,
    RuntimeHydrationSource, RuntimeModelDriver, RuntimeOutputSink, RuntimeToolExecutor,
};
pub use host::RuntimeHost;
pub use torque_kernel::{ExecutionResult, StepDecision};

#[cfg(test)]
mod tests {
    use crate::{
        ApprovalGateway, RuntimeCheckpointSink, RuntimeEventSink, RuntimeExecutionContext,
        RuntimeHost, RuntimeHydrationSource, RuntimeModelDriver, RuntimeOutputSink,
        RuntimeToolExecutor,
    };
    use torque_kernel::ExecutionRequest;

    #[test]
    fn crate_exports_runtime_surface() {
        let _ = std::any::type_name::<RuntimeHost>();
        let _ = std::any::type_name::<ExecutionRequest>();
        let _ = std::any::type_name::<RuntimeExecutionContext>();
        let _ = std::any::type_name::<dyn RuntimeModelDriver>();
        let _ = std::any::type_name::<dyn RuntimeToolExecutor>();
        let _ = std::any::type_name::<dyn RuntimeEventSink>();
        let _ = std::any::type_name::<dyn RuntimeCheckpointSink>();
        let _ = std::any::type_name::<dyn RuntimeHydrationSource>();
        let _ = std::any::type_name::<dyn RuntimeOutputSink>();
        let _ = std::any::type_name::<dyn ApprovalGateway>();
    }
}
