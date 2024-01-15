#!/bin/bash

qemu-system-x86_64 \
    -bios /usr/share/ovmf/x64/OVMF.fd \
    -drive format=raw,file=fat:rw:/home/h0ps/Programming/MiniatureOs/target/x86_64-unknown-uefi/debug \
    -no-reboot \
    -nographic \
    -monitor /dev/null \
    -enable-kvm