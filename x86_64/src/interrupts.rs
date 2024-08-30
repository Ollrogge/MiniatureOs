use crate::register::{RFlags, RFlagsReg};
use bitflags::{bitflags, Flags};
use core::{arch::asm, fmt};

/// Disables CPU interrupts.
///
/// # Safety
///
/// This function is unsafe because it directly manipulates the CPU state. The caller must ensure
/// that disabling interrupts does not lead to deadlocks or race conditions in the code.
pub unsafe fn disable() {
    unsafe { asm!("cli", options(nostack, preserves_flags)) }
}

/// Enables CPU interrupts.
///
/// # Safety
///
/// This function is unsafe because it directly manipulates the CPU state. The caller must ensure
/// that re-enabling interrupts is safe and does not introduce race conditions with other threads
/// or interrupt handlers.
pub unsafe fn enable() {
    unsafe { asm!("sti", options(nostack, preserves_flags)) }
}

// todo: https://os.phil-opp.com/catching-exceptions/
// cur: https://os.phil-opp.com/double-fault-exceptions/
// exception numbers: https://wiki.osdev.org/Exceptions

// rax, rcx, rdx, rsi, rdi, r8, r9, r10, r11

// Interrupts can occur at any time so save the scratch registers which are normally
// caller saved. Dont need to save callee-saved register since compiler takes care
// of not clobbering those registers in our interrupt handlers
#[macro_export]
macro_rules! push_scratch_registers {
    () => {
        "push rax; push rcx; push rdx; push rsi; push rdi; push r8; push r9; push r10; push r11"
    };
}

#[macro_export]
macro_rules! pop_scratch_registers {
    () => {
        "pop r11; pop r10; pop r9; pop r8; pop rdi; pop rsi; pop rdx; pop rcx; pop rax"
    };
}

// Macro does not create naming conflicts since it returns a block expression with
// an anonymous namespace.
// Wrapper is naked to prevent the rust compiler from emitting the function prologue
// and epilogue

// stack frame layout when exception occurs: CPU pushes the stack and
// instruction pointers (with their segment descriptors), the RFLAGS register,
// and an optional error code
// RFLAGS registers contains the IF (interrupt enable flag). When using iretq
// the interrupt enable flag will be set to the value it had before the interrupt occured.
// => Therefore we don't need to re-enable interrupts even though we are using
// an interrupt gate

// diff interrupt, trap gate:
//  when you call an interrupt-gate, interrupts get disabled, and when you
//  call a trap-gate, they don't

// METHODS USE interrupt gate, so interrupts will be disabled on entry and enabled
// on exit

// pointer alignment needed since exception frame = 5 registers + 9 scratch registers + 1 error code = 15 => unaligned
#[macro_export]
macro_rules! handler_with_error_code {
    ($name: ident) => {{
        #[naked]
        extern "C" fn wrapper() -> ! {
            unsafe {
                asm!(
                    push_scratch_registers!(),
                    "mov rsi, [rsp + 9*8]", // load error code (cant use pop before saving scratch registers since this would corrupt rsi)
                    "mov rdi, rsp",
                    "add rdi, 10*8", // calculate exception stack frame pointer
                    "sub rsp, 8", // alignment
                    "call {}",
                    "add rsp, 8",
                    pop_scratch_registers!(),
                    "add rsp, 8", // pop error code
                    "iretq",
                    sym $name,
                    options(noreturn)
                )
            }
        }
        wrapper
    }}
}

// No pointer alignment needed since exception frame = 5 registers + 9 scratch registers = 14 => aligned
#[macro_export]
macro_rules! handler_without_error_code {
    ($name: ident) => {{
        #[naked]
        extern "C" fn wrapper() -> ! {
            unsafe {
                asm!(
                    push_scratch_registers!(),
                    "mov rdi, rsp",
                    "add rdi, 9*8",
                    "call {}",
                    pop_scratch_registers!(),
                    "iretq",
                    sym $name,
                    options(noreturn),
                )
            }
        }
        wrapper
    }}
}

bitflags! {
    #[derive(Debug)]
    pub struct PageFaultErrorCode: u64 {
        const PROTECTION_VIOLATION = 1 << 0;
        const WRITE_VIOLATION = 1 << 1;
        const USER_MODE = 1 << 2;
        const MALFORMED_TABLE = 1 << 3;
        const INSTRUCTION_FETCH = 1 << 4;
    }
}

impl PageFaultErrorCode {
    pub fn is_non_present_fault(&self) -> bool {
        self.bits() & PageFaultErrorCode::PROTECTION_VIOLATION.bits() == 0
    }

    pub fn is_write_fault(&self) -> bool {
        self.bits() & PageFaultErrorCode::WRITE_VIOLATION.bits() != 0
    }
}

// naked functions have no function prologue
/// Information the CPU pushes onto the stack before jumping to the exception
/// handler function
#[repr(C)]
pub struct ExceptionStackFrame {
    instruction_pointer: u64,
    code_segment: u64,
    // todo: make this a struct to better associate the fields
    // this bitfield struct thingy where you can say the struct is a u64
    cpu_flags: u64,
    stack_pointer: u64,
    stack_segment: u64,
}

impl fmt::Debug for ExceptionStackFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "ExceptionFrame {{")?;
        writeln!(f, "    IP: {:#016x}", self.instruction_pointer)?;
        writeln!(f, "    CS: {:#016x}", self.code_segment)?;
        writeln!(f, "    FLAGS: {:#016x}", self.cpu_flags)?;
        writeln!(f, "    SP: {:#016x}", self.stack_pointer)?;
        writeln!(f, "    SS: {:#016x}", self.stack_segment)?;
        write!(f, "}}")
    }
}

fn are_enabled() -> bool {
    RFlagsReg::read().contains(RFlags::INTERRUPT_FLAG)
}

pub fn without_interrupts<F, R>(c: F) -> R
where
    F: FnOnce() -> R,
{
    let were_enabled = are_enabled();

    if were_enabled {
        unsafe { disable() };
    }

    let ret = c();

    if were_enabled {
        unsafe { enable() };
    }

    ret
}
