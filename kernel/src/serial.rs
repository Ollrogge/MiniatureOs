use core::fmt;
use lazy_static::lazy_static;
use util::mutex::Mutex;
use x86_64::{
    interrupts::{self},
    uart::*,
};

lazy_static! {
    pub static ref SERIAL: Mutex<SerialPort> = {
        let serial_port = SerialPort::new(0x3F8);
        serial_port.init();
        Mutex::new(serial_port)
    };
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;

    interrupts::without_interrupts(|| {
        SERIAL
            .lock()
            .write_fmt(args)
            .expect("Printing to serial failed");
    })
}

#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => ($crate::serial::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! serial_println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::serial_print!("{}\n", format_args!($($arg)*)));
}
