use std::process::Command;
fn main() {
    let mut command = Command::new("qemu-system-x86_64");

    /*
    const QEMU_ARGS: &[&str] = &["-serial", "stdio", "-display", "none", "--no-reboot"];
    command
        .args(["-drive", "format=raw,file=disk_image.img"])
        .args(QEMU_ARGS);
    */

    command
        .args(["-drive", "format=raw,file=disk_image.img"])
        .arg("-no-reboot")
        .arg("-nographic")
        .args(["-monitor", "/dev/null"]);

    command.status().unwrap();
}
