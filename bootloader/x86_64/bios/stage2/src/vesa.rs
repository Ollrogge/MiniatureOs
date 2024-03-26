//! This module implements functionality for VESA BIOS Extension (VBE).
//! Mainly this includes querying and setting display modes.
//! https://wiki.osdev.org/VESA_Video_Modes
//! http://www.petesqbsite.com/sections/tutorials/tuts/vbe3.pdf
//! specification for standard software access to graphics display controllers
//! which support resolutions, color depths, and frame buffer organizations
//! beyond the VGA hardware standard
use crate::println;
use api::{FramebufferInfo, PixelFormat};
use common::{const_assert, realmode::RealModePointer};
use core::{arch::asm, borrow::BorrowMut, default::Default, mem::size_of};
use x86_64::memory::{PhysicalMemoryRegion, Region};

/// All VESA functions return 0x4F in AL if they are supported and use AH as a
/// status flag, with 0x00 being success. This means that you should check that
/// AX is 0x004F after each VESA call to see if it succeeded.
const VESA_SUCCESS: u16 = 0x004f;

/// Display controller info
#[derive(Debug)]
#[repr(C, packed)]
#[allow(dead_code)]
pub struct VbeInfo {
    signature: [u8; 4], // should be "VESA"
    version: u16,       // should be 0x0300 for VBE 3.0
    oem_string_ptr: RealModePointer,
    capabilities: u32,
    /// Pointer to an array of video mode ids which can be used to query information
    /// about a mode
    video_mode_array_pointer: RealModePointer,
    total_memory: u16, // number of 64KB blocks
    reserved: [u8; 512 - 0x14],
}
const_assert!(size_of::<VbeInfo>() == 512, "VbeInfoBlock size");

impl Default for VbeInfo {
    fn default() -> VbeInfo {
        VbeInfo {
            signature: [0; 4],
            version: 0,
            oem_string_ptr: RealModePointer(0),
            capabilities: 0,
            video_mode_array_pointer: RealModePointer(0),
            total_memory: 0,
            reserved: [0; 512 - 0x14],
        }
    }
}

impl VbeInfo {
    /// Gets the VbeInfo
    pub fn get() -> Result<Self, u16> {
        const GET_CONTROLLER_INFO_CMD: u16 = 0x4f00;
        let mut obj = Self::default();
        let ret;
        unsafe {
            asm!("push es", "int 0x10", "pop es", inout("ax") GET_CONTROLLER_INFO_CMD => ret, in("di") &mut obj);
        }

        match ret {
            VESA_SUCCESS => Ok(obj),
            _ => Err(ret),
        }
    }

    /// Returns the video mode ID located at a specified `offset` from the
    /// beginning of the video mode array.
    ///
    /// The video mode ID is a 16-bit value uniquely identifying a video mode
    /// supported by the system.
    ///
    /// # Safety
    /// This function is `unsafe` because it performs unchecked pointer arithmetic
    /// based on the provided `offset`. Accessing beyond the end of the video mode
    /// array could lead to undefined behavior.
    unsafe fn get_mode(&self, offset: i32) -> Option<u16> {
        // variable required since else the access is unaligned as self.video_mode_ptr
        // is a member of a packed struct
        let video_mode_ptr = self.video_mode_array_pointer;
        let ptr = (((video_mode_ptr.segment() as u32) << 4) + video_mode_ptr.offset() as u32)
            as *const u16;

        let mode = unsafe { *ptr.offset(offset as isize) };

        if mode == 0xffff {
            return None;
        } else {
            Some(mode)
        }
    }

    /// Gets the display mode id of the mode closest to the specified parameters
    /// Code is basically copied from: https://wiki.osdev.org/VESA_Video_Modes
    pub fn get_best_mode(&self, width: u16, height: u16, depth: u8) -> Option<u16> {
        let mut best: Option<u16> = None;
        let mut best_pix_diff = u32::MAX;
        let mut best_depth_diff = u8::MAX;
        for i in 0.. {
            let mode = match unsafe { self.get_mode(i) } {
                Some(mode) => mode,
                None => break,
            };

            let info = match VbeModeInfo::get(mode) {
                Ok(info) => info,
                Err(c) => {
                    //println!("VesaModeInfo query failed with code: {:x}", c);
                    continue;
                }
            };

            // Check if this is a graphics mode with linear frame buffer support
            if info.attributes & 0x90 != 0x90 {
                continue;
            }

            // Check if this is a packed pixel or direct color mode
            if info.memory_model != 4 && info.memory_model != 6 {
                continue;
            }

            if info.width == width && info.height == height && info.bits_per_pixel == depth {
                return Some(mode);
            }

            let pix_diff =
                (info.width as u32 * info.height as u32).abs_diff(width as u32 * height as u32);
            let depth_diff = info.bits_per_pixel.abs_diff(depth);
            if best_pix_diff > pix_diff || best_pix_diff == pix_diff && best_depth_diff > depth_diff
            {
                best = Some(mode);
                best_depth_diff = depth_diff;
                best_pix_diff = pix_diff;
            }
        }

        best
    }

    pub fn set_mode(&self, mode: u16) -> Result<(), u16> {
        const SET_VIDEO_MODE_CMD: u16 = 0x4f02;
        let mut mode = mode;
        // bit 14 is the LFB bit: when set, it enables the linear framebuffer,
        //when clear, software must use bank switching
        mode |= 1 << 14;

        // Bit 15 is the DM bit: when set, the BIOS doesn't clear the screen.
        // Bit 15 is usually ignored and should always be cleared.
        mode &= !(1 << 15);

        let mut ret: u16;
        unsafe {
            asm!("push es", "int 0x10", "pop es", inout("ax") SET_VIDEO_MODE_CMD => ret, in("bx") mode, options(nomem));
        }

        match ret {
            VESA_SUCCESS => Ok(()),
            _ => Err(ret),
        }
    }
}

/// Vbe mode information block
/// Contains information about a specific display mode
#[derive(Debug)]
#[repr(C)]
pub struct VbeModeInfo {
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
const_assert!(size_of::<VbeModeInfo>() == 256, "VbeModeInfo size");

impl Default for VbeModeInfo {
    fn default() -> VbeModeInfo {
        VbeModeInfo {
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

impl VbeModeInfo {
    pub fn get(mode: u16) -> Result<Self, u16> {
        const GET_MODE_INFO_CMD: u16 = 0x4f01;
        let mut obj = Self::default();
        let ptr = RealModePointer(&mut obj as *mut VbeModeInfo as u32);
        let mut ret: u16;
        unsafe {
            asm!("push es", "mov es, {:x}", "int 0x10", "pop es", in(reg) ptr.segment(), in("di") ptr.offset(), inout("ax") GET_MODE_INFO_CMD => ret, in("cx") mode);
        }

        match ret {
            VESA_SUCCESS => Ok(obj),
            _ => Err(ret),
        }
    }

    pub fn get_pixel_format(&self) -> PixelFormat {
        match (self.red_position, self.green_position, self.blue_position) {
            (0, 8, 16) => PixelFormat::Rgb,
            (16, 8, 0) => PixelFormat::Bgr,
            (red_position, green_position, blue_position) => PixelFormat::Unknown {
                red_position,
                green_position,
                blue_position,
            },
        }
    }

    pub fn to_framebuffer_info(&self) -> FramebufferInfo {
        let bytes_per_pixel = self.bits_per_pixel / 8;
        let region = PhysicalMemoryRegion::new(
            self.framebuffer.into(),
            u64::from(self.height) * u64::from(self.bytes_per_scanline),
        );
        let stride = self.bytes_per_scanline / u16::from(bytes_per_pixel);

        FramebufferInfo::new(
            region,
            self.width,
            self.height,
            bytes_per_pixel,
            stride,
            self.get_pixel_format(),
        )
    }
}
