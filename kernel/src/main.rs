#![no_std]
#![no_main]
use core::panic::PanicInfo;

static mut TEST: [u8; 0xabc123] = [0; 0xabc123];

#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
#[link_section = ".start"]
pub extern "C" fn _start() -> ! {
    let mut cnt = 0x0;
    unsafe {
        for i in 0..TEST.len() {
            cnt += TEST[i];
            core::hint::spin_loop();
        }
    }
    loop {}
}
