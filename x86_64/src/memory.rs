use bit_field::BitField;
use core::{
    fmt::{self, Display, Formatter, LowerHex, Result},
    marker::PhantomData,
    ops::{Add, AddAssign, Sub},
};

pub const KiB: usize = 1024;
pub const MiB: usize = KiB * 1024;
pub const GiB: usize = MiB * 1024;

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

pub trait PageSize: Copy + Eq + PartialOrd + Ord {
    const SIZE: u64;
}

#[derive(Clone, Copy, Eq, PartialEq, PartialOrd, Ord, Debug)]
pub enum Size4KiB {}

impl PageSize for Size4KiB {
    const SIZE: u64 = 0x1000;
}

pub trait Address {
    fn as_u64(&self) -> u64;
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Ord, PartialOrd)]
pub struct PhysicalAddress(u64);

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

impl Display for PhysicalAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#x}", self.0)
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

impl Add<PhysicalAddress> for u64 {
    type Output = PhysicalAddress;

    fn add(self, rhs: PhysicalAddress) -> Self::Output {
        PhysicalAddress(self + rhs.0)
    }
}

impl Add<usize> for PhysicalAddress {
    type Output = Self;
    fn add(self, rhs: usize) -> Self::Output {
        let rhs: u64 = rhs.try_into().unwrap();
        Self(self.0 + rhs)
    }
}

impl Add<PhysicalAddress> for PhysicalAddress {
    type Output = Self;
    fn add(self, rhs: PhysicalAddress) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub<u64> for PhysicalAddress {
    type Output = Self;
    fn sub(self, rhs: u64) -> Self::Output {
        Self(self.0.checked_sub(rhs).unwrap())
    }
}

impl Sub<PhysicalAddress> for PhysicalAddress {
    type Output = Self;
    fn sub(self, rhs: PhysicalAddress) -> Self::Output {
        Self(self.0.checked_sub(rhs.0).unwrap())
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

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct VirtualAddress(u64);

impl VirtualAddress {
    pub fn new(address: u64) -> Self {
        Self(address)
    }

    pub fn align_down(&self, align: u64) -> VirtualAddress {
        let addr = self.0 & !(align - 1);
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

impl Add<VirtualAddress> for u64 {
    type Output = VirtualAddress;

    fn add(self, rhs: VirtualAddress) -> Self::Output {
        VirtualAddress(self + rhs.0)
    }
}

impl Add<usize> for VirtualAddress {
    type Output = Self;
    fn add(self, rhs: usize) -> Self::Output {
        let rhs: u64 = rhs.try_into().unwrap();
        Self(self.0 + rhs)
    }
}

impl Sub<u64> for VirtualAddress {
    type Output = Self;
    fn sub(self, rhs: u64) -> Self::Output {
        Self(self.0.checked_sub(rhs).unwrap())
    }
}

impl Add<VirtualAddress> for VirtualAddress {
    type Output = Self;
    fn add(self, rhs: VirtualAddress) -> Self::Output {
        Self(self.0 + rhs.0)
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

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct PhysicalFrame<S: PageSize = Size4KiB> {
    pub address: PhysicalAddress,
    pub size: PhantomData<S>,
}

impl<S: PageSize> Display for PhysicalFrame<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.address)
    }
}

impl<S: PageSize> PhysicalFrame<S> {
    pub fn containing_address(address: PhysicalAddress) -> Self {
        Self {
            address: address.align_down(S::SIZE),
            size: PhantomData,
        }
    }

    pub fn start(&self) -> u64 {
        self.address.as_u64()
    }

    pub fn size(&self) -> usize {
        S::SIZE as usize
    }

    pub fn range_inclusive(
        start: PhysicalFrame<S>,
        end: PhysicalFrame<S>,
    ) -> PhysicalFrameRangeInclusive<S> {
        PhysicalFrameRangeInclusive { start, end }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PhysicalFrameRangeInclusive<S: PageSize = Size4KiB> {
    pub start: PhysicalFrame<S>,
    pub end: PhysicalFrame<S>,
}

impl<S: PageSize> Iterator for PhysicalFrameRangeInclusive<S> {
    type Item = PhysicalFrame<S>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.start <= self.end {
            let frame = self.start;
            self.start += 1;
            Some(frame)
        } else {
            None
        }
    }
}

impl<S: PageSize> Add<u64> for PhysicalFrame<S> {
    type Output = Self;
    fn add(self, rhs: u64) -> Self::Output {
        PhysicalFrame::containing_address(self.address + rhs * S::SIZE)
    }
}

impl<S: PageSize> Add<PhysicalFrame<S>> for PhysicalFrame<S> {
    type Output = u64;
    fn add(self, rhs: PhysicalFrame<S>) -> Self::Output {
        let res = self.address + rhs.address;
        res.as_u64()
    }
}

impl<S: PageSize> Sub<u64> for PhysicalFrame<S> {
    type Output = Self;
    fn sub(self, rhs: u64) -> Self::Output {
        PhysicalFrame::containing_address(self.address - rhs * S::SIZE)
    }
}

impl<S: PageSize> Sub<PhysicalFrame<S>> for PhysicalFrame<S> {
    type Output = u64;
    fn sub(self, rhs: PhysicalFrame<S>) -> Self::Output {
        (self.address.as_u64() - rhs.address.as_u64()) / S::SIZE
    }
}

impl<S: PageSize> AddAssign<u64> for PhysicalFrame<S> {
    fn add_assign(&mut self, rhs: u64) {
        self.address += S::SIZE * rhs;
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Page<S: PageSize = Size4KiB> {
    pub address: VirtualAddress,
    pub size: PhantomData<S>,
}

impl<S: PageSize> Page<S> {
    pub fn containing_address(address: VirtualAddress) -> Self {
        Self {
            address: address.align_down(S::SIZE),
            size: PhantomData,
        }
    }
    pub fn range_inclusive(start: Page<S>, end: Page<S>) -> PageRangeInclusive<S> {
        PageRangeInclusive { start, end }
    }

    pub fn size(self) -> u64 {
        S::SIZE
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PageRangeInclusive<S: PageSize = Size4KiB> {
    pub start: Page<S>,
    pub end: Page<S>,
}

impl<S: PageSize> Iterator for PageRangeInclusive<S> {
    type Item = Page<S>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.start <= self.end {
            let frame = self.start;
            self.start += 1;
            Some(frame)
        } else {
            None
        }
    }
}

impl<S: PageSize> Add<u64> for Page<S> {
    type Output = Self;
    fn add(self, rhs: u64) -> Self::Output {
        Page::containing_address(self.address + rhs * S::SIZE)
    }
}

impl<S: PageSize> Sub<u64> for Page<S> {
    type Output = Self;
    fn sub(self, rhs: u64) -> Self::Output {
        Page::containing_address(self.address - rhs * S::SIZE)
    }
}

impl<S: PageSize> Sub<Page<S>> for Page<S> {
    type Output = u64;
    fn sub(self, rhs: Page<S>) -> Self::Output {
        (self.address.as_u64() - rhs.address.as_u64()) / S::SIZE
    }
}

impl<S: PageSize> AddAssign<u64> for Page<S> {
    fn add_assign(&mut self, rhs: u64) {
        self.address += S::SIZE * rhs;
    }
}

pub trait MemoryRegion: Copy + core::fmt::Debug {
    fn start(&self) -> u64;
    fn end(&self) -> u64;
    fn length(&self) -> u64;
    fn contains(&self, start: u64) -> bool;
    fn usable(&self) -> bool;
}
