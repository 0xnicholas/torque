use torque_harness::kernel_bridge::KernelRuntimeHandle as BridgeKernelRuntimeHandle;
use torque_harness::runtime::host::KernelRuntimeHandle as RuntimeKernelRuntimeHandle;

#[test]
fn runtime_host_is_available_from_the_new_module_path() {
    let _ = std::any::type_name::<BridgeKernelRuntimeHandle>();
    let _ = std::any::type_name::<RuntimeKernelRuntimeHandle>();
}
