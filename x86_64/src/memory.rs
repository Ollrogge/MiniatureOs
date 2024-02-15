use core::clone::Clone;
use core::fmt::{Formatter, LowerHex, Result};
use core::marker::PhantomData;
use core::ops::{Add, AddAssign};

pub trait PageSize {
    const SIZE: u64;
}

#[derive(Clone, Copy)]
pub enum Size4KiB {}

impl PageSize for Size4KiB {
    const SIZE: u64 = 0x1000;
}

pub trait Address {
    fn value(&self) -> u64;
}

#[derive(Copy, Clone, Debug)]
pub struct PhysicalAddress(pub u64);

impl PhysicalAddress {
    pub fn new(address: u64) -> Self {
        Self(address)
    }

    pub fn align_down(&self, align: u64) -> PhysicalAddress {
        let addr = self.0 & (align - 1);
        PhysicalAddress(addr)
    }

    pub fn align_up(&self, align: u64) -> PhysicalAddress {
        let addr = (self.0 + align - 1) & !(align - 1);
        PhysicalAddress(addr)
    }

    pub fn inner(&self) -> u64 {
        self.0
    }
}

impl Address for PhysicalAddress {
    fn value(&self) -> u64 {
        self.0
    }
}

impl Add<u64> for PhysicalAddress {
    type Output = Self;
    fn add(self, rhs: u64) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl AddAssign<u64> for PhysicalAddress {
    fn add_assign(&mut self, rhs: u64) {
        self.0 += rhs;
    }
}

impl LowerHex for PhysicalAddress {
    fn fmt(&self, f: &mut Formatter) -> Result {
        LowerHex::fmt(&self.0, f)
    }
}

pub struct VirtualAddress(u64);

pub const PAGE_SIZE: usize = 0x1000;

#[derive(Copy, Clone)]
pub struct PhysicalFrame<S: PageSize = Size4KiB> {
    pub address: PhysicalAddress,
    pub size: PhantomData<S>,
}

impl<S: PageSize> PhysicalFrame<S> {
    pub fn at_address(address: PhysicalAddress) -> Self {
        Self {
            address: address.align_down(PAGE_SIZE as u64),
            size: PhantomData,
        }
    }
}

impl<S: PageSize> Add<u64> for PhysicalFrame<S> {
    type Output = Self;
    fn add(self, rhs: u64) -> Self::Output {
        Self {
            address: self.address + S::SIZE * rhs,
            size: PhantomData,
        }
    }
}

impl<S: PageSize> AddAssign<u64> for PhysicalFrame<S> {
    fn add_assign(&mut self, rhs: u64) {
        self.address += S::SIZE * rhs;
    }
}

pub trait MemoryRegion {
    fn start(&self) -> u64;
    fn end(&self) -> u64;
    fn length(&self) -> u64;
    fn contains(&self, start: u64) -> bool;
}
