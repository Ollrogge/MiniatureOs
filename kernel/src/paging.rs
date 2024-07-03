use api::BootInfo;
use x86_64::{
    memory::{Address, VirtualAddress},
    paging::PageTable,
    register::Cr3,
};

pub unsafe fn init(bios_info: &'static BootInfo) -> &'static mut PageTable {
    let (plm4t, _) = Cr3::read();

    let virtual_base = VirtualAddress::new(plm4t.start() + bios_info.physical_memory_offset);
    let page_table_ptr: *mut PageTable = virtual_base.as_mut_ptr();
    &mut *page_table_ptr
}
