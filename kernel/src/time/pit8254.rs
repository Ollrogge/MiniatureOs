use bitflags::bitflags;
use x86_64::port::{io_wait, Port};

bitflags! {
    pub struct CommandRegisterFlags: u8 {
        const CHANNEL0 = 0 << 6;
        const CHANNEL1 = 1 << 6;
        const CHANNEL2 = 2 << 6;
        const READ_BACK_COMMAND = 3 << 6;

        const LATCH_COUNT_VAL_CMD = 0 << 4;
        const ACCESS_MODE_LOBYTE_ONLY = 1 << 4;
        const ACCESS_MODE_HIBYTE_ONLY = 2 << 4;
        const ACCESS_MODE_LOBYTE_HIBYTE = 3 << 4;

        // interrupt on terminal count
        const OPERATING_MODE0 = 0;
        // hardware re-triggerable one-shot
        const OPERATING_MODE1 = 1 << 1;
        const OPERATING_MODE2 = 2 << 1;
        const OPERATING_MODE3 = 3 << 1;
        const OPERATING_MODE4 = 4 << 1;
        const OPERATING_MODE5 = 5 << 1;

        const BCD_MODE = 1;
        const BINARY_MODE = 0;
    }

    pub struct ReadBackCommandFlags: u8 {
        // 1 == don't latch count
        const DONT_LATCH_COUNT_FLAG = 1 << 5 | CommandRegisterFlags::READ_BACK_COMMAND.bits();
        const LATCH_COUNT_FLAG = 0 << 5 | CommandRegisterFlags::READ_BACK_COMMAND.bits(); // 1 == don't latch status
        // 1 == don't latch status
        const DONT_LATCH_STATUS_FLAG = 1 << 4 | CommandRegisterFlags::READ_BACK_COMMAND.bits();
        const LATCH_STATUS_FLAG = 0 << 4 | CommandRegisterFlags::READ_BACK_COMMAND.bits();

        const READ_BACK_TIMER_CHANNEL2 = 1 << 3 | CommandRegisterFlags::READ_BACK_COMMAND.bits();
        const READ_BACK_TIMER_CHANNEL1 = 1 << 2 | CommandRegisterFlags::READ_BACK_COMMAND.bits();
        const READ_BACK_TIMER_CHANNEL0 = 1 << 1 | CommandRegisterFlags::READ_BACK_COMMAND.bits();
    }

    // other flags are equal to CommandRegisterFlags
    pub struct ReadBackStatusFlags: u8 {
        const OUTPUT_PIN_STATE = 1 << 7;
        const NULL_COUNT_FLAGS = 1 << 6;
    }

}

pub struct Pit {
    data0: Port<u8>,
    data1: Port<u8>,
    data2: Port<u8>,
    // write only
    command: Port<u8>,
}

/*
TODO : couldnt get this to work

macro_rules! create_read {
    ($num:literal) => {
        pub fn read_data$num(&self) -> u8 {
            self.data$num.read()
        }
    };
}


macro_rules! create_write {
    ($num:literal) => {
        pub fn read_data$num(&self, val: u8) {
            self.data$num.write(val)
        }
    };
}
*/

/*
    configuration example :

    void pit_init() {
        uint16_t divisor = PIT_FREQUENCY / DESIRED_FREQUENCY;
        outb(PIT_COMMAND, 0x36);
        outb(PIT_CHANNEL0, (uint8_t)(divisor & 0xFF));
        outb(PIT_CHANNEL0, (uint8_t)((divisor >> 8) & 0xFF));
        idt_set_descriptor(0x20, pit_irq_handler, 0x8E);
        print("PIT: Initialized\n");
    }
*/

impl Pit {
    // 1.193182 MHz
    pub const FREQUENCY: u64 = 1_193_182;
    pub const fn new() -> Self {
        Self {
            data0: Port::new(0x40),
            data1: Port::new(0x41),
            data2: Port::new(0x42),
            command: Port::new(0x43),
        }
    }

    pub fn read_data0(&self) -> u8 {
        self.data0.read()
    }

    pub fn read_data1(&self) -> u8 {
        self.data1.read()
    }

    pub fn read_data2(&self) -> u8 {
        self.data2.read()
    }

    pub fn write_data0(&self, data: u8) {
        self.data0.write(data)
    }

    pub fn write_data1(&self, data: u8) {
        self.data1.write(data)
    }

    pub fn write_data2(&self, data: u8) {
        self.data2.write(data)
    }

    pub fn write_command(&self, command: u8) {
        self.command.write(command)
    }
}
