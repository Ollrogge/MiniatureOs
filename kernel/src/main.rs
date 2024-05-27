#![no_std]
#![no_main]
#![feature(naked_functions)]
use api::{BootInfo, PhysicalMemoryRegions};
use core::{arch::asm, panic::PanicInfo};
use kernel::{kernel_init, memory::buddy_frame_allocator::BuddyFrameAllocator};
use x86_64::{
    instructions::int3,
    memory::{MemoryRegion, PhysicalMemoryRegion},
    println,
    register::Cr0,
};

#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    println!("Kernel PANIC: {}", info);
    loop {}
}

#[no_mangle]
#[link_section = ".start"]
pub extern "C" fn _start(info: &'static BootInfo) -> ! {
    start(info);
}

fn print_memory_map(map: &PhysicalMemoryRegions) {
    for region in map.iter() {
        println!(
            "Memory region, start: {:#x}, length: {:#x}, usable: {}",
            region.start,
            region.size,
            region.is_usable()
        );
    }
}

fn trigger_int3() {
    int3();
}

fn trigger_invalid_opcode() {
    unsafe {
        asm!("ud2");
    }
}

fn trigger_divide_by_zero() {
    unsafe {
        asm!("mov rax, {0:r}", "mov rcx, {1:r}", "div rcx", in(reg) 4, in(reg) 0);
    }
}

// should cause a pagefault because guard page is hit
fn stack_overflow() {
    stack_overflow()
}

// using *mut u64 here causes an infinite loop since address is not 8 byte aligned
// todo: this is weird ?, can cause infinite loops at other places ?
fn trigger_page_fault() {
    unsafe { *(0xdeabeef as *mut u8) = 42 };
}

// TODO: put this into the test_kernel
// TODO: write proper tests
fn test_buddy_allocator(allocator: &mut BuddyFrameAllocator) {
    // alloc some chunks to make sure we have buddies
    // (as there might be single 0x100 chunks at the beginning)
    let mut last_start = allocator.alloc(0x100).unwrap().start();
    loop {
        let c = allocator.alloc(0x100).unwrap();

        // the higher buddy is returned first
        if c.start() + 0x100 == last_start {
            break;
        }

        println!("Test: {:#x} {:#x}", c.start(), last_start);

        last_start = c.start();
    }
    // Test easy merge
    let c1 = allocator.alloc(0x100).unwrap();
    let c2 = allocator.alloc(0x100).unwrap();

    let addr = u64::min(c1.start(), c2.start());

    allocator.dealloc(c1);
    allocator.dealloc(c2);

    let c3 = allocator.alloc(0x200).unwrap();
    println!(
        "Test: c3: {:#x} addr:{:#x}, buddies ?: c1:{:#x} c2:{:#x}",
        c3.start(),
        addr,
        c1.start(),
        c2.start()
    );

    assert!(c3.start() == addr);

    let addr = c3.start();
    allocator.dealloc(c3);

    // Test multistage merge

    // c1 and c2 should be created from the c3 we just deallocated
    let c1 = allocator.alloc(0x100).unwrap();
    let c2 = allocator.alloc(0x100).unwrap();

    assert!(u64::min(c1.start(), c2.start()) == addr);

    let c3 = allocator.alloc(0x200).unwrap();
    let addr = u64::min(c3.start(), u64::min(c1.start(), c2.start()));

    // merge 2* 0x100 into 0x200
    allocator.dealloc(c1);
    allocator.dealloc(c2);
    // merge c3 with the 0x200 chunk created by deallocing c1 and c2
    allocator.dealloc(c3);

    let c4 = allocator.alloc(0x200).unwrap();

    println!("Test: {:#x} {:#x}", c4.start(), addr);

    assert!(c4.start() == addr);
}

fn start(info: &'static BootInfo) -> ! {
    println!("Hello from kernel <3");

    print_memory_map(&info.memory_regions);

    kernel_init(info).unwrap();

    println!("Interrupts initialized");

    let mut allocator = BuddyFrameAllocator::new();
    allocator.init(info.memory_regions.into_iter().cloned());
    println!("Buddy allocator initialized");

    test_buddy_allocator(&mut allocator);

    // invalid opcode
    /*
     */
    trigger_int3();
    trigger_page_fault();

    println!("Did not crash, successfully returned from int3");

    //stack_overflow();

    loop {}
}
