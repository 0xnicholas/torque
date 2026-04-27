use torque_harness::kernel_bridge;
use torque_harness::runtime;

#[test]
fn runtime_module_is_exported_alongside_kernel_bridge() {
    let _ = std::any::type_name::<kernel_bridge::KernelRuntimeHandle>();
    let _ = std::any::type_name::<runtime::host::KernelRuntimeHandle>();
}
