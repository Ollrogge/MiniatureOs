<div align="center">

# MiniatureOS
Miniature hobby bootloader & POSIX-compatible kernel to teach me the basic concepts of OS development and different processor architectures.

</div>

### Build
**Add nightly toolchain**
```bash
rustup target add x86_64-unknown-none
```

**Configure cargo**
Add the following to `.cargo/config.toml`:
```toml
 [unstable]
  # enable the unstable artifact-dependencies feature, see
  # https://doc.rust-lang.org/nightly/cargo/reference/unstable.html#artifact-dependencies
  bindeps = true
```

### Notes
- This is a very early work in progress which mainly consists of only a bootloader at the moment.

### Features
- [X] BIOS bootloader
- [X] Heap allocator
    - Bump / Linkedlist frame allocator
    - Buddy heap allocator

### Goals
Following are the long-term goals of this project:
- 0 external dependencies at some point
- POSIX compatible
- Support for x86_64 & aarch64 & riscv64