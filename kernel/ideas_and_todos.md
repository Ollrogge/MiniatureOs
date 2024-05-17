
### Links
- https://blog.lenot.re/a/mapping-consistency
    - virtual memory theory, fallible operations


### Memory allocations
- assume that they never fail, else you need to implement fallible allocation stuff which makes everything hard
    - Rusts view: Many collection methods may decide to allocate (push, insert, extend, entry, reserve, with_capacity, …) and those allocations may fail. Early on in Rust’s history we made a policy decision not to expose this fact at the API level, preferring to abort. This is because most developers aren’t prepared to handle it, or interested. Handling allocation failure haphazardly is likely to lead to many never-tested code paths and therefore bugs. We call this approach infallible collection allocation, because the developer model is that allocations just don’t fail.

### Lazy memory allocations (copy on write)
- when a page of memory is mapped with write permission and zero-initialised, in reality, the kernel maps the virtual memory to a “default” physical page that contains all zeros and is shared between all pages that have not yet been written to.
- when the first write from the process occurs on the page, it triggers an exception (as the page is mapped to read-only) which is caught by the kernel. The kernel then allocates a physical page, remaps the virtual memory with that page so that the kernel can write on it, then resumes execution at the instruction that caused the exception.
- feature also works when forking a process: During the fork operation, the virtual memories of both processes are remapped so that they point to the same physical page, but are both read-only. Then, when either process tries to write on the page, the kernel allocates a new physical page, copies the data, then remaps the memory to allow writing before resuming.
    - requires OOM killer since processes were lied to and assume they have the memory already


## Todos

**Println**
+ use a different println in kernel and tests than the one exported by x86_64 crate. Defining it there is just a dirty hack to get println debugging working for this code

+ use reserved physicalmemory region type only for regions used by BIOS. Else use sth like allocated