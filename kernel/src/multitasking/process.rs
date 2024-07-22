use super::{
    scheduler::Scheduler,
    thread::{Thread, ThreadEntryFunc, ThreadPriority},
};
use crate::{
    error::KernelError,
    memory::{
        address_space::AddressSpace,
        manager::{AllocationStrategy, MemoryManager},
        region::{RegionType, VirtualMemoryRegion},
        virtual_memory_object::MemoryBackedVirtualMemoryObject,
    },
    serial_println, GlobalData,
};
use alloc::{collections::BTreeMap, format, string::String, sync::Arc, vec::Vec};
use api::BootInfo;
use core::sync::atomic::{AtomicU64, Ordering::Relaxed};
use util::mutex::{Mutex, MutexGuard};
use x86_64::{
    memory::{PageAlignedSize, KIB},
    paging::{PageTableEntryFlags, Translator},
    register::Cr3,
};
/**
 *  https://www.youtube.com/watch?v=3xgOybGlYes&t=1090s
 *
 * The complete memory management is handled by the MemoryManager. It allocates
 * frames, handles page faults etc
 *
 * The kernel is one process. Therefore, an execution unit in the kernel space will always
 * be a kernel thread not a process.
 *
 * Each process has an associated address space. The address space manages the
 * page table and virtual memory allocations. The allocated virtual memory is
 * stored inside VirtualMemoryRegions.
 *
 * Each VirtualMemoryRegion is backed by a VirtualMemoryObject. This object
 * is either RAM backed or file backed.
 *
 * The VirtualMemoryObject is responsible for allocating physical memory for itself
 *
 *
 * AnonymousVMObject::try_create_with_size = lazy, allocate frame when pagefault
 * AnonymousVMObject::try_create_with_physical_pages => create pages
 *
 *
 *
 * The address space contains virtual
 * memory regions.
 *
 *
 *
 * userspace directory has copy of complete kernel space directory
 * kernel mapped into every process
 *
 *
 * Each process has a virtual memory manager
 * Each thread has a kernel and user stack.
 *  + User stack initialization should be done by whatever loads the executable
 *
 *
 *
 * Initial "colonel" process which runs the idle loop
 *  - only ever runs when there is nothing to do
 *  - has pid 0
 *
 * - finializer kernel process: tears down dead processes in zombie state
 *
 * all process list which is basically a linked_list of processes
 *
 * enable interrupts once multitasking is ready
 *
 *  Every Process has an AddressSpace.
    - An AddressSpace has a number of Region objects, each with a virtual base address, size, permission bits, etc.
    - Every Region has an underlying VMObject.

- VMObject is virtual and can be AnonymousVMObject (MAP_ANONYMOUS) or InodeVMObject (MAP_FILE).

- Cross-process memory sharing occurs when two or more Regions in separate AddressSpaces use the same underlying VMObject.

- MemoryManager handles physical page allocation, fault handling, page tables, etc.
 *
 */

static PROCESS_TREE: Mutex<ProcessTree> = Mutex::new(ProcessTree::new());
const DEFAULT_STACK_SIZE: PageAlignedSize = PageAlignedSize::new(32 * KIB as usize);

struct ProcessTree {
    inner: BTreeMap<ProcessId, Arc<Mutex<Process>>>,
}

impl ProcessTree {
    pub const fn new() -> Self {
        Self {
            inner: BTreeMap::new(),
        }
    }

    pub fn add_process(&mut self, id: ProcessId, process: Arc<Mutex<Process>>) {
        self.inner.insert(id, process);
    }

    pub fn lock() -> MutexGuard<'static, Self> {
        PROCESS_TREE.lock()
    }
}

#[derive(Clone, Copy, PartialEq, PartialOrd, Eq, Ord)]
pub struct ProcessId(u64);

impl ProcessId {
    pub fn new() -> Self {
        static IDS: AtomicU64 = AtomicU64::new(0);
        Self(IDS.fetch_add(1, Relaxed))
    }
}

pub struct Process {
    id: ProcessId,
    name: String,
    address_space: AddressSpace,
}

impl Process {
    pub fn new(name: String, cr3: u64) -> Self {
        Self {
            id: ProcessId::new(),
            name,
            address_space: AddressSpace::new(cr3, GlobalData::the().physical_memory_offset()),
        }
    }

    pub fn id(&self) -> ProcessId {
        self.id
    }

    pub fn current() -> Arc<Mutex<Process>> {
        unsafe { Scheduler::the().current_thread().process.clone() }
    }

    // TODO: rwlock
    pub fn address_space(&mut self) -> &mut AddressSpace {
        &mut self.address_space
    }
}

pub fn init(boot_info: &'static BootInfo) -> Result<(), KernelError> {
    let process = Arc::new(Mutex::new(Process::new(
        String::from("colonel"),
        Cr3::read_raw(),
    )));

    PROCESS_TREE
        .lock()
        .add_process(process.lock().id(), process.clone());

    let mut memory_manager = MemoryManager::the().lock();

    let mut kernel_stack_boot_frames = Vec::new();
    let page_table = memory_manager.kernel_page_table();
    // skip guard page
    for page in boot_info.kernel_stack.iter().skip(1) {
        let (frame, _) = page_table.translate(page)?;
        kernel_stack_boot_frames.push(frame);
    }

    let obj = MemoryBackedVirtualMemoryObject::new(kernel_stack_boot_frames);

    let thread_name = String::from("colonel_thread");
    let stack_name = format!("{}_stack", thread_name);

    memory_manager.region_tree().try_allocate_range_in_region(
        stack_name.clone(),
        RegionType::Stack,
        boot_info.kernel_stack.clone(),
    )?;

    let stack = VirtualMemoryRegion::new(
        boot_info.kernel_stack.clone(),
        stack_name,
        obj,
        RegionType::Stack,
    );

    let thread = Thread::colonel_thread(thread_name, process, stack);

    Scheduler::init(thread);

    Ok(())
}

fn try_create_stack_thread(
    name: String,
) -> Result<VirtualMemoryRegion<MemoryBackedVirtualMemoryObject>, KernelError> {
    MemoryManager::the()
        .lock()
        .allocate_kernel_region_with_size(
            DEFAULT_STACK_SIZE,
            name,
            RegionType::Stack,
            PageTableEntryFlags::WRITABLE
                | PageTableEntryFlags::PRESENT
                | PageTableEntryFlags::NO_EXECUTE,
            AllocationStrategy::AllocateNow,
        )
}

pub fn spawn_kernel_thread(
    name: String,
    func: ThreadEntryFunc,
    priority: ThreadPriority,
) -> Result<(), KernelError> {
    let cur_process = Process::current();
    let thread_stack = try_create_stack_thread(format!("{}_stack", name.clone()))?;
    let thread = Thread::new(name, cur_process, thread_stack, func, priority);

    unsafe { Scheduler::the().add_thread(thread) };

    Ok(())
}
