# http://www.cs.cmu.edu/~410-s07/p4/p4-boot.pdf
# read stage1 from disk, load it into memory and execute it

.section .boot, "awx"
.global _start
.code16

_start:
    # zero segment registers
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov fs, ax
    mov gs, ax

    # clear the direction flag (e.g. go forward in memory when using
    # instructions like lodsb)
    cld

    # initialize stack
    mov sp, 0x7c00

# https://wiki.osdev.org/A20_Line
# need to enable A20 (address line 20) line to access more than 1MB of memory
# represents 21st bit of any memory access
enable_a20:
    in al, 0x92
    test al, 0x2
    jnz enable_a20_after
    or al, 0x2
    and al, 0xFE
    out 0x92, al
enable_a20_after:

; https://wiki.osdev.org/Disk_access_using_the_BIOS_(INT_13h)
check_int13h:
    push 1
    mov ah, 0x41
    mov bx, 0x55AA
    int 0x13

    jc fail
    pop ax

rust:
    push dx # drive number
    call first_stage







