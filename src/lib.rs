use std::env;
pub fn run_test_kernel(img_path: &str) {
    let mut cmd = std::process::Command::new("qemu-system-x86_64");
    cmd.arg("-drive").arg(format!("format=raw,file={img_path}"));
    cmd.arg("-no-reboot");
    cmd.arg("-nographic");
    cmd.arg("-monitor").arg("/dev/null");
    cmd.arg("-device")
        .arg("isa-debug-exit,iobase=0xf4,iosize=0x04");
    if env::consts::OS == "linux" {
        cmd.arg("-enable-kvm");
    }

    let output = cmd.output().expect("failed to execute qemu");
    assert_eq!(
        output.status.code(),
        Some(33),
        "test failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ); // 33 = success, 35 = failure. Idk why

    println!("{}", String::from_utf8_lossy(&output.stdout));
}
