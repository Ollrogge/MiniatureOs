use bitflags::bitflags;
use core::{
    arch::asm,
    fmt::{self, Debug},
};
use lazy_static::lazy_static;
use x86_64::{
    gdt::{GlobalDescriptorTable, SegmentDescriptor, SegmentSelector},
    handler_with_error_code, handler_without_error_code,
    idt::InterruptDescriptorTable,
    instructions::int3,
    interrupts::{ExceptionStackFrame, PageFaultErrorCode},
    memory::{Address, VirtualAddress},
    pop_scratch_registers, println, push_scratch_registers,
    register::{CS, SS},
    tss::{TaskStateSegment, DOUBLE_FAULT_IST_IDX},
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
                .set_handler_function(handler_with_error_code!(double_fault_handler))
                .set_interrupt_stack_index(DOUBLE_FAULT_IST_IDX as u16);
        }

        idt
    };
}

lazy_static! {
    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_IDX] = {
            const STACK_SIZE: usize = 4096 * 5;
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

            let stack_start = VirtualAddress::from_ptr(unsafe { &STACK });
            let stack_end = stack_start + STACK_SIZE;

            stack_end
        };

        tss
    };
}

lazy_static! {
    static ref GDT: (
        GlobalDescriptorTable,
        SegmentSelector,
        SegmentSelector,
        SegmentSelector
    ) = {
        let mut gdt = GlobalDescriptorTable::new();
        let tss_selector = gdt.add_entry(SegmentDescriptor::new_tss_segment(&TSS));
        let kernel_code_selector = gdt.add_entry(SegmentDescriptor::kernel_code_segment());
        let kernel_data_selector = gdt.add_entry(SegmentDescriptor::kernel_data_segment());
        (
            gdt,
            tss_selector,
            kernel_code_selector,
            kernel_data_selector,
        )
    };
}

pub fn init() {
    // load the gdt
    GDT.0.load();
    unsafe {
        // update cs and ss segment registers as they have to point to same selectors
        CS::write(GDT.2);
        SS::write(GDT.3);
        // load the tss selector into the task register
        TaskStateSegment::load(GDT.1);
    }

    IDT.load();
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
