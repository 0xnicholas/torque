pub struct KernelRuntimeHandle;

impl KernelRuntimeHandle {
    pub fn new(
        _agent_definitions: Vec<torque_kernel::AgentDefinition>,
        _event_repo: std::sync::Arc<dyn crate::repository::EventRepository>,
        _checkpoint_repo: std::sync::Arc<dyn crate::repository::CheckpointRepository>,
        _checkpointer: std::sync::Arc<dyn checkpointer::Checkpointer>,
    ) -> Self {
        todo!("implemented in Task 3.3")
    }
}
