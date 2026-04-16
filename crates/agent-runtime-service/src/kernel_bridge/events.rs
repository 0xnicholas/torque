pub struct EventRecorder;

impl EventRecorder {
    pub fn to_db_events(
        _result: &torque_kernel::ExecutionResult,
        _sequence_offset: u64,
    ) -> Vec<crate::models::v1::event::Event> {
        todo!("implemented in Task 3.2")
    }
}
