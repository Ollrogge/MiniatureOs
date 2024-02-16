use bit_field::BitField;
use core::clone::Clone;
use core::fmt::{Formatter, LowerHex, Result};
use core::marker::PhantomData;
use core::ops::{Add, AddAssign, Rem};

#[derive(Clone, Copy, Debug)]
pub struct Region {
    pub start: u64,
    pub size: u64,
}

impl Region {
    pub fn new(start: u64, len: u64) -> Region {
        Region { start, size: len }
    }
}

pub trait PageSize {
    const SIZE: u64;
}

#[derive(Clone, Copy)]
pub enum Size4KiB {}

impl PageSize for Size4KiB {
    const SIZE: u64 = 0x1000;
}

pub trait Address {
    fn as_u64(&self) -> u64;
}

#[derive(Copy, Clone, Debug)]
pub struct PhysicalAddress(pub u64);

impl PhysicalAddress {
    pub fn new(address: u64) -> Self {
        Self(address)
    }

    pub fn align_down(&self, align: u64) -> PhysicalAddress {
        let addr = self.0 & !(align - 1);
        PhysicalAddress(addr)
    }

    pub fn align_up(&self, align: u64) -> PhysicalAddress {
        let addr = (self.0 + align - 1) & !(align - 1);
        PhysicalAddress(addr)
    }

    pub fn as_mut_ptr<T>(&self) -> *mut T {
        self.as_u64() as *mut T
    }

    pub fn inner(&self) -> u64 {
        self.0
    }
}

impl Address for PhysicalAddress {
    fn as_u64(&self) -> u64 {
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

#[derive(Clone)]
pub struct VirtualAddress(u64);

impl VirtualAddress {
    pub fn new(address: u64) -> Self {
        Self(address)
    }

    pub fn align_down(&self, align: u64) -> VirtualAddress {
        let addr = self.0 & (align - 1);
        VirtualAddress(addr)
    }

    pub fn align_up(&self, align: u64) -> VirtualAddress {
        let addr = (self.0 + align - 1) & !(align - 1);
        VirtualAddress(addr)
    }

    pub fn as_mut_ptr<T>(&self) -> *mut T {
        self.as_u64() as *mut T
    }

    pub fn as_ptr<T>(&self) -> *const T {
        self.as_u64() as *const T
    }

    pub fn l4_index(&self) -> usize {
        self.0.get_bits(39..=47) as usize
    }

    pub fn l3_index(&self) -> usize {
        self.0.get_bits(30..=38) as usize
    }

    pub fn l2_index(&self) -> usize {
        self.0.get_bits(21..=29) as usize
    }

    pub fn l1_index(&self) -> usize {
        self.0.get_bits(12..=20) as usize
    }
}

impl Add<u64> for VirtualAddress {
    type Output = Self;
    fn add(self, rhs: u64) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl AddAssign<u64> for VirtualAddress {
    fn add_assign(&mut self, rhs: u64) {
        self.0 += rhs;
    }
}

impl LowerHex for VirtualAddress {
    fn fmt(&self, f: &mut Formatter) -> Result {
        LowerHex::fmt(&self.0, f)
    }
}

impl Address for VirtualAddress {
    fn as_u64(&self) -> u64 {
        self.0
    }
}

pub const PAGE_SIZE: usize = 0x1000;

#[derive(Copy, Clone)]
pub struct PhysicalFrame<S: PageSize = Size4KiB> {
    pub address: PhysicalAddress,
    pub size: PhantomData<S>,
}

impl<S: PageSize> PhysicalFrame<S> {
    pub fn at_address(address: PhysicalAddress) -> Self {
        Self {
            address: address.align_down(S::SIZE),
            size: PhantomData,
        }
    }

    pub fn start(&self) -> u64 {
        self.address.as_u64()
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

pub struct Page<S: PageSize = Size4KiB> {
    pub address: VirtualAddress,
    pub size: PhantomData<S>,
}

impl<S: PageSize> Page<S> {
    pub fn at_address(address: VirtualAddress) -> Self {
        Self {
            address: address.align_down(S::SIZE),
            size: PhantomData,
        }
    }
}

impl<S: PageSize> Add<u64> for Page<S> {
    type Output = Self;
    fn add(self, rhs: u64) -> Self::Output {
        Self {
            address: self.address + S::SIZE * rhs,
            size: PhantomData,
        }
    }
}

impl<S: PageSize> AddAssign<u64> for Page<S> {
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
