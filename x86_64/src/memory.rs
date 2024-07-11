use bit_field::BitField;
use core::{
    fmt::{self, Display, Formatter, LowerHex, Result},
    marker::PhantomData,
    ops::{Add, AddAssign, Range, Rem, Sub},
};

pub const KIB: u64 = 1024;
pub const MIB: u64 = KIB * 1024;
pub const GIB: u64 = MIB * 1024;
pub const TIB: u64 = GIB * 1024;

/// A trait for types that can allocate a frame of memory.
///
/// # Safety
///
/// The implementer of this trait must guarantee that the `allocate_frame`
/// method returns only unique unused frames. Otherwise, undefined behavior
/// may result from two callers modifying or deallocating the same frame.
pub unsafe trait FrameAllocator<S: PageSize> {
    /// Allocate a frame of the appropriate size and return it if possible.
    fn allocate_frame(&mut self) -> Option<PhysicalFrame<S>>;
    fn deallocate_frame(&mut self, frame: PhysicalFrame<S>);
}

pub trait MemoryRegion: Copy + core::fmt::Debug {
    fn start(&self) -> u64;
    fn set_start(&mut self, start: u64);
    fn end(&self) -> u64;
    fn size(&self) -> usize;
    fn set_size(&mut self, size: usize);
    fn contains(&self, start: u64) -> bool;
    fn is_usable(&self) -> bool;
}

#[derive(Clone, Copy, Debug)]
pub struct Region {
    pub start: u64,
    pub size: usize,
}

impl Region {
    pub fn new(start: u64, size: usize) -> Region {
        Region { start, size }
    }
}

impl MemoryRegion for Region {
    fn start(&self) -> u64 {
        self.start
    }

    fn end(&self) -> u64 {
        self.start + self.size as u64
    }

    fn size(&self) -> usize {
        self.size
    }

    fn contains(&self, address: u64) -> bool {
        self.start() <= address && address <= self.end()
    }

    fn is_usable(&self) -> bool {
        true
    }

    fn set_start(&mut self, start: u64) {
        self.start = start
    }

    fn set_size(&mut self, size: usize) {
        self.size = size
    }
}

pub struct PhysicalRange {
    pub start: PhysicalAddress,
    pub size: usize,
}

impl PhysicalRange {
    pub fn new(start: PhysicalAddress, size: usize) -> Self {
        Self { start, size }
    }

    pub fn start(&self) -> PhysicalAddress {
        self.start
    }

    pub fn end(&self) -> PhysicalAddress {
        self.start + self.size
    }
    fn contains(&self, address: PhysicalAddress) -> bool {
        self.start() <= address && address < self.end()
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct VirtualRange {
    pub start: VirtualAddress,
    pub size: usize,
}

impl VirtualRange {
    pub const fn new(start: VirtualAddress, size: usize) -> Self {
        Self { start, size }
    }

    pub const fn new_empty() -> Self {
        Self {
            start: VirtualAddress::new(0),
            size: 0,
        }
    }

    pub fn start(&self) -> VirtualAddress {
        self.start
    }

    pub fn end(&self) -> VirtualAddress {
        self.start + self.size
    }

    fn contains(&self, address: VirtualAddress) -> bool {
        self.start() <= address && address < self.end()
    }
}

#[allow(dead_code)]
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub enum PhysicalMemoryRegionType {
    /// Either reserved by firmware
    #[default]
    Reserved,
    /// Memory that can be freely used by OS
    Free,

    /// Used by Bootloader / Kernel
    Used,
}

// ensure 8 byte alignment so it works between the different cpu modes where we have
// 2 byte, 4 byte and 8 byte alignments
// This struct MUST NOT contain any usize types since it is passed between different
// CPU operating modes and therefore usize representation changes.
#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
#[repr(align(8))]
pub struct PhysicalMemoryRegion {
    pub start: u64,
    pub size: u64,
    pub typ: PhysicalMemoryRegionType,
}

impl PhysicalMemoryRegion {
    pub fn new(start: u64, size: u64, typ: PhysicalMemoryRegionType) -> Self {
        Self { start, size, typ }
    }

    pub fn address(&self) -> PhysicalAddress {
        PhysicalAddress(self.start)
    }
}

impl MemoryRegion for PhysicalMemoryRegion {
    fn start(&self) -> u64 {
        self.start
    }

    fn end(&self) -> u64 {
        self.start + self.size as u64
    }

    fn size(&self) -> usize {
        self.size as usize
    }

    fn contains(&self, address: u64) -> bool {
        self.start() <= address && address <= self.end()
    }

    fn is_usable(&self) -> bool {
        self.typ == PhysicalMemoryRegionType::Free
    }

    fn set_start(&mut self, start: u64) {
        self.start = start
    }

    fn set_size(&mut self, size: usize) {
        self.size = size as u64
    }
}

pub struct PageAlignedSize(usize);

impl PageAlignedSize {
    pub fn new(size: usize) -> Self {
        Self(PageAlignedSize::align_up(size))
    }

    pub fn align_up(size: usize) -> usize {
        (size + Size4KiB::SIZE - 1) & !(Size4KiB::SIZE - 1)
    }

    pub fn inner(&self) -> usize {
        self.0
    }
}

pub trait PageSize: Copy + Eq + PartialOrd + Ord {
    const SIZE: usize;
    fn is_aligned(val: usize) -> bool {
        val & (Self::SIZE - 1) == 0
    }
}

#[derive(Clone, Copy, Eq, PartialEq, PartialOrd, Ord, Debug)]
pub enum Size4KiB {}

impl PageSize for Size4KiB {
    const SIZE: usize = 0x1000;
}

#[derive(Clone, Copy, Eq, PartialEq, PartialOrd, Ord, Debug)]
pub enum Size2MiB {}

impl PageSize for Size2MiB {
    const SIZE: usize = 0x200000;
}

pub trait Address {
    fn as_u64(&self) -> u64;
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Ord, PartialOrd)]
pub struct PhysicalAddress(u64);

impl PhysicalAddress {
    pub const fn new(address: u64) -> Self {
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

    /// Returns the inner value of the tuple struct as raw mutable pointer
    pub fn inner_as_mut_ptr(&mut self) -> *mut u64 {
        &mut self.0 as *mut u64
    }

    /// Returns the inner value of the tuple struct as const raw pointer
    pub fn inner_as_ptr(&self) -> *const u64 {
        &self.0 as *const u64
    }

    /// Inteprets the inner value as address and returns a raw mutable pointer to it
    pub fn as_mut_ptr<T>(&self) -> *mut T {
        self.as_u64() as *mut T
    }

    /// Inteprets the inner value as address and returns a const raw poiner to it
    pub fn as_ptr<T>(&self) -> *const T {
        self.as_u64() as *const T
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
    type Output = u64;
    fn sub(self, rhs: PhysicalAddress) -> u64 {
        self.0.checked_sub(rhs.0).unwrap()
    }
}

impl Rem<u64> for PhysicalAddress {
    type Output = Self;
    fn rem(self, rhs: u64) -> Self::Output {
        Self(self.0 % rhs)
    }
}

impl Rem<usize> for PhysicalAddress {
    type Output = Self;
    fn rem(self, rhs: usize) -> Self::Output {
        let rhs: u64 = rhs.try_into().unwrap();
        Self(self.0 % rhs)
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
    pub const fn new(address: u64) -> Self {
        Self(address)
    }

    pub fn is_aligned(&self, align: u64) -> bool {
        self.0 & (align - 1) == 0
    }

    pub fn align_down(&self, align: u64) -> Self {
        let addr = self.0 & !(align - 1);
        VirtualAddress(addr)
    }

    pub fn align_up(&self, align: usize) -> Self {
        let align = align as u64;
        let addr = (self.0 + align - 1) & !(align - 1);
        VirtualAddress(addr)
    }

    pub fn from_ptr<T>(ptr: &T) -> Self {
        let addr = ptr as *const _ as u64;
        VirtualAddress(addr)
    }

    pub fn from_raw_ptr<T>(ptr: *const T) -> Self {
        let addr = ptr as u64;
        VirtualAddress(addr)
    }

    pub fn from_raw_mut_ptr<T>(ptr: *mut T) -> Self {
        let addr = ptr as u64;
        VirtualAddress(addr)
    }

    /// Returns the inner value of the tuple struct as raw mutable pointer
    pub fn inner_as_mut_ptr(&mut self) -> *mut u64 {
        &mut self.0 as *mut u64
    }

    /// Returns the inner value of the tuple struct as const raw pointer
    pub fn inner_as_ptr(&self) -> *const u64 {
        &self.0 as *const u64
    }

    /// Inteprets the inner value as address and returns a raw mutable pointer to it
    pub fn as_mut_ptr<T>(&self) -> *mut T {
        self.as_u64() as *mut T
    }

    /// Inteprets the inner value as address and returns a const raw poiner to it
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

impl Add<usize> for VirtualAddress {
    type Output = Self;
    fn add(self, rhs: usize) -> Self::Output {
        let rhs: u64 = rhs.try_into().unwrap();
        Self(self.0 + rhs)
    }
}

impl Add<VirtualAddress> for u64 {
    type Output = VirtualAddress;

    fn add(self, rhs: VirtualAddress) -> Self::Output {
        VirtualAddress(self + rhs.0)
    }
}

impl Sub<u64> for VirtualAddress {
    type Output = Self;
    fn sub(self, rhs: u64) -> Self::Output {
        Self(self.0.checked_sub(rhs).unwrap())
    }
}

impl Sub<usize> for VirtualAddress {
    type Output = Self;
    fn sub(self, rhs: usize) -> Self::Output {
        let rhs: u64 = rhs.try_into().unwrap();
        Self(self.0.checked_sub(rhs).unwrap())
    }
}

impl Rem<u64> for VirtualAddress {
    type Output = Self;
    fn rem(self, rhs: u64) -> Self::Output {
        Self(self.0 % rhs)
    }
}

impl Rem<usize> for VirtualAddress {
    type Output = Self;
    fn rem(self, rhs: usize) -> Self::Output {
        let rhs: u64 = rhs.try_into().unwrap();
        Self(self.0 % rhs)
    }
}

// Just makes more sense that subbing two addresses returns the distance between them,
// not another address
impl Sub<VirtualAddress> for VirtualAddress {
    type Output = u64;
    fn sub(self, rhs: VirtualAddress) -> u64 {
        self.as_u64().checked_sub(rhs.as_u64()).unwrap()
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
    pub const fn new() -> Self {
        Self {
            address: PhysicalAddress::new(0),
            size: PhantomData,
        }
    }
    pub fn containing_address(address: PhysicalAddress) -> Self {
        Self {
            address: address.align_down(S::SIZE as u64),
            size: PhantomData,
        }
    }

    pub fn end(&self) -> u64 {
        self.start() + self.size() as u64
    }

    pub fn start(&self) -> u64 {
        self.address.as_u64()
    }

    pub fn address(&self) -> PhysicalAddress {
        self.address
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
            // add 1 frame to start = S::SIZE
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
        PhysicalFrame::containing_address(self.address + rhs * S::SIZE as u64)
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
        PhysicalFrame::containing_address(self.address - rhs * S::SIZE as u64)
    }
}

impl<S: PageSize> Sub<PhysicalFrame<S>> for PhysicalFrame<S> {
    type Output = u64;
    fn sub(self, rhs: PhysicalFrame<S>) -> Self::Output {
        (self.address.as_u64() - rhs.address.as_u64()) / S::SIZE as u64
    }
}

impl<S: PageSize> AddAssign<u64> for PhysicalFrame<S> {
    fn add_assign(&mut self, rhs: u64) {
        self.address += S::SIZE as u64 * rhs;
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Page<S: PageSize = Size4KiB> {
    pub address: VirtualAddress,
    pub size: PhantomData<S>,
}

impl<S: PageSize> Page<S> {
    /// Get the page that the virtual address is contained in.
    ///
    /// Aligns the address down to the next page boundary
    pub fn containing_address(address: VirtualAddress) -> Self {
        Self {
            address: address.align_down(S::SIZE as u64),
            size: PhantomData,
        }
    }

    /// Get the page corresponding to the address
    ///
    /// The addressed passed must be page aligned. Else this function
    /// panics
    pub fn for_address(address: VirtualAddress) -> Self {
        assert!(address.is_aligned(S::SIZE as u64));
        Self {
            address,
            size: PhantomData,
        }
    }

    pub fn range_inclusive(start: Page<S>, end: Page<S>) -> PageRangeInclusive<S> {
        PageRangeInclusive {
            start_page: start,
            end_page: end,
        }
    }

    pub fn size(self) -> usize {
        S::SIZE
    }

    pub fn start_address(&self) -> VirtualAddress {
        self.address
    }

    pub fn end_address(&self) -> VirtualAddress {
        self.address + self.size()
    }

    pub fn address(&self) -> VirtualAddress {
        self.address
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PageRangeInclusive<S: PageSize = Size4KiB> {
    pub start_page: Page<S>,
    pub end_page: Page<S>,
}

impl<S: PageSize> Into<Range<u64>> for PageRangeInclusive<S> {
    fn into(self) -> Range<u64> {
        self.start_page.start_address().as_u64()..self.end_page.end_address().as_u64()
    }
}

impl<S: PageSize> PageRangeInclusive<S> {
    pub fn new(start_page: Page<S>, end_page: Page<S>) -> Self {
        Self {
            start_page,
            end_page,
        }
    }

    pub const fn empty() -> Self {
        Self {
            start_page: Page {
                address: VirtualAddress::new(0),
                size: PhantomData,
            },
            end_page: Page {
                address: VirtualAddress::new(0),
                size: PhantomData,
            },
        }
    }

    pub fn start_page(&self) -> Page<S> {
        self.start_page
    }

    pub fn end_page(&self) -> Page<S> {
        self.end_page
    }

    pub fn start_address(&self) -> VirtualAddress {
        self.start_page.address()
    }

    pub fn end_address(&self) -> VirtualAddress {
        self.end_page.end_address()
    }

    pub fn contains(&self, range: &PageRangeInclusive) -> bool {
        self.start_address() <= range.start_address() && self.end_address() >= range.end_address()
    }

    pub fn overlaps(&self, range: &PageRangeInclusive) -> bool {
        !(self.end_address() < range.start_address() || self.start_address() > range.end_address())
    }

    pub fn size(&self) -> usize {
        usize::try_from(self.end_page.end_address() - self.start_page.start_address()).unwrap()
    }

    pub fn len(&self) -> usize {
        self.size() / S::SIZE
    }
}

impl<S: PageSize> Iterator for PageRangeInclusive<S> {
    type Item = Page<S>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.start_page <= self.end_page {
            let frame = self.start_page;
            self.start_page += 1;
            Some(frame)
        } else {
            None
        }
    }
}

impl<S: PageSize> Add<u64> for Page<S> {
    type Output = Self;
    fn add(self, rhs: u64) -> Self::Output {
        Page::containing_address(self.address + rhs * S::SIZE as u64)
    }
}

impl<S: PageSize> Add<usize> for Page<S> {
    type Output = Self;
    fn add(self, rhs: usize) -> Self::Output {
        Page::containing_address(self.address + rhs * S::SIZE)
    }
}

impl<S: PageSize> Sub<u64> for Page<S> {
    type Output = Self;
    fn sub(self, rhs: u64) -> Self::Output {
        Page::containing_address(self.address - rhs * S::SIZE as u64)
    }
}

impl<S: PageSize> Sub<usize> for Page<S> {
    type Output = Self;
    fn sub(self, rhs: usize) -> Self::Output {
        Page::containing_address(self.address - rhs * S::SIZE)
    }
}

impl<S: PageSize> Sub<Page<S>> for Page<S> {
    type Output = u64;
    fn sub(self, rhs: Page<S>) -> Self::Output {
        (self.address - rhs.address) / S::SIZE as u64
    }
}

impl<S: PageSize> AddAssign<u64> for Page<S> {
    fn add_assign(&mut self, rhs: u64) {
        self.address += S::SIZE as u64 * rhs;
    }
}
