#![no_std]
#![no_main]

mod uefi;
use core::panic::PanicInfo;

// https://medium.com/@applepies12/writing-an-os-in-rust-part-1-bb310ff2ee6d
// https://github.com/stakach/uefi-bootstrap/blob/master/bootstrap/uefi_bootstrap.zig

#[no_mangle]
pub extern "efiapi" fn efi_main(handle: uefi::Handle, system_table: *const uefi::SystemTable) {
    loop {}
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
