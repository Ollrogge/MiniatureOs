#!/bin/bash

cargo +nightly build --release -Zbuild-std=core --target x86-mbr.json -Zbuild-std-features=compiler-builtins-mem

objcopy -I elf32-i386 -O binary ../../../target/x86-mbr/release/mbr ../../../target/disk_image.bin
