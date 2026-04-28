use torque_harness::runtime::host::KernelRuntimeHandle;

#[test]
fn runtime_host_is_available_from_the_new_module_path() {
    let _ = std::any::type_name::<KernelRuntimeHandle>();
}
