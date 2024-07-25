use crate::memory::{Address, VirtualAddress};
use core::arch::asm;

pub fn int3() {
    unsafe {
        asm!("int3", options(nomem, nostack));
    }
}

/// Invalidates any translation lookaside buffer (TLB) entries specified with the source operand.
/// The source operand is a memory address. The processor determines the page
/// that contains that address and flushes all TLB entries for that page.
pub fn flush_tlb(address: VirtualAddress) {
    unsafe {
        asm!("invlpg [{0}]", in(reg) address.as_u64() as usize, options(nostack, preserves_flags))
    }
}

pub fn hlt() {
    unsafe { asm!("hlt", options(nostack, nomem, preserves_flags)) }
}

#[cfg(target_arch = "x86_64")]
pub fn rdtsc() -> u64 {
    use core::arch::x86_64::_rdtsc;
    unsafe { _rdtsc() }
}
