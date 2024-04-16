use crate::println;
use core::{
    arch::asm,
    fmt::{self, Debug},
};
use lazy_static::lazy_static;
use x86_64::idt::InterruptDescriptorTable;

// todo: https://os.phil-opp.com/catching-exceptions/
// numbers: https://wiki.osdev.org/Exceptions

// naked functions have no function prologue

/// Information the CPU pushes onto the stack before calling the exception
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
        write!(f, "}}") // `write!` for the last line to avoid a new line after the closing brace
    }
}

macro_rules! handler_with_error_code {
    ($name: ident) => {{
        #[naked]
        extern "C" fn wrapper() -> ! {
            unsafe {
                asm!(
                    "pop rsi", // pop error code
                    "mov rdi, rsp",
                    "sub rsp, 8", // align stack pointer to 16 byte boundary
                    "call {}",
                    sym $name,
                    options(noreturn)
                )
            }
        }
        wrapper
    }}
}

macro_rules! handler_without_error_code {
    ($name: ident) => {{
        #[naked]
        extern "C" fn wrapper() -> ! {
            unsafe {
                asm!(
                    "mov rdi, rsp",
                    "sub rsp, 8", // align stack pointer to 16 byte boundary
                    "call {}",
                    sym $name,
                    options(noreturn)
                )
            }
        }
        wrapper
    }}
}

/*
pub fn print_char(c: u8) {
    unsafe {
        asm!("mov ah, 0x0E; xor bh, bh; int 0x10", in("al") c);
    }
}
*/

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::default();

        idt.divide_error
            .set_handler_function(handler_without_error_code!(divide_by_zero_handler));
        idt.breakpoint
            .set_handler_function(handler_without_error_code!(breakpoint_handler));

        idt
    };
}

pub fn init() {
    //let test = IDT.load as *const _ as u64;
    /*
    todo: make asserts work properly with logs ?
    assert_eq!(
        (&IDT as *const _ as usize) % 16,
        0,
        "IDT is not 16-byte aligned"
    );
    */
    println!("IDT addr: {:p}", &IDT);
    IDT.load();
}

extern "C" fn divide_by_zero_handler(frame: &ExceptionStackFrame) -> ! {
    println!("Exception: divide by zero");
    loop {}
}

extern "C" fn breakpoint_handler(frame: &ExceptionStackFrame) -> ! {
    println!("Int3 triggered: {:?}", frame);
    loop {}
}
