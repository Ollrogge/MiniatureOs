use crate::println;
use lazy_static::lazy_static;
use x86_64::idt::InterruptDescriptorTable;

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::default();

        idt.divide_error.set_handler_fn(divide_by_zero_handler);
        idt.breakpoint.set_handler_fn(breakpoint_handler);

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

extern "C" fn divide_by_zero_handler() -> ! {
    println!("Exception: divide by zero");
    loop {}
}

extern "C" fn breakpoint_handler() -> ! {
    println!("Int3 triggered");
    loop {}
}
