use core::arch::asm;
use core::fmt;
use core::fmt::Write;

pub fn _print(args: fmt::Arguments) {
    let mut writer = Writer::new();
    writer.write_fmt(args).expect("Printing to serial failed");
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::print::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\r\n"));
    ($($arg:tt)*) => ($crate::print!("\r{}\n", format_args!($($arg)*)));
}

struct Writer;

impl Writer {
    pub fn new() -> Writer {
        Writer {}
    }

    fn print_char(c: u8) {
        unsafe {
            asm!("mov ah, 0x0E; xor bh, bh; int 0x10", in("al") c);
        }
    }

    fn print_str(s: &str) {
        for c in s.chars() {
            if c.is_ascii() {
                Self::print_char(c as u8);
                if c == '\n' {
                    Self::print_char(b'\r');
                }
            } else {
                Self::print_char(b'.');
            }
        }
    }

    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            Self::print_char(c as u8);
        }

        Ok(())
    }
}

impl Write for Writer {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        Self::print_str(s);
        Ok(())
    }
}
