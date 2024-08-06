use MiniatureOs::run_test_kernel;

// run and see output: cargo test test_kernel_unittests -- --nocapture
#[test]
fn test_kernel_unittests() {
    run_test_kernel(env!("TEST_KERNEL_ALLOCATORS_BIOS_PATH"));
}
