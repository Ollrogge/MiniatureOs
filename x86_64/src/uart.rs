use bitflags::bitflags;
use core::{arch::asm, fmt, marker::PhantomData};

macro_rules! wait_for {
    ($cond:expr) => {
        while !$cond {
            core::hint::spin_loop()
        }
    };
}

bitflags! {
    struct LineStatusFlags: u8 {
        const DATA_READY = 1 << 0;
        const OVERRUN_ERROR = 1 << 1;
        const PARITY_ERROR = 1 << 2;
        const FRAMING_ERROR = 1 << 3;
        const BREAK_INDICATOR = 1 << 4;
        const TRANSMITTER_HOLDING_REGISTER_EMPTY = 1 << 5;
        const TRANSMITTER_EMPTY = 1 << 6;
        const IMPENDING_ERROR = 1 << 7;
    }
}

trait WritePortRegister {
    unsafe fn write_to_register(port: u16, val: Self);
}

trait ReadPortRegister {
    unsafe fn read_from_register(port: u16) -> Self;
}

impl ReadPortRegister for u8 {
    unsafe fn read_from_register(port: u16) -> u8 {
        let value: u8;
        unsafe {
            asm!("in al, dx", out("al")value, in("dx")port,
                 options(nomem, nostack, preserves_flags));
        }
        value
    }
}

impl WritePortRegister for u8 {
    unsafe fn write_to_register(port: u16, val: u8) {
        unsafe {
            asm!("out dx, al", in("dx")port, in("al")val,
            options(nomem, nostack, preserves_flags));
        }
    }
}

struct PortRegister<T> {
    address: u16,
    phantom: PhantomData<T>,
}

impl<T> PortRegister<T> {
    pub fn new(address: u16) -> Self {
        PortRegister {
            address,
            phantom: PhantomData,
        }
    }
}

impl<T: ReadPortRegister> PortRegister<T> {
    pub unsafe fn read(&self) -> T {
        unsafe { T::read_from_register(self.address) }
    }
}

impl<T: WritePortRegister> PortRegister<T> {
    pub unsafe fn write(&self, val: T) {
        unsafe { T::write_to_register(self.address, val) }
    }
}

impl<T> PartialEq for PortRegister<T> {
    fn eq(&self, other: &Self) -> bool {
        self.address == other.address
    }
}

#[allow(dead_code)]
pub struct SerialPort {
    data: PortRegister<u8>,
    int_en: PortRegister<u8>,
    fifo_ctrl: PortRegister<u8>,
    line_ctrl: PortRegister<u8>,
    modem_ctrl: PortRegister<u8>,
    line_stat: PortRegister<u8>,
    mode_stat: PortRegister<u8>,
}

impl SerialPort {
    pub fn new(base: u16) -> Self {
        SerialPort {
            data: PortRegister::new(base),
            int_en: PortRegister::new(base + 1),
            fifo_ctrl: PortRegister::new(base + 2),
            line_ctrl: PortRegister::new(base + 3),
            modem_ctrl: PortRegister::new(base + 4),
            line_stat: PortRegister::new(base + 5),
            mode_stat: PortRegister::new(base + 6),
        }
    }

    // 8N1 init routine
    pub fn init(&self) {
        unsafe {
            // disable interrupts
            self.int_en.write(0x0);

            // enable DLAB (set baud rate divisor)
            self.line_ctrl.write(0x80);

            // Set port speed to 38400 baud
            // low byte
            self.data.write(0x3);
            // high byte
            self.int_en.write(0x0);

            // 8 bits, no parity, one stop bit
            self.line_ctrl.write(0x3);

            // Enable FIFO, clear TX/RX queues and
            // set interrupt watermark at 14 bytes
            self.fifo_ctrl.write(0xC7);

            // Mark data terminal ready, signal request to send
            // and enable auxilliary output #2 (used as interrupt line for CPU)
            self.modem_ctrl.write(0x0B);

            // Enable interrupts
            self.int_en.write(0x01);
        }
    }

    fn line_status_flags(&self) -> LineStatusFlags {
        unsafe { LineStatusFlags::from_bits_truncate(self.line_stat.read()) }
    }

    pub fn send(&self, data: u8) {
        wait_for!(self
            .line_status_flags()
            .contains(LineStatusFlags::TRANSMITTER_HOLDING_REGISTER_EMPTY));

        unsafe { self.data.write(data) }
    }

    pub fn recv(&self) -> u8 {
        wait_for!(self
            .line_status_flags()
            .contains(LineStatusFlags::DATA_READY));

        unsafe { self.data.read() }
    }
}

impl fmt::Write for SerialPort {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            if c.is_ascii() {
                self.send(c as u8);
            } else {
                self.send(b'.');
            }
        }

        Ok(())
    }
}
