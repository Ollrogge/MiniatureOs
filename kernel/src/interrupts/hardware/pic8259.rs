//! This module implements a driver for the 8259 programmable interrupt controller (PIC)
//!
//! This is a very important chip in the x86 architecture as it enables the system to
//! handle devices based on interrupts instead of polling.
//!
//! The 8259 PIC has long been replaced by APIC but its interface is still supported
//! by current systems for backward compatibility.
//!
//! Intitially only a single 8259 PIC chip was used providing 8 IRQs. This was extended
//! by adding a second 8259 PIC chip using the 8259's ability to cascade interrupts
//! (have interrupts flow from one chip to another). By that a total of 15 IRQs could be supported.
//!
//! Almost all of the 15 interrupt lines have a fixed mapping:
//!                      ____________                          ____________
//! Real Time Clock --> |            |   Timer -------------> |            |
//! ACPI -------------> |            |   Keyboard-----------> |            |      _____
//! Available --------> | Secondary  |----------------------> | Primary    |     |     |
//! Available --------> | Interrupt  |   Serial Port 2 -----> | Interrupt  |---> | CPU |
//! Mouse ------------> | Controller |   Serial Port 1 -----> | Controller |     |_____|
//! Co-Processor -----> |            |   Parallel Port 2/3 -> |            |
//! Primary ATA ------> |            |   Floppy disk -------> |            |
//! Secondary ATA ----> |____________|   Parallel Port 1----> |____________|
//!
//! https://os.phil-opp.com/hardware-interrupts/
//!
//!
use x86_64::{
    port::{io_wait, Port},
    println,
};

#[repr(u8)]
enum InitialisationWord1 {
    Icw4 = 1,
    Init = 10,
}

#[repr(u8)]
enum InitialisationWord4 {
    Mode8086 = 1,
}

pub struct Pic {
    command: Port<u8>,
    data: Port<u8>,
}

const MASTER_PIC_BASE: u16 = 0x20;
const SLAVE_PIC_BASE: u16 = 0xa0;

impl Pic {
    pub const fn new(command: Port<u8>, data: Port<u8>) -> Self {
        Self { data, command }
    }

    pub fn read_command(&self) -> u8 {
        self.command.read()
    }

    pub fn write_command(&self, command: u8) {
        self.data.write(command)
    }

    pub fn read_data(&self) -> u8 {
        self.data.read()
    }

    pub fn write_data(&self, data: u8) {
        self.data.write(data)
    }
}

pub struct ChainedPics {
    master: Pic,
    slave: Pic,
}

impl ChainedPics {
    pub const fn new() -> Self {
        Self {
            master: Pic::new(Port::new(MASTER_PIC_BASE), Port::new(MASTER_PIC_BASE + 1)),
            slave: Pic::new(Port::new(SLAVE_PIC_BASE), Port::new(SLAVE_PIC_BASE + 1)),
        }
    }

    // https://wiki.osdev.org/8259_PIC
    pub fn init(&mut self, master_offset: u8, slave_offset: u8) {
        println!("Initializing PIC");
        // save masks
        // (When no command is issued, the data port allows us to access the interrupt mask of the 8259 PIC. )
        let master_mask = self.master.read_data();
        let slave_mask = self.slave.read_data();

        // start initialization sequence
        self.master
            .write_command(InitialisationWord1::Init as u8 | InitialisationWord1::Icw4 as u8);
        io_wait();

        self.slave
            .write_command(InitialisationWord1::Init as u8 | InitialisationWord1::Icw4 as u8);
        io_wait();

        // remap master interrupt vector offset
        self.master.data.write(master_offset);
        io_wait();

        // remap slave interrupt vector offset
        self.master.write_data(slave_offset);
        io_wait();

        // tell master there is a slave PIC at IRQ2 (third line)
        self.master.write_data(0x4);
        io_wait();

        // tell slave PIC its cascade identity
        self.slave.write_data(0x2);
        io_wait();

        // use 8086 mode
        self.master.write_data(InitialisationWord4::Mode8086 as u8);
        io_wait();

        self.slave.write_data(InitialisationWord4::Mode8086 as u8);
        io_wait();

        // restore masks
        self.master.write_data(master_mask);
        self.slave.write_data(slave_mask);
    }

    // TODO
    pub fn end_of_interrupt(&self) {}
}
