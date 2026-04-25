pub mod checkpoint;
pub mod context;
pub mod environment;
pub mod events;
pub mod host;
pub mod tools;

#[cfg(test)]
mod tests {
    use crate::host::RuntimeHost;
    use torque_kernel::ExecutionRequest;

    #[test]
    fn crate_exports_runtime_surface() {
        let _ = std::any::type_name::<RuntimeHost>();
        let _ = std::any::type_name::<ExecutionRequest>();
    }
}
