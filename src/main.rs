use std::env;
fn main() {
    // read env variables that were set in build script
    let bios_path = env!("BIOS_PATH");

    let mut cmd = std::process::Command::new("qemu-system-x86_64");
    cmd.arg("-drive")
        .arg(format!("format=raw,file={bios_path}"));
    cmd.arg("-no-reboot");
    cmd.arg("-serial").arg("stdio");
    //cmd.arg("-nographic");
    cmd.arg("-monitor").arg("/dev/null");
    if env::consts::OS == "linux" {
        cmd.arg("-enable-kvm");
    }
    cmd.arg("-s");

    let mut child = cmd.spawn().unwrap();
    child.wait().unwrap();
}
