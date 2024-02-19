use bitflags::bitflags;
use core::arch::asm;

bitflags! {
    /// Flags of the Extended Feature Enable Register.
    pub struct EferFlags: u64 {
        /// Enables the `syscall` and `sysret` instructions.
        const SYSTEM_CALL_EXTENSIONS = 1;
        /// Activates long mode, requires activating paging.
        const LONG_MODE_ENABLE = 1 << 8;
        /// Indicates that long mode is active.
        const LONG_MODE_ACTIVE = 1 << 10;
        /// Enables the no-execute page-protection feature.
        const NO_EXECUTE_ENABLE = 1 << 11;
        /// Enables SVM extensions.
        const SECURE_VIRTUAL_MACHINE_ENABLE = 1 << 12;
        /// Enable certain limit checks in 64-bit mode.
        const LONG_MODE_SEGMENT_LIMIT_ENABLE = 1 << 13;
        /// Enable the `fxsave` and `fxrstor` instructions to execute faster in 64-bit mode.
        const FAST_FXSAVE_FXRSTOR = 1 << 14;
        /// Changes how the `invlpg` instruction operates on TLB entries of upper-level entries.
        const TRANSLATION_CACHE_EXTENSION = 1 << 15;
    }
}

pub struct Msr;

impl Msr {
    pub fn read(num: u32) -> u64 {
        let (high, low): (u32, u32);
        unsafe {
            asm!(
                "rdmsr",
                in("ecx") num,
                out("eax") low, out("edx") high,
                options(nomem, nostack, preserves_flags),
            );
        }
        ((high as u64) << 32) | (low as u64)
    }

    pub fn write(num: u32, val: u64) {
        let high = (val >> 32) as u32;
        let low = val as u32;

        unsafe {
            asm!(
                "wrmsr",
                in("ecx") num,
                in("eax") low,
                in("edx") high,
                options(nomem, nostack, preserves_flags),
            )
        }
    }
}

pub struct Efer;

impl Efer {
    const NUM: u32 = 0xC0000080;

    pub fn update<F>(f: F)
    where
        F: FnOnce(&mut EferFlags),
    {
        let mut flags = Self::read();
        f(&mut flags);
        Self::write(flags);
    }

    pub fn read_raw() -> u64 {
        Msr::read(Self::NUM)
    }

    pub fn read() -> EferFlags {
        EferFlags::from_bits_truncate(Self::read_raw())
    }

    pub fn write(val: EferFlags) {
        Self::write_raw(val.bits())
    }

    pub fn write_raw(val: u64) {
        Msr::write(Self::NUM, val)
    }
}

#[derive(Debug)]
pub struct Cr0;

bitflags! {
    /// Configuration flags of the [`Cr0`] register.
    pub struct Cr0Flags: u64 {
        /// Enables protected mode.
        const PROTECTED_MODE_ENABLE = 1;
        /// Enables monitoring of the coprocessor, typical for x87 instructions.
        ///
        /// Controls (together with the [`TASK_SWITCHED`](Cr0Flags::TASK_SWITCHED)
        /// flag) whether a `wait` or `fwait` instruction should cause an `#NE` exception.
        const MONITOR_COPROCESSOR = 1 << 1;
        /// Force all x87 and MMX instructions to cause an `#NE` exception.
        const EMULATE_COPROCESSOR = 1 << 2;
        /// Automatically set to 1 on _hardware_ task switch.
        ///
        /// This flags allows lazily saving x87/MMX/SSE instructions on hardware context switches.
        const TASK_SWITCHED = 1 << 3;
        /// Indicates support of 387DX math coprocessor instructions.
        ///
        /// Always set on all recent x86 processors, cannot be cleared.
        const EXTENSION_TYPE = 1 << 4;
        /// Enables the native (internal) error reporting mechanism for x87 FPU errors.
        const NUMERIC_ERROR = 1 << 5;
        /// Controls whether supervisor-level writes to read-only pages are inhibited.
        ///
        /// When set, it is not possible to write to read-only pages from ring 0.
        const WRITE_PROTECT = 1 << 16;
        /// Enables automatic usermode alignment checking if [`RFlags::ALIGNMENT_CHECK`] is also set.
        const ALIGNMENT_MASK = 1 << 18;
        /// Ignored, should always be unset.
        ///
        /// Must be unset if [`CACHE_DISABLE`](Cr0Flags::CACHE_DISABLE) is unset.
        /// Older CPUs used this to control write-back/write-through cache strategy.
        const NOT_WRITE_THROUGH = 1 << 29;
        /// Disables some processor caches, specifics are model-dependent.
        const CACHE_DISABLE = 1 << 30;
        /// Enables paging.
        ///
        /// If this bit is set, [`PROTECTED_MODE_ENABLE`](Cr0Flags::PROTECTED_MODE_ENABLE) must be set.
        const PAGING = 1 << 31;
    }
}

impl Cr0 {
    pub fn update<F>(f: F)
    where
        F: FnOnce(&mut Cr0Flags),
    {
        let mut flags = Self::read();
        f(&mut flags);
        Self::write(flags);
    }

    // usize is a hack to make this compile from 16 / 32 bit context as well
    fn read() -> Cr0Flags {
        Cr0Flags::from_bits_truncate(Self::read_raw())
    }

    pub fn read_raw() -> u64 {
        let mut cr0: usize;
        unsafe {
            asm!("mov {}, cr0", out(reg) cr0, options(nomem, nostack, preserves_flags));
        }
        cr0 as u64
    }

    // usize is a hack to make this compile from 16 / 32 bit context as well
    pub fn write(val: Cr0Flags) {
        Self::write_raw(val.bits())
    }

    pub fn write_raw(val: u64) {
        unsafe { asm!("mov cr0, {}", in(reg) val as usize, options(nostack, preserves_flags)) };
    }
}
