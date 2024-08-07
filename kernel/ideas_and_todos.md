
### Links
- https://blog.lenot.re/a/mapping-consistency
    - virtual memory theory, fallible operations

### Lazy memory allocations (copy on write)
- when a page of memory is mapped with write permission and zero-initialised, in reality, the kernel maps the virtual memory to a “default” physical page that contains all zeros and is shared between all pages that have not yet been written to.
- when the first write from the process occurs on the page, it triggers an exception (as the page is mapped to read-only) which is caught by the kernel. The kernel then allocates a physical page, remaps the virtual memory with that page so that the kernel can write on it, then resumes execution at the instruction that caused the exception.
- feature also works when forking a process: During the fork operation, the virtual memories of both processes are remapped so that they point to the same physical page, but are both read-only. Then, when either process tries to write on the page, the kernel allocates a new physical page, copies the data, then remaps the memory to allow writing before resuming.
    - requires OOM killer since processes were lied to and assume they have the memory already

### Initramfs
+ temporary root file system which the OS uses as a filesystem containing stuff necessary for it to boot
+ solution to chicken-or-egg problem at boot time: the kernel needs to load its modules from disk, but these modules include the driver that normally allows the kernel to access that disk

### Timer
+ APIC timer
    + local timer hardwired to each cpu core
    + good for multiprocessor systems
    + harder to implement since it oscillates at the individual CPU's frequencies
    + higher precision
    + better but harder to implement
    + https://wiki.osdev.org/APIC_Timer

## Next steps
+ copy on write stack allocation
+ do the executor tutorial of the blog os series
+ implement todo datastructures, clean up code
+ implement support for APIC
+ ramdisk
+ virtual filesystem using ext2
+ elf loader user programs
+ userspace -> init elf

## Todos

**Logging**
+ add proper logging

**Sizes**
+ Have Size4Kib::SIZE actually be a usize and not u64

**Memory regions**
+ better differentiate between BIOS firmware memory regions, bootloaders ones, kernel ones...

**Physical and Virtual address**
+ Generate operations on Addresses (e.g. add / sub...) using macros ?

**Println**
+ use a different println in kernel and tests than the one exported by x86_64 crate. Defining it there is just a dirty hack to get println debugging working for this code

+ use reserved physicalmemory region type only for regions used by BIOS. Else use sth like allocated

+ write a logger that can be enabled per module, similar to RIOTS DEBUG macro

+ implement the MapperFlush functionality also in bootloader, to be forced to flush tlb later

**Guard page**
+ check if implementation correct
+ don't need to map a guard page without any option (unmapped basically same)
+ just need to make sure it isnt allocated anymore


**Physicalframe**
+ why is it generic over PageSize ? there can only be frames
with a size of 4KiB

**Error handling**
+ impove error handling
    + e.g. paging
+ don't just always use except. Pass errors in a smarter way, print errors where they originate
+ allocators

**Threads / Processes**
+ implement a way to start kernel threads
+ implement processes only for user space / kernel modules
+ https://wiki.osdev.org/Brendan%27s_Multi-tasking_Tutorial
+ https://wiki.osdev.org/Context_Switching
+ Save Context: Push registers onto the stack or save them in the thread's context structure.
+ Switch Stack: Change the stack pointer (ESP) to the new thread's stack.
+ Restore Context: Pop registers from the new thread's stack or restore them from the thread's context structure.
+ Return: Use iret to restore the instruction pointer (EIP) and continue execution.

**Filesystem**
+  basic ext2 implementation using a node graph.
+ serenity ext2 tour: https://www.youtube.com/watch?v=vHRd9QRYQBA

**Datastructures**
+ OnceCell
 + A cell which can be written to only once