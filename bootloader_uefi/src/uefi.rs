use core::ffi::c_void;

/// The common header that all UEFI tables begin with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct TableHeader {
    /// Unique identifier for this table.
    pub signature: u64,
    /// Revision of the spec this table conforms to.
    pub revision: u32,
    /// The size in bytes of the entire table.
    pub size: u32,
    /// 32-bit CRC-32-Castagnoli of the entire table,
    /// calculated with this field set to 0.
    pub crc: u32,
    /// Reserved field that must be set to 0.
    pub reserved: u32,
}

pub type Handle = *const c_void;
type Event = *const c_void;
type Char16 = u16;
type PhysicalAddress = u64;
type VirtualAddress = u64;

struct InputKey {
    pub scan_code: u16,
    pub unicode_char: Char16,
}

#[repr(usize)]
pub enum Status {
    /// The operation completed successfully.
    Success = 0,
    /// The string contained characters that could not be rendered and were skipped.
    WarnUnknownGlyph = 1,
    /// The handle was closed, but the file was not deleted.
    WarnDeleteFailure = 2,
    /// The handle was closed, but the data to the file was not flushed properly.
    WarnWriteFailure = 3,
    /// The resulting buffer was too small, and the data was truncated.
    WarnBufferTooSmall = 4,
    /// The data has not been updated within the timeframe set by local policy.
    WarnStaleData = 5,
    /// The resulting buffer contains UEFI-compliant file system.
    WarnFileSystem = 6,
    /// The operation will be processed across a system reset.
    WarnResetRequired = 7,

    /// The image failed to load.
    LoadError = Self::ERROR_BIT | 1,
    /// A parameter was incorrect.
    InvalidParameter = Self::ERROR_BIT | 2,
    /// The operation is not supported.
    Unsupported = Self::ERROR_BIT | 3,
    /// The buffer was not the proper size for the request.
    BadBufferSize = Self::ERROR_BIT | 4,
    /// The buffer is not large enough to hold the requested data.
    /// The required buffer size is returned in the appropriate parameter.
    BufferTooSmall = Self::ERROR_BIT | 5,
    /// There is no data pending upon return.
    NotReady = Self::ERROR_BIT | 6,
    /// The physical device reported an error while attempting the operation.
    DeviceError = Self::ERROR_BIT | 7,
    /// The device cannot be written to.
    WriteProtected = Self::ERROR_BIT | 8,
    /// A resource has run out.
    OutOfResources = Self::ERROR_BIT | 9,
    /// An inconsistency was detected on the file system.
    VolumeCorrupted = Self::ERROR_BIT | 10,
    /// There is no more space on the file system.
    VolumeFull = Self::ERROR_BIT | 11,
    /// The device does not contain any medium to perform the operation.
    NoMedia = Self::ERROR_BIT | 12,
    /// The medium in the device has changed since the last access.
    MediaChanged = Self::ERROR_BIT | 13,
    /// The item was not found.
    NotFound = Self::ERROR_BIT | 14,
    /// Access was denied.
    AccessDenied = Self::ERROR_BIT | 15,
    /// The server was not found or did not respond to the request.
    NoResponse = Self::ERROR_BIT | 16,
    /// A mapping to a device does not exist.
    NoMapping = Self::ERROR_BIT | 17,
    /// The timeout time expired.
    Timeout = Self::ERROR_BIT | 18,
    /// The protocol has not been started.
    NotStarted = Self::ERROR_BIT | 19,
    /// The protocol has already been started.
    AlreadyStarted = Self::ERROR_BIT | 20,
    /// The operation was aborted.
    Aborted = Self::ERROR_BIT | 21,
    /// An ICMP error occurred during the network operation.
    IcmpError = Self::ERROR_BIT | 22,
    /// A TFTP error occurred during the network operation.
    TftpError = Self::ERROR_BIT | 23,
    /// A protocol error occurred during the network operation.
    ProtocolError = Self::ERROR_BIT | 24,
    /// The function encountered an internal version that was
    /// incompatible with a version requested by the caller.
    IncompatibleVersion = Self::ERROR_BIT | 25,
    /// The function was not performed due to a security violation.
    SecurityViolation = Self::ERROR_BIT | 26,
    /// A CRC error was detected.
    CrcError = Self::ERROR_BIT | 27,
    /// Beginning or end of media was reached.
    EndOfMedia = Self::ERROR_BIT | 28,
    /// The end of the file was reached.
    EndOfFile = Self::ERROR_BIT | 31,
    /// The language specified was invalid.
    InvalidLanguage = Self::ERROR_BIT,
}

impl Status {
    /// Bit indicating that an UEFI status code is an error.
    pub const ERROR_BIT: usize = 1 << (core::mem::size_of::<usize>() * 8 - 1);

    // Returns true if status code indicates success.
    /*
    pub fn is_success(self) -> bool {
        self == Status::Success
    }
    */

    // Returns true if status code indicates a warning.
    /*
    pub fn is_warning(self) -> bool {
        (self != Status::Success) && (self.0 & Self::ERROR_BIT == 0)
    }
    */

    // Returns true if the status code indicates an error.
    /*
    pub const fn is_error(self) -> bool {
        self.0 & Self::ERROR_BIT != 0
    }
    */
}

/// The type of a memory range.
///
/// UEFI allows firmwares and operating systems to introduce new memory types
/// in the 0x70000000..0xFFFFFFFF range. Therefore, we don't know the full set
/// of memory types at compile time, and it is _not_ safe to model this C enum
/// as a Rust enum.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u32)]
enum MemoryType {
    /// This enum variant is not used.
    Reserved = 0,
    /// The code portions of a loaded UEFI application.
    LoaderCode = 1,
    /// The data portions of a loaded UEFI applications,
    /// as well as any memory allocated by it.
    LoaderData = 2,
    /// Code of the boot drivers.
    ///
    /// Can be reused after OS is loaded.
    BootServiceCode = 3,
    /// Memory used to store boot drivers' data.
    ///
    /// Can be reused after OS is loaded.
    BootServiceData = 4,
    /// Runtime drivers' code.
    RuntimeServiceCode = 5,
    /// Runtime services' code.
    RuntimeServiceData = 6,
    /// Free usable memory.
    CONVENTIONAL = 7,
    /// Memory in which errors have been detected.
    UNUSABLE = 8,
    /// Memory that holds ACPI tables.
    /// Can be reclaimed after they are parsed.
    AcpiReclaim = 9,
    /// Firmware-reserved addresses.
    AcpiNonVolatile = 10,
    /// A region used for memory-mapped I/O.
    Mmio = 11,
    /// Address space used for memory-mapped port I/O.
    MmioPortSpace = 12,
    /// Address space which is part of the processor.
    PalCode = 13,
    /// Memory region which is usable and is also non-volatile.
    PersistentMemory = 14,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u64)]
enum MemoryAttribute {
    /// Supports marking as uncacheable.
    Uncacheable = 1,
    /// Supports write-combining.
    WriteCombine = 2,
    /// Supports write-through.
    WriteThrough = 4,
    /// Support write-back.
    WriteBack = 8,
    /// Supports marking as uncacheable, exported and
    /// supports the "fetch and add" semaphore mechanism.
    UncacheableExported = 0x10,
    /// Supports write-protection.
    WriteProtect = 0x1000,
    /// Supports read-protection.
    ReadProtect = 0x2000,
    /// Supports disabling code execution.
    ExecuteProtect = 0x4000,
    /// Persistent memory.
    NonVolatile = 0x8000,
    /// This memory region is more reliable than other memory.
    MoreReliable = 0x10000,
    /// This memory range can be set as read-only.
    ReadOnly = 0x20000,
    /// This memory is earmarked for specific purposes such as for specific
    /// device drivers or applications. This serves as a hint to the OS to
    /// avoid this memory for core OS data or code that cannot be relocated.
    SpecialPurpose = 0x40000,
    /// This memory region is capable of being protected with the CPU's memory
    /// cryptography capabilities.
    CpuCrypto = 0x80000,
    /// This memory must be mapped by the OS when a runtime service is called.
    Runtime = 0x8000000000000000,
    /// This memory region is described with additional ISA-specific memory
    /// attributes as specified in `MemoryAttribute::ISA_MASK`.
    IsaValid = 0x4000000000000000,
    /// These bits are reserved for describing optional ISA-specific cache-
    /// ability attributes that are not covered by the standard UEFI Memory
    /// Attribute cacheability bits such as `UNCACHEABLE`, `WRITE_COMBINE`,
    /// `WRITE_THROUGH`, `WRITE_BACK`, and `UNCACHEABLE_EXPORTED`.
    ///
    /// See Section 2.3 "Calling Conventions" in the UEFI Specification
    /// for further information on each ISA that takes advantage of this.
    IsaMask = 0x0FFF_F000_0000_0000,
}

#[repr(u32)]
enum VariableAttributes {
    /// Variable is maintained across a power cycle.
    NonVolatile = 0x1,

    /// Variable is accessible during the time that boot services are
    /// accessible.
    BootServiceAccess = 0x2,

    /// Variable is accessible during the time that runtime services are
    /// accessible.
    RuntimeAccess = 0x4,

    /// Variable is stored in the portion of NVR allocated for error
    /// records.
    HardwareErrorRecord = 0x8,

    /// Deprecated.
    AuthenticatedWriteAccess = 0x10,

    /// Variable payload begins with an EFI_VARIABLE_AUTHENTICATION_2
    /// structure.
    TimeBasedAuthenticatedWriteAccess = 0x20,

    /// This is never set in the attributes returned by
    /// `get_variable`. When passed to `set_variable`, the variable payload
    /// will be appended to the current value of the variable if supported
    /// by the firmware.
    AppendWrite = 0x40,

    /// Variable payload begins with an EFI_VARIABLE_AUTHENTICATION_3
    /// structure.
    EnhancedAuthenticatedAccess = 0x80,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct MemoryDescriptor {
    /// Type of memory occupying this range.
    pub typ: MemoryType,
    /// Starting physical address.
    pub physical_start: PhysicalAddress,
    /// Starting virtual address.
    pub virtual_start: VirtualAddress,
    /// Number of 4 KiB pages contained in this range.
    pub page_count: u64,
    /// The capability attributes of this memory range.
    pub att: MemoryAttribute,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct SimpleTextOutputMode {
    pub max_mode: i32,
    pub mode: i32,
    pub attribute: i32,
    pub cursor_column: i32,
    pub cursor_row: i32,
    pub cursor_visible: bool,
}

/// Real time clock capabilities.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct TimeCapabilities {
    /// Reporting resolution of the clock in counts per second. 1 for a normal
    /// PC-AT CMOS RTC device, which reports the time with 1-second resolution.
    pub resolution: u32,

    /// Timekeeping accuracy in units of 1e-6 parts per million.
    pub accuracy: u32,

    /// Whether a time set operation clears the device's time below the
    /// "resolution" reporting level. False for normal PC-AT CMOS RTC devices.
    pub sets_to_zero: bool,
}

/// Date and time representation.
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
#[repr(C)]
pub struct Time {
    /// Year. Valid range: `1900..=9999`.
    pub year: u16,

    /// Month. Valid range: `1..=12`.
    pub month: u8,

    /// Day of the month. Valid range: `1..=31`.
    pub day: u8,

    /// Hour. Valid range: `0..=23`.
    pub hour: u8,

    /// Minute. Valid range: `0..=59`.
    pub minute: u8,

    /// Second. Valid range: `0..=59`.
    pub second: u8,

    /// Unused padding.
    pub pad1: u8,

    /// Nanosececond. Valid range: `0..=999_999_999`.
    pub nanosecond: u32,

    /// Offset in minutes from UTC. Valid range: `-1440..=1440`, or
    /// [`Time::UNSPECIFIED_TIMEZONE`].
    pub time_zone: i16,

    /// Daylight savings time information.
    pub daylight: u8,

    /// Unused padding.
    pub pad2: u8,
}

/// The type of system reset.
#[repr(u32)]
enum ResetType {
    /// System-wide reset.
    ///
    /// This is analogous to power cycling the device.
    Cold = 0,
    /// System-wide re-initialization.
    ///
    /// If the system doesn't support a warm reset, this will trigger a cold
    /// reset.
    Warm = 1,
    /// The system is powered off.
    Shutdown = 2,
    /// A platform-specific reset type.
    PlatformSpecific = 3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u32)]
enum CapsuleFlags {
    /// The meaning of this bit depends on the capsule GUID.
    TypeSpecificBit0 = 1 << 0,
    /// The meaning of this bit depends on the capsule GUID.
    TypeSpecificBit1 = 1 << 1,
    /// The meaning of this bit depends on the capsule GUID.
    TypeSpecificBit2 = 1 << 2,

    /// The meaning of this bit depends on the capsule GUID.
    TypeSpecificBit3 = 1 << 3,

    /// The meaning of this bit depends on the capsule GUID.
    TypeSpecificBit4 = 1 << 4,

    /// The meaning of this bit depends on the capsule GUID.
    TypeSpecificBit5 = 1 << 5,

    /// The meaning of this bit depends on the capsule GUID.
    TypeSpecificBit6 = 1 << 6,

    /// The meaning of this bit depends on the capsule GUID.
    TypeSpecificBit7 = 1 << 7,

    /// The meaning of this bit depends on the capsule GUID.
    TypeSpecificBit8 = 1 << 8,

    /// The meaning of this bit depends on the capsule GUID.
    TypeSpecificBit9 = 1 << 9,

    /// The meaning of this bit depends on the capsule GUID.
    TypeSpecificBit10 = 1 << 10,

    /// The meaning of this bit depends on the capsule GUID.
    TypeSpecificBit11 = 1 << 11,

    /// The meaning of this bit depends on the capsule GUID.
    TypeSpecificBit12 = 1 << 12,

    /// The meaning of this bit depends on the capsule GUID.
    TypeSpecificBit13 = 1 << 13,

    /// The meaning of this bit depends on the capsule GUID.
    TypeSpecificBit14 = 1 << 14,

    /// The meaning of this bit depends on the capsule GUID.
    TypeSpecificBit15 = 1 << 15,

    /// Indicates the firmware should process the capsule after system reset.
    PersistAcrossReset = 1 << 16,

    /// Causes the contents of the capsule to be coalesced from the
    /// scatter-gather list into a contiguous buffer, and then a pointer to
    /// that buffer will be placed in the configuration table after system
    /// reset.
    ///
    /// If this flag is set, [`PERSIST_ACROSS_RESET`] must be set as well.
    ///
    /// [`PERSIST_ACROSS_RESET`]: Self::PERSIST_ACROSS_RESET
    PopulateSystemTable = 1 << 17,

    /// Trigger a system reset after passing the capsule to the firmware.
    ///
    /// If this flag is set, [`PERSIST_ACROSS_RESET`] must be set as well.
    ///
    /// [`PERSIST_ACROSS_RESET`]: Self::PERSIST_ACROSS_RESET
    InitiateReset = 1 << 18,
}
/// Common header at the start of a capsule.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
struct CapsuleHeader {
    /// GUID that defines the type of data in the capsule.
    pub capsule_guid: Guid,

    /// Size in bytes of the capsule header. This may be larger than the size of
    /// `CapsuleHeader` since the specific capsule type defined by
    /// [`capsule_guid`] may add additional header fields.
    ///
    /// [`capsule_guid`]: Self::capsule_guid
    pub header_size: u32,

    /// Capsule update flags.
    pub flags: CapsuleFlags,

    /// Size in bytes of the entire capsule, including the header.
    pub capsule_image_size: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

#[derive(Debug)]
#[repr(C)]
struct RuntimeServices {
    pub header: TableHeader,
    pub get_time:
        unsafe extern "efiapi" fn(time: *mut Time, capabilities: *mut TimeCapabilities) -> Status,
    pub set_time: unsafe extern "efiapi" fn(time: *const Time) -> Status,
    pub get_wakeup_time:
        unsafe extern "efiapi" fn(enabled: *mut u8, pending: *mut u8, time: *mut Time) -> Status,
    pub set_wakeup_time: unsafe extern "efiapi" fn(enable: u8, time: *const Time) -> Status,
    pub set_virtual_address_map: unsafe extern "efiapi" fn(
        map_size: usize,
        desc_size: usize,
        desc_version: u32,
        virtual_map: *mut MemoryDescriptor,
    ) -> Status,
    pub convert_pointer:
        unsafe extern "efiapi" fn(debug_disposition: usize, address: *mut *const c_void) -> Status,
    pub get_variable: unsafe extern "efiapi" fn(
        variable_name: *const Char16,
        vendor_guid: *const Guid,
        attributes: *mut VariableAttributes,
        data_size: *mut usize,
        data: *mut u8,
    ) -> Status,
    pub get_next_variable_name: unsafe extern "efiapi" fn(
        variable_name_size: *mut usize,
        variable_name: *mut u16,
        vendor_guid: *mut Guid,
    ) -> Status,
    pub set_variable: unsafe extern "efiapi" fn(
        variable_name: *const Char16,
        vendor_guid: *const Guid,
        attributes: VariableAttributes,
        data_size: usize,
        data: *const u8,
    ) -> Status,
    pub get_next_high_monotonic_count: unsafe extern "efiapi" fn(high_count: *mut u32) -> Status,
    pub reset_system: unsafe extern "efiapi" fn(
        rt: ResetType,
        status: Status,
        data_size: usize,
        data: *const u8,
    ) -> !,

    // UEFI 2.0 Capsule Services.
    pub update_capsule: unsafe extern "efiapi" fn(
        capsule_header_array: *const *const CapsuleHeader,
        capsule_count: usize,
        scatter_gather_list: PhysicalAddress,
    ) -> Status,
    pub query_capsule_capabilities: unsafe extern "efiapi" fn(
        capsule_header_array: *const *const CapsuleHeader,
        capsule_count: usize,
        maximum_capsule_size: *mut usize,
        reset_type: *mut ResetType,
    ) -> Status,

    // Miscellaneous UEFI 2.0 Service.
    pub query_variable_info: unsafe extern "efiapi" fn(
        attributes: VariableAttributes,
        maximum_variable_storage_size: *mut u64,
        remaining_variable_storage_size: *mut u64,
        maximum_variable_size: *mut u64,
    ) -> Status,
}

/// Task priority level.
///
/// Although the UEFI specification repeatedly states that only the variants
/// specified below should be used in application-provided input, as the other
/// are reserved for internal firmware use, it might still happen that the
/// firmware accidentally discloses one of these internal TPLs to us.
///
/// Since feeding an unexpected variant to a Rust enum is UB, this means that
/// this C enum must be interfaced via the newtype pattern.
#[repr(usize)]
pub enum TaskPriorityLevel {
    /// Normal task execution level.
    Application = 4,
    /// Async interrupt-style callbacks run at this TPL.
    Callback = 8,
    /// Notifications are masked at this level.
    ///
    /// This is used in critical sections of code.
    Notify = 16,
    /// Highest priority level.
    ///
    /// Even processor interrupts are disable at this level.
    HighLevel = 31,
}

#[repr(u32)]
enum EventType {
    /// The event is a timer event and may be passed to `BootServices::set_timer()`
    /// Note that timers only function during boot services time.
    Timer = 0x8000_0000,

    /// The event is allocated from runtime memory.
    /// This must be done if the event is to be signaled after ExitBootServices.
    Runtime = 0x4000_0000,

    /// Calling wait_for_event or check_event will enqueue the notification
    /// function if the event is not already in the signaled state.
    /// Mutually exclusive with `NOTIFY_SIGNAL`.
    NotifyWait = 0x0000_0100,

    /// The notification function will be enqueued when the event is signaled
    /// Mutually exclusive with `NOTIFY_WAIT`.
    NotifySignal = 0x0000_0200,

    /// The event will be signaled at ExitBootServices time.
    /// This event type should not be combined with any other.
    /// Its notification function must follow some special rules:
    /// - Cannot use memory allocation services, directly or indirectly
    /// - Cannot depend on timer events, since those will be deactivated
    SignalExitBootService = 0x0000_0201,

    /// The event will be notified when SetVirtualAddressMap is performed.
    /// This event type should not be combined with any other.
    SignalVirtualAddressChange = 0x6000_0202,
}

/// Raw event notification function.
pub type EventNotifyFn = unsafe extern "efiapi" fn(event: Event, context: *mut c_void);

#[repr(u32)]
enum InterfaceType {
    Native = 0,
}

#[derive(Debug)]
#[repr(C)]
pub struct DevicePathProtocol {
    pub major_type: u8,
    pub sub_type: u8,
    pub length: [u8; 2],
    // followed by payload (dynamically sized)
}

#[derive(Debug)]
#[repr(C)]
pub struct OpenProtocolInformationEntry {
    pub agent_handle: Handle,
    pub controller_handle: Handle,
    pub attributes: u32,
    pub open_count: u32,
}

/// Table of pointers to all the boot services.
#[derive(Debug)]
#[repr(C)]
pub struct BootServices {
    pub header: TableHeader,

    // Task Priority services
    pub raise_tpl: unsafe extern "efiapi" fn(new_tpl: TaskPriorityLevel) -> TaskPriorityLevel,
    pub restore_tpl: unsafe extern "efiapi" fn(old_tpl: TaskPriorityLevel),

    // Memory allocation functions
    pub allocate_pages: unsafe extern "efiapi" fn(
        alloc_ty: u32,
        mem_ty: MemoryType,
        count: usize,
        addr: *mut PhysicalAddress,
    ) -> Status,
    pub free_pages: unsafe extern "efiapi" fn(addr: PhysicalAddress, pages: usize) -> Status,
    pub get_memory_map: unsafe extern "efiapi" fn(
        size: *mut usize,
        map: *mut MemoryDescriptor,
        key: *mut usize,
        desc_size: *mut usize,
        desc_version: *mut u32,
    ) -> Status,
    pub allocate_pool: unsafe extern "efiapi" fn(
        pool_type: MemoryType,
        size: usize,
        buffer: *mut *mut u8,
    ) -> Status,
    pub free_pool: unsafe extern "efiapi" fn(buffer: *mut u8) -> Status,

    // Event & timer functions
    pub create_event: unsafe extern "efiapi" fn(
        typ: EventType,
        notify_tpl: TaskPriorityLevel,
        notify_func: Option<EventNotifyFn>,
        notify_ctx: *mut c_void,
        out_event: *mut Event,
    ) -> Status,
    pub set_timer: unsafe extern "efiapi" fn(event: Event, ty: u32, trigger_time: u64) -> Status,
    pub wait_for_event: unsafe extern "efiapi" fn(
        number_of_events: usize,
        events: *mut Event,
        out_index: *mut usize,
    ) -> Status,
    pub signal_event: unsafe extern "efiapi" fn(event: Event) -> Status,
    pub close_event: unsafe extern "efiapi" fn(event: Event) -> Status,
    pub check_event: unsafe extern "efiapi" fn(event: Event) -> Status,

    // Protocol handlers
    pub install_protocol_interface: unsafe extern "efiapi" fn(
        handle: *mut Handle,
        guid: *const Guid,
        interface_type: InterfaceType,
        interface: *const c_void,
    ) -> Status,
    pub reinstall_protocol_interface: unsafe extern "efiapi" fn(
        handle: Handle,
        protocol: *const Guid,
        old_interface: *const c_void,
        new_interface: *const c_void,
    ) -> Status,
    pub uninstall_protocol_interface: unsafe extern "efiapi" fn(
        handle: Handle,
        protocol: *const Guid,
        interface: *const c_void,
    ) -> Status,
    pub handle_protocol: unsafe extern "efiapi" fn(
        handle: Handle,
        proto: *const Guid,
        out_proto: *mut *mut c_void,
    ) -> Status,
    pub reserved: *mut c_void,
    pub register_protocol_notify: unsafe extern "efiapi" fn(
        protocol: *const Guid,
        event: Event,
        registration: *mut *const c_void,
    ) -> Status,
    pub locate_handle: unsafe extern "efiapi" fn(
        search_ty: i32,
        proto: *const Guid,
        key: *const c_void,
        buf_sz: *mut usize,
        buf: *mut Handle,
    ) -> Status,
    pub locate_device_path: unsafe extern "efiapi" fn(
        proto: *const Guid,
        device_path: *mut *const DevicePathProtocol,
        out_handle: *mut Handle,
    ) -> Status,
    pub install_configuration_table:
        unsafe extern "efiapi" fn(guid_entry: *const Guid, table_ptr: *const c_void) -> Status,

    // Image services
    pub load_image: unsafe extern "efiapi" fn(
        boot_policy: u8,
        parent_image_handle: Handle,
        device_path: *const DevicePathProtocol,
        source_buffer: *const u8,
        source_size: usize,
        image_handle: *mut Handle,
    ) -> Status,
    pub start_image: unsafe extern "efiapi" fn(
        image_handle: Handle,
        exit_data_size: *mut usize,
        exit_data: *mut *mut Char16,
    ) -> Status,
    pub exit: unsafe extern "efiapi" fn(
        image_handle: Handle,
        exit_status: Status,
        exit_data_size: usize,
        exit_data: *mut Char16,
    ) -> !,
    pub unload_image: unsafe extern "efiapi" fn(image_handle: Handle) -> Status,
    pub exit_boot_services:
        unsafe extern "efiapi" fn(image_handle: Handle, map_key: usize) -> Status,

    // Misc services
    pub get_next_monotonic_count: unsafe extern "efiapi" fn(count: *mut u64) -> Status,
    pub stall: unsafe extern "efiapi" fn(microseconds: usize) -> Status,
    pub set_watchdog_timer: unsafe extern "efiapi" fn(
        timeout: usize,
        watchdog_code: u64,
        data_size: usize,
        watchdog_data: *const u16,
    ) -> Status,

    // Driver support services
    pub connect_controller: unsafe extern "efiapi" fn(
        controller: Handle,
        driver_image: Handle,
        remaining_device_path: *const DevicePathProtocol,
        recursive: bool,
    ) -> Status,
    pub disconnect_controller: unsafe extern "efiapi" fn(
        controller: Handle,
        driver_image: Handle,
        child: Handle,
    ) -> Status,

    // Protocol open / close services
    pub open_protocol: unsafe extern "efiapi" fn(
        handle: Handle,
        protocol: *const Guid,
        interface: *mut *mut c_void,
        agent_handle: Handle,
        controller_handle: Handle,
        attributes: u32,
    ) -> Status,
    pub close_protocol: unsafe extern "efiapi" fn(
        handle: Handle,
        protocol: *const Guid,
        agent_handle: Handle,
        controller_handle: Handle,
    ) -> Status,
    pub open_protocol_information: unsafe extern "efiapi" fn(
        handle: Handle,
        protocol: *const Guid,
        entry_buffer: *mut *const OpenProtocolInformationEntry,
        entry_count: *mut usize,
    ) -> Status,

    // Library services
    pub protocols_per_handle: unsafe extern "efiapi" fn(
        handle: Handle,
        protocol_buffer: *mut *mut *const Guid,
        protocol_buffer_count: *mut usize,
    ) -> Status,
    pub locate_handle_buffer: unsafe extern "efiapi" fn(
        search_ty: i32,
        proto: *const Guid,
        key: *const c_void,
        no_handles: *mut usize,
        buf: *mut *mut Handle,
    ) -> Status,
    pub locate_protocol: unsafe extern "efiapi" fn(
        proto: *const Guid,
        registration: *mut c_void,
        out_proto: *mut *mut c_void,
    ) -> Status,

    /// Warning: this function pointer is declared as `extern "C"` rather than
    /// `extern "efiapi". That means it will work correctly when called from a
    /// UEFI target (`*-unknown-uefi`), but will not work when called from a
    /// target with a different calling convention such as
    /// `x86_64-unknown-linux-gnu`.
    ///
    /// Support for C-variadics with `efiapi` requires the unstable
    /// [`extended_varargs_abi_support`](https://github.com/rust-lang/rust/issues/100189)
    /// feature.
    pub install_multiple_protocol_interfaces:
        unsafe extern "C" fn(handle: *mut Handle, ...) -> Status,

    /// Warning: this function pointer is declared as `extern "C"` rather than
    /// `extern "efiapi". That means it will work correctly when called from a
    /// UEFI target (`*-unknown-uefi`), but will not work when called from a
    /// target with a different calling convention such as
    /// `x86_64-unknown-linux-gnu`.
    ///
    /// Support for C-variadics with `efiapi` requires the unstable
    /// [`extended_varargs_abi_support`](https://github.com/rust-lang/rust/issues/100189)
    /// feature.
    pub uninstall_multiple_protocol_interfaces: unsafe extern "C" fn(handle: Handle, ...) -> Status,

    // CRC services
    pub calculate_crc32:
        unsafe extern "efiapi" fn(data: *const c_void, data_size: usize, crc32: *mut u32) -> Status,

    // Misc services
    pub copy_mem: unsafe extern "efiapi" fn(dest: *mut u8, src: *const u8, len: usize),
    pub set_mem: unsafe extern "efiapi" fn(buffer: *mut u8, len: usize, value: u8),

    // New event functions (UEFI 2.0 or newer)
    pub create_event_ex: unsafe extern "efiapi" fn(
        ty: EventType,
        notify_tpl: TaskPriorityLevel,
        notify_fn: Option<EventNotifyFn>,
        notify_ctx: *mut c_void,
        event_group: *mut Guid,
        out_event: *mut Event,
    ) -> Status,
}

// protocol used to obtain input from the ConsoleIn device
#[repr(C)]
pub struct SimpleTextInputProtocol {
    pub reset: extern "efiapi" fn(this: *mut Self, extended_verification: bool) -> Status,
    pub read_key_stroke: extern "efiapi" fn(this: *mut Self, key: *mut InputKey) -> Status,
    pub wait_for_key: Event,
}

// Protocol used to control text-based output devices
#[derive(Debug)]
#[repr(C)]
pub struct SimpleTextOutputProtocol {
    pub reset: unsafe extern "efiapi" fn(this: *mut Self, extended: bool) -> Status,
    pub output_string: unsafe extern "efiapi" fn(this: *mut Self, string: *const Char16) -> Status,
    pub test_string: unsafe extern "efiapi" fn(this: *mut Self, string: *const Char16) -> Status,
    pub query_mode: unsafe extern "efiapi" fn(
        this: *mut Self,
        mode: usize,
        columns: *mut usize,
        rows: *mut usize,
    ) -> Status,
    pub set_mode: unsafe extern "efiapi" fn(this: *mut Self, mode: usize) -> Status,
    pub set_attribute: unsafe extern "efiapi" fn(this: *mut Self, attribute: usize) -> Status,
    pub clear_screen: unsafe extern "efiapi" fn(this: *mut Self) -> Status,
    pub set_cursor_position:
        unsafe extern "efiapi" fn(this: *mut Self, column: usize, row: usize) -> Status,
    pub enable_cursor: unsafe extern "efiapi" fn(this: *mut Self, visible: bool) -> Status,
    pub mode: *mut SimpleTextOutputMode,
}

/// UEFI configuration table.
///
/// Each table is uniquely identified by a GUID. The type of data pointed to by
/// `vendor_table`, as well as whether that address is physical or virtual,
/// depends on the GUID.
#[derive(Debug, Eq, PartialEq)]
#[repr(C)]
pub struct ConfigurationTable {
    pub vendor_guid: Guid,
    pub vendor_table: *mut c_void,
}

#[repr(C)]
pub struct SystemTable {
    header: TableHeader,
    firmware_vendor: *mut Char16,
    firmware_revision: u32,
    console_in_handle: Handle,
    con_in: *mut SimpleTextInputProtocol,
    console_out_handle: Handle,
    con_out: *mut SimpleTextOutputProtocol,
    standard_error_handle: Handle,
    std_err: *mut SimpleTextOutputProtocol,
    runtime_services: *mut RuntimeServices,
    boot_services: *mut BootServices,
    number_of_table_entries: usize,
    configuration_table: *mut ConfigurationTable,
}
