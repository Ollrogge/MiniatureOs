#!/bin/bash

qemu-system-x86_64 \
    -drive format=raw,file=target/disk_image.bin \
    -no-reboot \
    -monitor /dev/null \