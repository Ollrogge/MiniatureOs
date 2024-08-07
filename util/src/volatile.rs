use core::ptr;

pub struct Volatile<T: Copy>(T);

impl<T: Copy> Volatile<T> {
    pub fn read(&self) -> T {
        unsafe { ptr::read_volatile(&self.0) }
    }

    pub fn write(&mut self, val: T) {
        unsafe {
            ptr::write_volatile(&mut self.0, val);
        }
    }
}
