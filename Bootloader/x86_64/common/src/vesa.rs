use crate::const_assert;
use crate::println;
use core::{arch::asm, default::Default, mem::size_of};

#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct RealModePtr(u32);

impl RealModePtr {
    pub fn segment(&self) -> u16 {
        (self.0 >> 16) as u16
    }

    pub fn offset(&self) -> u16 {
        self.0 as u16
    }
}

#[derive(Debug)]
#[repr(C, packed)]
#[allow(dead_code)]
pub struct VesaInfo {
    signature: [u8; 4], // should be "VESA"
    version: u16,       // should be 0x0300 for VBE 3.0
    oem_string_ptr: RealModePtr,
    capabilities: u32,
    video_mode_ptr: RealModePtr,
    total_memory: u16, // number of 64KB blocks
    reserved: [u8; 512 - 0x14],
}
const_assert!(size_of::<VesaInfo>() == 512, "VbeInfoBlock size");

impl Default for VesaInfo {
    fn default() -> VesaInfo {
        VesaInfo {
            signature: [0; 4],
            version: 0,
            oem_string_ptr: RealModePtr(0),
            capabilities: 0,
            video_mode_ptr: RealModePtr(0),
            total_memory: 0,
            reserved: [0; 512 - 0x14],
        }
    }
}

impl VesaInfo {
    pub fn query(&mut self) -> Result<(), u16> {
        let mut ret = 0x0;
        unsafe {
            asm!("int 0x10", inout("ax") 0x4f00u16 => ret, in("di") self, options(nostack, nomem));
        }

        match ret {
            0x004f => Ok(()),
            _ => Err(ret),
        }
    }

    fn get_mode(&mut self, offset: i32) -> Option<u16> {
        /*
                error[E0793]: reference to packed field is unaligned
          --> Bootloader/x86_64/common/src/vesa.rs:61:38
           |
        61 |         let video_mode_ptr_segment = self.video_mode_ptr.segment() as u32;
           |                                      ^^^^^^^^^^^^^^^^^^^
           |
           = note: packed structs are only aligned by one byte, and many modern architectures penalize unaligned field accesses
           = note: creating a misaligned reference is undefined behavior (even if that reference is never dereferenced)
           = help: copy the field contents to a local variable, or replace the reference with a raw pointer and use `read_unaligned`/`write_unaligned` (loads and stores via `*p` must be properly aligned even when using raw pointers)
        */
        // variable required since else the access is unaligned as self.video_mode_ptr
        // is a member of a packed struct
        let video_mode_ptr = self.video_mode_ptr;
        let ptr = (((video_mode_ptr.segment() as u32) << 4) + video_mode_ptr.offset() as u32)
            as *const u16;

        let mode = unsafe { *ptr.offset(offset as isize) };

        if mode == 0xffff {
            return None;
        } else {
            Some(mode)
        }
    }

    pub fn get_best_mode(&mut self, max_width: u16, max_height: u16) {
        for i in 0.. {
            let mode = match self.get_mode(i) {
                Some(mode) => mode,
                None => break,
            };

            println!("Mode: {:#x} ", mode);
        }
    }
}

#[derive(Debug)]
#[repr(C)]
struct VesaModeInfo {
    attributes: u16,
    window_a: u8,
    window_b: u8,
    granularity: u16,
    window_size: u16,
    segment_a: u16,
    segment_b: u16,
    window_function_ptr: u32,
    bytes_per_scanline: u16,
    width: u16,
    height: u16,
    w_char: u8,
    y_char: u8,
    planes: u8,
    bits_per_pixel: u8,
    banks: u8,
    memory_model: u8,
    bank_size: u8,
    image_pages: u8,
    reserved_0: u8,
    red_mask: u8,
    red_position: u8,
    green_mask: u8,
    green_position: u8,
    blue_mask: u8,
    blue_position: u8,
    reserved_mask: u8,
    reserved_position: u8,
    direct_color_attributes: u8,
    framebuffer: u32,
    off_screen_memory_offset: u32,
    off_screen_memory_size: u16,
    reserved: [u8; 206],
}
const_assert!(size_of::<VesaModeInfo>() == 256, "VbeModeInfo size");

impl Default for VesaModeInfo {
    fn default() -> VesaModeInfo {
        VesaModeInfo {
            attributes: 0,
            window_a: 0,
            window_b: 0,
            granularity: 0,
            window_size: 0,
            segment_a: 0,
            segment_b: 0,
            window_function_ptr: 0,
            bytes_per_scanline: 0,
            width: 0,
            height: 0,
            w_char: 0,
            y_char: 0,
            planes: 0,
            bits_per_pixel: 0,
            banks: 0,
            memory_model: 0,
            bank_size: 0,
            image_pages: 0,
            reserved_0: 0,
            red_mask: 0,
            red_position: 0,
            green_mask: 0,
            green_position: 0,
            blue_mask: 0,
            blue_position: 0,
            reserved_mask: 0,
            reserved_position: 0,
            direct_color_attributes: 0,
            framebuffer: 0,
            off_screen_memory_offset: 0,
            off_screen_memory_size: 0,
            reserved: [0; 206],
        }
    }
}

impl VesaModeInfo {
    pub fn query(&mut self, mode: u16) -> Result<(), u16> {
        let ptr = RealModePtr(self as *mut VesaModeInfo as u32);
        let mut ret: u16;
        unsafe {
            asm!("mov es, {:x}", "int 0x10", in(reg) ptr.segment(), inout("ax") 0x4f01u16 => ret, in("cx") mode, in("di") ptr.offset(), options(nostack, nomem));
        }

        if ret != 0x4f {
            Err(ret)
        } else {
            Ok(())
        }
    }
}
