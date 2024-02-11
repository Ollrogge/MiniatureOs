// TODO: dont feel the need for graphical output in a bootloader rn
use common::BiosFramebufferInfo;

pub struct FrameBufferWriter {
    framebuffer: &'static mut [u8],
    info: BiosFramebufferInfo,
    x_pos: usize,
    y_pos: usize,
}
