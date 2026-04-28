use torque_harness::runtime;

#[test]
fn runtime_module_is_exported() {
    let _ = std::any::type_name::<runtime::host::KernelRuntimeHandle>();
}
