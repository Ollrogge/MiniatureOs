use MiniatureOs::run_test_kernel;

// run and see output: cargo test test_kernel_unittests -- --nocapture
#[test]
fn test_kernel_allocators() {
    run_test_kernel(env!("TEST_KERNEL_ALLOCATORS_BIOS_PATH"));
}

#[test]
fn test_kernel_multitasking() {
    run_test_kernel(env!("TEST_KERNEL_MULTITASKING_BIOS_PATH"));
}
