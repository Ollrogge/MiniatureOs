use MiniatureOs::run_test_kernel;
#[test]
fn test_kernel_unittests() {
    run_test_kernel(env!("TEST_KERNEL_UNITTESTS_BIOS_PATH"));
}
