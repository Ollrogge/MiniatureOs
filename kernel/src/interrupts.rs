use bitflags::bitflags;
use core::{
    arch::asm,
    fmt::{self, Debug},
};
use lazy_static::lazy_static;
use x86_64::{idt::InterruptDescriptorTable, instructions::int3, println};

// todo: https://os.phil-opp.com/catching-exceptions/
// cur: https://os.phil-opp.com/double-fault-exceptions/
// exception numbers: https://wiki.osdev.org/Exceptions

// rax, rcx, rdx, rsi, rdi, r8, r9, r10, r11

// Interrupts can occur at any time so save the scratch registers which are normally
// caller saved. Dont need to save callee-saved register since compiler takes care
// of not clobbering those registers in our interrupt handlers
macro_rules! push_scratch_registers {
    () => {
        "push rax; push rcx; push rdx; push rsi; push rdi; push r8; push r9; push r10; push r11"
    };
}

macro_rules! pop_scratch_registers {
    () => {
        "pop rax; pop rcx; pop rdx; pop rsi; pop rdi; pop r8; pop r9; pop r10; pop r11"
    };
}

// Macro does not create naming conflicts since it returns a block expression with
// an anonymous namespace.
// Wrapper is naked to prevent the rust compiler from emitting the function prologue
// and epilogue

// pointer alignment needed since exception frame = 5 registers + 9 scratch registers + 1 error code = 15 => unaligned
macro_rules! handler_with_error_code {
    ($name: ident) => {{
        #[naked]
        extern "C" fn wrapper() -> ! {
            unsafe {
                asm!(
                    push_scratch_registers!(),
                    "mov rsi, [rsp + 9*8]", // pop error code (cant use pop before saving scratch registers since this would corrupt rsi)
                    "mov rdi, rsp",
                    "add rdi, 10*8", // jump over saved scratch registers and error code
                    "sub rsp, 8",
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
                    options(noreturn)
                )
            }
        }
        wrapper
    }}
}

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::default();

        idt.divide_error
            .set_handler_function(handler_without_error_code!(divide_by_zero_handler));
        idt.breakpoint
            .set_handler_function(handler_without_error_code!(breakpoint_handler));
        idt.invalid_opcode
            .set_handler_function(handler_without_error_code!(invalid_opcode_handler));

        /*
        idt.page_fault
            .set_handler_function(handler_with_error_code!(page_fault_handler));
        */
        idt.alignment_check
            .set_handler_function(handler_with_error_code!(alignment_check_handler));
        idt.double_fault
            .set_handler_function(handler_with_error_code!(double_fault_handler));

        idt
    };
}

pub fn init() {
    IDT.load();
}

bitflags! {
    struct PageFaultErrorCode: u64 {
        const PROTECTION_VIOLATION = 1 << 0;
        const WRITE_VIOLATION = 1 << 1;
        const USER_MODE = 1 << 2;
        const MALFORMED_TABLE = 1 << 3;
        const INSTRUCTION_FETCH = 1 << 4;
    }
}

// naked functions have no function prologue
/// Information the CPU pushes onto the stack before jumping to the exception
/// handler function
#[repr(C)]
struct ExceptionStackFrame {
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

// C calling convention
extern "C" fn divide_by_zero_handler(frame: &ExceptionStackFrame) -> ! {
    println!("Exception: divide by zero");
    loop {}
}

extern "C" fn invalid_opcode_handler(frame: &ExceptionStackFrame) -> ! {
    println!("Invalid opcode handler");
    loop {}
}

extern "C" fn general_protection_fault_handler(frame: &ExceptionStackFrame, error_code: u64) -> ! {
    println!("General protection fault");
    loop {}
}

extern "C" fn page_fault_handler(frame: &ExceptionStackFrame, error_code: u64) {
    let error = PageFaultErrorCode::from_bits(error_code).unwrap();
    println!(
        "Page fault handler \n error_code: {:?} \n exception frame: {:?}",
        error, frame
    );
    // TODO: handle
    loop {}
}

extern "C" fn alignment_check_handler(frame: &ExceptionStackFrame, error_code: u64) -> ! {
    println!("Alignment check handler");
    loop {}
}

extern "C" fn breakpoint_handler(frame: &ExceptionStackFrame) {
    println!("Int3 triggered: {:?}", frame);
}

// double fault acts kind of like a catch-all block
// “double fault exception can occur when a second exception occurs during the
// handling of a prior (first) exception handler”. The “can” is important:
// Only very specific combinations of exceptions lead to a double fault
// https://os.phil-opp.com/double-fault-exceptions/
extern "C" fn double_fault_handler(frame: &ExceptionStackFrame, error_code: u64) -> ! {
    println!("Double fault handler");
    loop {}
}
