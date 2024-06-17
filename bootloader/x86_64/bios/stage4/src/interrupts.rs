// Populate IDT for easier debugging of crashes in the bootloader
use core::arch::asm;
use lazy_static::lazy_static;
use x86_64::{
    handler_with_error_code, handler_without_error_code,
    idt::InterruptDescriptorTable,
    interrupts::{ExceptionStackFrame, PageFaultErrorCode},
    pop_scratch_registers, println, push_scratch_registers,
};

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::default();

        unsafe {
            idt.divide_error
                .set_handler_function(handler_without_error_code!(divide_by_zero_handler));
            idt.breakpoint
                .set_handler_function(handler_without_error_code!(breakpoint_handler));
            idt.invalid_opcode
                .set_handler_function(handler_without_error_code!(invalid_opcode_handler));

            idt.page_fault
                .set_handler_function(handler_with_error_code!(page_fault_handler));

            idt.alignment_check
                .set_handler_function(handler_with_error_code!(alignment_check_handler));
            idt.double_fault
                .set_handler_function(handler_with_error_code!(double_fault_handler));
        }

        idt
    };
}

pub fn init() {
    IDT.load();
}

// C calling convention
extern "C" fn divide_by_zero_handler(frame: &ExceptionStackFrame) -> ! {
    println!("Divide by zero exception in bootloade: {:?}", frame);
    loop {}
}

extern "C" fn invalid_opcode_handler(frame: &ExceptionStackFrame) -> ! {
    println!("Invalid opcode in bootloader: {:?}", frame);
    loop {}
}

extern "C" fn general_protection_fault_handler(frame: &ExceptionStackFrame, error_code: u64) -> ! {
    println!("General protection fault in bootloader: {:?}", frame);
    loop {}
}

extern "C" fn page_fault_handler(frame: &ExceptionStackFrame, error_code: u64) {
    let error = PageFaultErrorCode::from_bits(error_code).unwrap();
    println!(
        "Page fault in bootloader: \n error_code: {:?} \n exception frame: {:?}",
        error, frame
    );
    // TODO: handle
    loop {}
}

extern "C" fn alignment_check_handler(frame: &ExceptionStackFrame, error_code: u64) -> ! {
    println!("Alignment check exception in bootloader: {:?}", frame);
    loop {}
}

extern "C" fn breakpoint_handler(frame: &ExceptionStackFrame) {
    println!("Int3 in bootloader: {:?}", frame);
}

extern "C" fn double_fault_handler(frame: &ExceptionStackFrame, error_code: u64) -> ! {
    println!("Double fault in bootloader: {:?}", frame);
    loop {}
}
