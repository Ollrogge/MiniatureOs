use core::arch::asm;

/// Disables CPU interrupts.
///
/// # Safety
///
/// This function is unsafe because it directly manipulates the CPU state. The caller must ensure
/// that disabling interrupts does not lead to deadlocks or race conditions in the code.
pub unsafe fn disable() {
    unsafe { asm!("cli", options(nostack, nomem, preserves_flags)) }
}

/// Enables CPU interrupts.
///
/// # Safety
///
/// This function is unsafe because it directly manipulates the CPU state. The caller must ensure
/// that re-enabling interrupts is safe and does not introduce race conditions with other threads
/// or interrupt handlers.
pub unsafe fn enable() {
    unsafe { asm!("sti", options(nostack, nomem, preserves_flags)) }
}
