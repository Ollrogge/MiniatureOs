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

### Features
- [X] BIOS bootloader
