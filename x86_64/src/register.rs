//! This module implements helper functions for x86 registers
use crate::{
    gdt::SegmentSelector,
    memory::{Address, PhysicalAddress, PhysicalFrame, VirtualAddress},
};
use bitflags::bitflags;
use core::arch::asm;

bitflags! {
    /// The RFLAGS register. All bit patterns are valid representations for this type.
    #[repr(transparent)]
    #[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Copy)]
    pub struct RFlags: u64 {
        /// Processor feature identification flag.
        ///
        /// If this flag is modifiable, the CPU supports CPUID.
        const ID = 1 << 21;
        /// Indicates that an external, maskable interrupt is pending.
        ///
        /// Used when virtual-8086 mode extensions (CR4.VME) or protected-mode virtual
        /// interrupts (CR4.PVI) are activated.
        const VIRTUAL_INTERRUPT_PENDING = 1 << 20;
        /// Virtual image of the INTERRUPT_FLAG bit.
        ///
        /// Used when virtual-8086 mode extensions (CR4.VME) or protected-mode virtual
        /// interrupts (CR4.PVI) are activated.
        const VIRTUAL_INTERRUPT = 1 << 19;
        /// Enable automatic alignment checking if CR0.AM is set. Only works if CPL is 3.
        const ALIGNMENT_CHECK = 1 << 18;
        /// Enable the virtual-8086 mode.
        const VIRTUAL_8086_MODE = 1 << 17;
        /// Allows to restart an instruction following an instruction breakpoint.
        const RESUME_FLAG = 1 << 16;
        /// Used by `iret` in hardware task switch mode to determine if current task is nested.
        const NESTED_TASK = 1 << 14;
        /// The high bit of the I/O Privilege Level field.
        ///
        /// Specifies the privilege level required for executing I/O address-space instructions.
        const IOPL_HIGH = 1 << 13;
        /// The low bit of the I/O Privilege Level field.
        ///
        /// Specifies the privilege level required for executing I/O address-space instructions.
        const IOPL_LOW = 1 << 12;
        /// Set by hardware to indicate that the sign bit of the result of the last signed integer
        /// operation differs from the source operands.
        const OVERFLOW_FLAG = 1 << 11;
        /// Determines the order in which strings are processed.
        const DIRECTION_FLAG = 1 << 10;
        /// Enable interrupts.
        const INTERRUPT_FLAG = 1 << 9;
        /// Enable single-step mode for debugging.
        const TRAP_FLAG = 1 << 8;
        /// Set by hardware if last arithmetic operation resulted in a negative value.
        const SIGN_FLAG = 1 << 7;
        /// Set by hardware if last arithmetic operation resulted in a zero value.
        const ZERO_FLAG = 1 << 6;
        /// Set by hardware if last arithmetic operation generated a carry ouf of bit 3 of the
        /// result.
        const AUXILIARY_CARRY_FLAG = 1 << 4;
        /// Set by hardware if last result has an even number of 1 bits (only for some operations).
        const PARITY_FLAG = 1 << 2;
        /// Set by hardware if last arithmetic operation generated a carry out of the
        /// most-significant bit of the result.
        const CARRY_FLAG = 1;
    }
}

// 64 bit wide reg
pub struct RFlagsReg;

impl RFlagsReg {
    pub fn read() -> RFlags {
        RFlags::from_bits_truncate(Self::read_raw() as u64)
    }

    pub fn read_raw() -> u64 {
        let mut val: u64 = 0;
        #[cfg(target_arch = "x86_64")]
        unsafe {
            asm!("pushfq; pop {}", out(reg) val, options(nomem, preserves_flags));
        }

        val
    }

    pub unsafe fn write(flags: RFlags) {
        let old_value = Self::read_raw() as u64;
        let reserved = old_value & !(RFlags::all().bits());
        let new_value = reserved | flags.bits();

        unsafe {
            Self::write_raw(new_value);
        }
    }

    pub unsafe fn write_raw(val: u64) {
        // HACK: we mark this function as preserves_flags to prevent Rust from restoring
        // saved flags after the "popf" below. See above note on safety.
        #[cfg(target_arch = "x86_64")]
        unsafe {
            asm!("push {}; popfq", in(reg) val, options(nomem, preserves_flags));
        }
    }
}

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

/// Model specific register.
/// This struct should not be used on its own. Only by implementations of
/// model specific registers
struct Msr;

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

/// The extended feature enable register.
/// This is a model-specific register mainly used to enable / disable long mode
pub struct Efer;

impl Efer {
    const MSR_NUM: u32 = 0xC0000080;

    /// Updates EFER register flags.
    ///
    /// # Safety
    ///
    /// Unsafe because it’s possible to break memory safety with wrong flags,
    /// e.g. by disabling long mode
    pub unsafe fn update<F>(f: F)
    where
        F: FnOnce(&mut EferFlags),
    {
        let mut flags = Self::read();
        f(&mut flags);
        Self::write(flags);
    }

    /// Reads the raw EFER register.
    pub fn read_raw() -> u64 {
        Msr::read(Self::MSR_NUM)
    }

    /// Reads the EFER flags.
    pub fn read() -> EferFlags {
        EferFlags::from_bits_truncate(Self::read_raw())
    }

    /// Writes a raw value to the EFER register
    ///
    /// # Safety
    ///
    /// Unsafe because it’s possible to break memory safety with wrong flags,
    /// e.g. by disabling long mode
    pub unsafe fn write_raw(val: u64) {
        Msr::write(Self::MSR_NUM, val)
    }

    /// Writes EFER flags
    ///
    /// # Safety
    ///
    /// Unsafe because it’s possible to break memory safety with wrong flags,
    /// e.g. by disabling long mode
    pub unsafe fn write(val: EferFlags) {
        Self::write_raw(val.bits())
    }
}

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

/// Control register 0. This register holds various configuration flags indicating
/// stuff like that cpu is in protected mode, or that paging is enabled
#[derive(Debug)]
pub struct Cr0;

impl Cr0 {
    /// Updates CR0 register flags.
    ///
    /// # Safety
    ///
    /// Unsafe because it’s possible to break memory safety with wrong flags,
    /// e.g. by disabling paging
    pub unsafe fn update<F>(f: F)
    where
        F: FnOnce(&mut Cr0Flags),
    {
        let mut flags = Self::read();
        f(&mut flags);
        Self::write(flags);
    }

    /// Reads the raw EFER register.
    pub fn read_raw() -> u64 {
        let mut cr0: usize;
        unsafe {
            asm!("mov {}, cr0", out(reg) cr0, options(nomem, nostack, preserves_flags));
        }
        cr0 as u64
    }

    /// Reads the CR0 flags.
    fn read() -> Cr0Flags {
        Cr0Flags::from_bits_truncate(Self::read_raw())
    }

    /// Writes CR0 flags
    ///
    /// # Safety
    ///
    /// Unsafe because it’s possible to break memory safety with wrong flags,
    /// e.g. by disabling paging
    pub unsafe fn write(val: Cr0Flags) {
        unsafe { Self::write_raw(val.bits()) }
    }

    /// Writes a raw value to the CR0 register
    ///
    /// # Safety
    ///
    /// Unsafe because it’s possible to break memory safety with wrong flags,
    /// e.g. by disabling paging
    pub unsafe fn write_raw(val: u64) {
        unsafe { asm!("mov cr0, {}", in(reg) val as usize, options(nostack, preserves_flags)) };
    }
}

#[derive(Debug)]
pub struct Cr2;

impl Cr2 {
    /// Reads the raw cr2 register.
    pub fn read_raw() -> u64 {
        let mut cr2: usize;
        unsafe {
            asm!("mov {}, cr2", out(reg) cr2, options(nomem, nostack, preserves_flags));
        }
        cr2 as u64
    }

    /// Reads the CR0 flags.
    pub fn read() -> VirtualAddress {
        VirtualAddress::new(Self::read_raw())
    }
}

bitflags! {
    /// Controls cache settings for the highest-level page table.
    ///
    /// Unused if paging is disabled or if [`PCID`](Cr4Flags::PCID) is enabled.
    #[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Copy)]
    pub struct Cr3Flags: u64 {
        /// Use a writethrough cache policy for the table (otherwise a writeback policy is used).
        const PAGE_LEVEL_WRITETHROUGH = 1 << 3;
        /// Disable caching for the table.
        const PAGE_LEVEL_CACHE_DISABLE = 1 << 4;
    }
}

#[derive(Debug)]
pub struct Cr3;

impl Cr3 {
    /// Updates CR3 register flags.
    ///
    /// # Safety
    ///
    /// Unsafe because it’s possible to break memory safety with wrong flags,
    /// e.g. by disabling paging
    pub unsafe fn update_flags<F>(f: F)
    where
        F: FnOnce(&mut Cr3Flags),
    {
        let (pml4t, mut flags) = Self::read();
        f(&mut flags);
        Self::write(pml4t, flags);
    }

    /// Updates CR3 page directory base address
    ///
    /// # Safety
    ///
    /// Unsafe because it’s possible to break memory safety with a wrong address
    pub unsafe fn update_pml4t_base(pml4t: PhysicalFrame) {
        let (_, flags) = Self::read();
        Self::write(pml4t, flags);
    }

    /// Reads the raw EFER register.
    pub fn read_raw() -> u64 {
        let mut cr3: usize;
        unsafe {
            asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack, preserves_flags));
        }
        cr3 as u64
    }

    /// Read pml4t address and CR3 flags
    pub fn read() -> (PhysicalFrame, Cr3Flags) {
        let raw = Self::read_raw();
        let frame =
            PhysicalFrame::containing_address(PhysicalAddress::new(raw & 0x_000f_ffff_ffff_f000));
        let flags = Cr3Flags::from_bits_truncate(raw & 0xfff);
        (frame, flags)
    }

    /// Writes CR0 flags
    ///
    /// Does not preserve any values
    ///
    /// # Safety
    ///
    /// Unsafe because it’s possible to break memory safety with wrong flags,
    /// e.g. by disabling paging
    pub unsafe fn write(frame: PhysicalFrame, val: Cr3Flags) {
        unsafe { Self::write_raw(frame.start() | val.bits()) }
    }

    /// Writes a raw value to the CR0 register
    ///
    /// # Safety
    ///
    /// Unsafe because it’s possible to break memory safety with wrong flags,
    /// e.g. by disabling paging
    pub unsafe fn write_raw(val: u64) {
        unsafe { asm!("mov cr0, {}", in(reg) val as usize, options(nostack, preserves_flags)) };
    }
}

/// Code Segment
///
/// While most fields in the Code-Segment [`Descriptor`] are unused in 64-bit
/// long mode, some of them must be set to a specific value. The
/// [`EXECUTABLE`](DescriptorFlags::EXECUTABLE),
/// [`USER_SEGMENT`](DescriptorFlags::USER_SEGMENT), and
/// [`LONG_MODE`](DescriptorFlags::LONG_MODE) bits must be set, while the
/// [`DEFAULT_SIZE`](DescriptorFlags::DEFAULT_SIZE) bit must be unset.
///
/// The [`DPL_RING_3`](DescriptorFlags::DPL_RING_3) field can be used to change
/// privilege level. The [`PRESENT`](DescriptorFlags::PRESENT) bit can be used
/// to make a segment present or not present.
///
/// All other fields (like the segment base and limit) are ignored by the
/// processor and setting them has no effect.
#[derive(Debug)]
pub struct CS;

impl CS {
    /// Reads the code segment register
    pub fn read() -> u16 {
        let mut cs: u16;
        unsafe { asm!("mov {:x}, cs", out(reg) cs, options(nostack, nomem, preserves_flags)) };
        cs
    }

    /// Writes to the code segment register
    ///
    /// Since wen can't directly write to the cs register push selector + new
    /// rip onto stack and retf
    ///
    /// # Safety
    ///
    /// Directly writing to the code segment register can lead to undefined
    /// behavior if the value is wrong
    ///
    #[cfg(target_arch = "x86_64")]
    pub unsafe fn write(val: SegmentSelector) {
        Self::write_raw(val.raw() as usize)
    }

    #[cfg(target_arch = "x86_64")]
    pub unsafe fn write_raw(val: usize) {
        unsafe {
            asm!(
                "push {sel}",
                "lea {tmp}, [2f + rip]",
                "push {tmp}",
                "retfq",
                "2:",
                sel = in(reg) val,
                tmp = lateout(reg) _,
                options(preserves_flags),
            );
        }
    }
}

/// Stack Segment
///
/// Entirely unused in 64-bit mode; setting the segment register does nothing.
/// However, in ring 3, the SS register still has to point to a valid
/// [`Descriptor`] (it cannot be zero). This
/// means a user-mode read/write segment descriptor must be present in the GDT.
///
/// This register is also set by the `syscall`/`sysret` and
/// `sysenter`/`sysexit` instructions (even on 64-bit transitions). This is to
/// maintain symmetry with 32-bit transitions where setting SS actually will
/// actually have an effect.
pub struct SS;
impl SS {
    /// Reads the code segment register
    pub fn read() -> u16 {
        let mut ss: u16;
        unsafe { asm!("mov {:x}, cs", out(reg) ss, options(nostack, nomem, preserves_flags)) };
        ss
    }

    /// Writes to the code segment register
    ///
    /// # Safety
    ///
    /// Directly writing to the code segment register can lead to undefined
    /// behavior if the value is wrong
    pub unsafe fn write(val: SegmentSelector) {
        Self::write_raw(val.raw())
    }

    pub unsafe fn write_raw(val: u16) {
        unsafe {
            asm!(
                "mov ss, {:x}", in(reg) val,
                options(nostack, nomem, preserves_flags)
            )
        };
    }
}

/// Data Segment
///
/// Entirely unused in 64-bit mode; setting the segment register does nothing.
#[derive(Debug)]
pub struct DS;
impl DS {
    /// Reads the ds register
    pub fn read() -> u16 {
        let mut ds: u16;
        unsafe { asm!("mov {:x}, ds", out(reg) ds, options(nostack, nomem, preserves_flags)) };
        ds
    }

    /// Writes to the ds register
    ///
    /// # Safety
    ///
    /// Directly writing to the ds register can lead to undefined behavior
    pub unsafe fn write(val: SegmentSelector) {
        Self::write_raw(val.raw());
    }

    pub unsafe fn write_raw(val: u16) {
        unsafe {
            asm!(
                "mov ds, {:x}", in(reg) val,
                options(nostack, nomem, preserves_flags)
            )
        };
    }
}

/// ES Segment
///
/// Entirely unused in 64-bit mode; setting the segment register does nothing.
#[derive(Debug)]
pub struct ES;
impl ES {
    /// Reads the es register
    pub fn read() -> u16 {
        let mut es: u16;
        unsafe { asm!("mov {:x}, es", out(reg) es, options(nostack, nomem, preserves_flags)) };
        es
    }

    /// Writes to the es register
    ///
    /// # Safety
    ///
    /// Directly writing to the es register can lead to undefined behavior
    pub unsafe fn write(val: SegmentSelector) {
        Self::write_raw(val.raw());
    }

    pub unsafe fn write_raw(val: u16) {
        unsafe {
            asm!(
                "mov es, {:x}", in(reg) val,
                options(nostack, nomem, preserves_flags)
            )
        };
    }
}

/// FS Segment
///
/// Only base is used in 64-bit mode, see [`Segment64`]. This is often used in
/// user-mode for Thread-Local Storage (TLS).
#[derive(Debug)]
pub struct FS;

/// GS Segment
///
/// Only base is used in 64-bit mode, see [`Segment64`]. In kernel-mode, the GS
/// base often points to a per-cpu kernel data structure.
#[derive(Debug)]
pub struct GS;
