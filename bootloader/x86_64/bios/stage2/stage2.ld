ENTRY(_start)

SECTIONS {
    /* starts directly after mbr */
    . = 0x7c00 + 512;

    .start : {
        *(.start)
    }
    .text : {
        *(.text .text.*)
    }
    .bss : {
        *(.bss .bss.*)
    }
    .rodata : {
        *(.rodata .rodata.*)
    }
    .data : {
        *(.data .data.*)
    }
    .eh_frame : {
        *(.eh_frame .eh_frame.*)
    }
    .eh_frame_hdr : {
        *(.eh_frame_hdr .eh_frame_hdr.*)
    }

    . = ALIGN(512);

    _second_stage_end = .;

    /* limit to 512kB boundary because follow region is probably reserved ? */
    . = 0x0007FFFF - 2;
    .end_marker :
    {
        SHORT(0xdead)
    }
}
