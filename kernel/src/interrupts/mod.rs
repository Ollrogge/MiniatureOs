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
    interrupts::{self, ExceptionStackFrame, PageFaultErrorCode},
    memory::{Address, PageSize, Size4KiB, VirtualAddress},
    mutex::Mutex,
    pop_scratch_registers, print, println, push_scratch_registers,
    register::{CS, DS, ES, SS},
    tss::{TaskStateSegment, DOUBLE_FAULT_IST_IDX},
};

mod hardware;
use hardware::pic8259::ChainedPics;
pub const MASTER_PIC_OFFSET: u8 = 0x20;
pub const SLAVE_PIC_OFFSET: u8 = MASTER_PIC_OFFSET + 8;
static PICS: Mutex<ChainedPics> = Mutex::new(ChainedPics::new());

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = 0,
}

impl InterruptIndex {
    fn as_u8(self) -> u8 {
        self as u8
    }

    fn as_usize(self) -> usize {
        usize::from(self.as_u8())
    }

    fn as_remapped_idt_number(self) -> u8 {
        self.as_u8() + MASTER_PIC_OFFSET as u8
    }
}

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::default();

        unsafe {
            idt.divide_error
                .set_handler_function(handler_without_error_code!(divide_by_zero_handler));

            idt.debug
                .set_handler_function(handler_without_error_code!(debug_handler));

            idt.non_maskable_interrupt
                .set_handler_function(handler_without_error_code!(non_maskable_interrupt));

            idt.breakpoint
                .set_handler_function(handler_without_error_code!(breakpoint_handler));

            idt.invalid_opcode
                .set_handler_function(handler_without_error_code!(invalid_opcode_handler));

            idt.device_not_available
                .set_handler_function(handler_without_error_code!(device_not_available_handler));

            idt.invalid_tss
                .set_handler_function(handler_with_error_code!(invalid_tss_handler));

            idt.segment_not_present
                .set_handler_function(handler_with_error_code!(segment_not_present_handler));

            idt.stack_segment_fault
                .set_handler_function(handler_with_error_code!(stack_segment_fault_handler));

            idt.page_fault
                .set_handler_function(handler_with_error_code!(page_fault_handler));

            idt.alignment_check
                .set_handler_function(handler_with_error_code!(alignment_check_handler));

            idt.double_fault
                .set_handler_function(handler_with_error_code!(double_fault_handler))
                .set_interrupt_stack_index(DOUBLE_FAULT_IST_IDX as u16);

            idt.interrupts[InterruptIndex::Timer.as_usize()]
                .set_handler_function(handler_without_error_code!(timer_interrupt_handler));
        }

        idt
    };
}

lazy_static! {
    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_IDX] = {
            const STACK_SIZE: usize = Size4KiB::SIZE as usize * 5;
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
        // 0x8
        let tss_selector = gdt.add_entry(SegmentDescriptor::new_tss_segment(&TSS));
        // 0x18
        let kernel_code_selector = gdt.add_entry(SegmentDescriptor::kernel_code_segment());
        // 0x20
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
        // update cs and ss segment registers
        CS::write(GDT.2);
        DS::write(GDT.3);
        ES::write(GDT.3);
        SS::write(GDT.3);
        // load the tss selector into the task register
        TaskStateSegment::load(GDT.1);
    }

    IDT.load();

    // initialize & remap pic
    PICS.lock().init(MASTER_PIC_OFFSET, SLAVE_PIC_OFFSET);
    //PIC.lock().remap_pic();
    unsafe { interrupts::enable() };
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

extern "C" fn segment_not_present_handler(frame: &ExceptionStackFrame, error_code: u64) -> ! {
    println!(
        "General protection fault handler \n error_code: {:?} \n exception frame: {:?}",
        error_code, frame
    );
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

extern "C" fn invalid_tss_handler(frame: &ExceptionStackFrame, error_code: u64) -> ! {
    println!("Invalid tss handler: {:?}", frame);
    loop {}
}

extern "C" fn stack_segment_fault_handler(frame: &ExceptionStackFrame, error_code: u64) -> ! {
    println!("Stack segment handler: {:?}", frame);
    loop {}
}

extern "C" fn breakpoint_handler(frame: &ExceptionStackFrame) {
    println!("Int3 triggered: {:?}", frame);
}

extern "C" fn non_maskable_interrupt(frame: &ExceptionStackFrame) {
    println!("Non maskable interrupt handler {:?}", frame);
}

extern "C" fn debug_handler(frame: &ExceptionStackFrame) {
    println!("Debug handler {:?}", frame);
}

extern "C" fn device_not_available_handler(frame: &ExceptionStackFrame) {
    println!("Device not available handler {:?}", frame);
}

// double fault acts kind of like a catch-all block
// “double fault exception can occur when a second exception occurs during the
// handling of a prior (first) exception handler”. The “can” is important:
// Only very specific combinations of exceptions lead to a double fault
// https://os.phil-opp.com/double-fault-exceptions/
// (A double fault will always generate an error code with a value of zero. )
extern "C" fn double_fault_handler(frame: &ExceptionStackFrame, _error_code: u64) -> ! {
    println!("Double fault error code: {}", _error_code);
    println!("Double fault handler: {:?}", frame);
    loop {}
}

extern "C" fn timer_interrupt_handler(_frame: &ExceptionStackFrame) {
    print!(".");
    PICS.lock()
        .notify_end_of_interrupt(InterruptIndex::Timer.as_remapped_idt_number());
}
