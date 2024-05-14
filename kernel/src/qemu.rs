use x86_64::uart::PortRegister;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed = 0x11,
}

pub fn exit(exit_code: QemuExitCode) -> ! {
    unsafe {
        let port = PortRegister::new(0xf4);
        port.write(exit_code as u32);
    }

    unreachable!();
}
