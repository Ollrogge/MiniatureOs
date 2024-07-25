pub mod pit8254;

use crate::serial_println;
use core::sync::atomic::{AtomicU64, Ordering};
use pit8254::{CommandRegisterFlags, Pit, ReadBackCommandFlags, ReadBackStatusFlags};
use x86_64::{instructions::rdtsc, interrupts};

static TIME: Time = Time::new();

pub const HZ: u64 = 1;
pub const KHZ: u64 = 1000 * HZ;
pub const MHZ: u64 = 1000 * KHZ;
pub const US_PER_MS: u64 = 1_000;
pub const US_PER_S: u64 = 1_000_000;

pub struct Time {
    rdtsc_start: AtomicU64,
    rdtsc_mhz: AtomicU64,
}

pub fn init() {
    TIME.calibrate();
}

impl Time {
    pub const fn new() -> Self {
        Self {
            rdtsc_start: AtomicU64::new(0),
            rdtsc_mhz: AtomicU64::new(0),
        }
    }

    pub fn future_s(seconds: u64) -> u64 {
        rdtsc() + (seconds * US_PER_S * TIME.rdtsc_mhz())
    }

    pub fn now() -> u64 {
        rdtsc()
    }

    pub fn rdtsc_mhz(&self) -> u64 {
        self.rdtsc_mhz.load(Ordering::Relaxed)
    }

    pub fn rdtsc_start(&self) -> u64 {
        self.rdtsc_start.load(Ordering::Relaxed)
    }

    pub fn elapsed_us(start: u64) -> u64 {
        ((rdtsc() - start) as f64 / TIME.rdtsc_mhz() as f64) as u64
    }

    pub fn elapsed_s(start: u64) -> u64 {
        (Self::elapsed_us(start) / US_PER_S) as u64
    }

    pub fn elapsed_ms(start: u64) -> u64 {
        (Self::elapsed_us(start) / US_PER_MS) as u64
    }

    pub fn uptime_us() -> u64 {
        TIME.rdtsc_start()
    }

    pub fn uptime_s() -> u64 {
        (TIME.rdtsc_start() / US_PER_S) as u64
    }

    pub fn uptime_ms() -> u64 {
        (TIME.rdtsc_start() / US_PER_MS) as u64
    }

    // approach taken from brandon falks chocolate milk: https://github.com/gamozolabs/chocolate_milk/blob/master/kernel/src/time.rs#L61
    // determine frequency of rdtsc using PIT
    pub fn calibrate(&self) {
        let pit = Pit::new();
        unsafe { interrupts::disable() };

        let start = rdtsc();

        self.rdtsc_start.store(start, Ordering::Relaxed);

        // read back the config for channel 0
        pit.write_command(ReadBackCommandFlags::READ_BACK_TIMER_CHANNEL0.bits());

        // config stored in bits 0-5
        // is OPERATING_MODE2
        let old_config = pit.read_data0() & ((1 << 6) - 1);

        // Channel 0, interrupt on termincal count, lobyte/hibyte
        pit.write_command(
            CommandRegisterFlags::CHANNEL0.bits()
                | CommandRegisterFlags::OPERATING_MODE0.bits()
                | CommandRegisterFlags::ACCESS_MODE_LOBYTE_HIBYTE.bits(),
        );

        // Configure Pit to count down from 65535. This takes roughly 54.92 milliseconds (65535 / 1193182).
        // We poll by sending the read back command to check whether the output pin is set to 1, indicating the
        // countdown completed.
        pit.write_data0(0xff);
        pit.write_data0(0xff);

        loop {
            pit.write_command(
                (ReadBackCommandFlags::READ_BACK_TIMER_CHANNEL0
                    | ReadBackCommandFlags::LATCH_COUNT_FLAG)
                    .bits(),
            );

            if pit.read_data0() & ReadBackStatusFlags::OUTPUT_PIN_STATE.bits() != 0 {
                break;
            }
        }

        let end = rdtsc();

        // time in seconds that the countdown was supposed to take
        let elapsed = 65535f64 / Pit::FREQUENCY as f64;

        // Compute MHz for the rdtsc
        let computed_rate = ((end - start) as f64) / elapsed / MHZ as f64;

        // Round to the nearest 100MHz value
        let rounded_rate = (((computed_rate / 100.0) + 0.5) as u64) * 100;

        serial_println!("Rounded tsc rate: {:#x} MHz", rounded_rate);

        self.rdtsc_mhz.store(rounded_rate, Ordering::Relaxed);

        // restore old config
        pit.write_command(old_config | CommandRegisterFlags::CHANNEL0.bits());
        // set reload value to 65535
        pit.write_data0(0x0);
        pit.write_data0(0x0);

        unsafe { interrupts::enable() }
    }
}
