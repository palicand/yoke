#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    dead_code
)]

use core_foundation_sys::base::CFAllocatorRef;
use core_foundation_sys::dictionary::CFMutableDictionaryRef;
use core_foundation_sys::number::{CFNumberGetValue, kCFNumberSInt32Type};
use core_foundation_sys::runloop::CFRunLoopSourceRef;
use core_foundation_sys::string::{CFStringCreateWithCString, CFStringRef, kCFStringEncodingUTF8};
use libc::{c_char, c_void};
use yoke_volume::state::{HORI_PS4_VID_PID, QUADSTICK_VID_PIDS, VidPid};

pub type io_object_t = u32;
pub type io_service_t = io_object_t;
pub type io_iterator_t = io_object_t;
pub type io_registry_entry_t = io_object_t;
pub type kern_return_t = i32;
pub type mach_port_t = u32;

pub type IONotificationPortRef = *mut c_void;
pub type IOServiceMatchingCallback =
    unsafe extern "C" fn(refcon: *mut c_void, iterator: io_iterator_t);

pub const kIOMatchedNotification: &[u8] = b"IOServiceFirstMatch\0";
pub const kIOTerminatedNotification: &[u8] = b"IOServiceTerminate\0";
pub const kIOUSBDeviceClassName: &[u8] = b"IOUSBDevice\0";
pub const kIOMainPortDefault: mach_port_t = 0;
pub const kIOServicePlane: &[u8] = b"IOService\0";

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    pub fn IOServiceMatching(name: *const c_char) -> CFMutableDictionaryRef;

    pub fn IOServiceGetMatchingServices(
        main_port: mach_port_t,
        matching: CFMutableDictionaryRef,
        existing: *mut io_iterator_t,
    ) -> kern_return_t;

    pub fn IONotificationPortCreate(main_port: mach_port_t) -> IONotificationPortRef;
    pub fn IONotificationPortDestroy(port: IONotificationPortRef);
    pub fn IONotificationPortGetRunLoopSource(port: IONotificationPortRef) -> CFRunLoopSourceRef;

    pub fn IOServiceAddMatchingNotification(
        port: IONotificationPortRef,
        notification_type: *const c_char,
        matching: CFMutableDictionaryRef,
        callback: IOServiceMatchingCallback,
        refcon: *mut c_void,
        iterator: *mut io_iterator_t,
    ) -> kern_return_t;

    pub fn IOIteratorNext(iterator: io_iterator_t) -> io_object_t;
    pub fn IOObjectRelease(object: io_object_t) -> kern_return_t;

    pub fn IORegistryEntryCreateCFProperty(
        entry: io_registry_entry_t,
        key: CFStringRef,
        allocator: CFAllocatorRef,
        options: u32,
    ) -> *const c_void;

    pub fn IORegistryEntryGetParentEntry(
        entry: io_registry_entry_t,
        plane: *const c_char,
        parent: *mut io_registry_entry_t,
    ) -> kern_return_t;

    pub fn IOBSDNameMatching(
        main_port: mach_port_t,
        options: u32,
        bsd_name: *const c_char,
    ) -> CFMutableDictionaryRef;

    pub fn IOServiceGetMatchingService(
        main_port: mach_port_t,
        matching: CFMutableDictionaryRef,
    ) -> io_service_t;
}

/// Walk the `IORegistry` ancestry from `start` upward looking for the first
/// entry that exposes both `idVendor` and `idProduct` (i.e. the enclosing
/// `IOUSBDevice`). Bounded to 16 steps to defend against pathological
/// registry shapes.
///
/// Reference semantics: `start` is owned by the caller and is NEVER released
/// by this function. Each intermediate parent fetched via
/// `IORegistryEntryGetParentEntry` is a +1 (Create-rule) reference and IS
/// released here, regardless of the success path.
///
/// # Safety
/// `start` must be a valid `io_service_t` from a `Get`-rule API (the caller
/// retains ownership).
pub unsafe fn find_usb_ancestor_vid_pid(start: io_service_t) -> Option<VidPid> {
    let plane = kIOServicePlane.as_ptr().cast::<c_char>();
    let mut current = start;
    let mut steps = 0u32;
    loop {
        if let (Some(vid), Some(pid)) =
            (unsafe { read_u16_property(current, "idVendor") }, unsafe {
                read_u16_property(current, "idProduct")
            })
        {
            if current != start {
                unsafe { IOObjectRelease(current) };
            }
            return Some(VidPid {
                vendor: vid,
                product: pid,
            });
        }
        if steps >= 16 {
            if current != start {
                unsafe { IOObjectRelease(current) };
            }
            return None;
        }
        let mut parent: io_registry_entry_t = 0;
        let kr = unsafe {
            IORegistryEntryGetParentEntry(current, plane, std::ptr::addr_of_mut!(parent))
        };
        if current != start {
            unsafe { IOObjectRelease(current) };
        }
        if kr != 0 || parent == 0 {
            return None;
        }
        current = parent;
        steps += 1;
    }
}

/// Look up an `IOMedia` (or other registry entry) by BSD name (e.g. `disk4s1`).
///
/// # Safety
/// `bsd_name` must point to a valid NUL-terminated C string for the duration
/// of the call. The returned service, if any, is owned by the caller and must
/// be released with `IOObjectRelease`.
pub unsafe fn iomedia_for_bsd_name(bsd_name: *const c_char) -> Option<io_service_t> {
    let matching = unsafe { IOBSDNameMatching(kIOMainPortDefault, 0, bsd_name) };
    if matching.is_null() {
        return None;
    }
    // IOServiceGetMatchingService consumes the matching dict reference, so we
    // must not release it ourselves.
    let svc = unsafe { IOServiceGetMatchingService(kIOMainPortDefault, matching) };
    if svc == 0 { None } else { Some(svc) }
}

/// Read `locationID` for the given `IOKit` USB device entry. The value
/// encodes the device's physical USB port path and survives re-enumeration
/// when the device swaps emulation modes (and therefore VID:PID).
///
/// # Safety
/// `entry` must be a valid `io_registry_entry_t` owned by the caller.
pub unsafe fn read_location_id(entry: io_registry_entry_t) -> Option<u32> {
    let cstring_key = std::ffi::CString::new("locationID").ok()?;
    let cf_key = unsafe {
        CFStringCreateWithCString(
            std::ptr::null(),
            cstring_key.as_ptr(),
            kCFStringEncodingUTF8,
        )
    };
    if cf_key.is_null() {
        return None;
    }
    let raw = unsafe { IORegistryEntryCreateCFProperty(entry, cf_key, std::ptr::null(), 0) };
    unsafe { core_foundation_sys::base::CFRelease(cf_key.cast()) };
    if raw.is_null() {
        return None;
    }
    let number_ref = raw.cast::<core_foundation_sys::number::__CFNumber>();
    let mut value: i32 = 0;
    let ok = unsafe {
        CFNumberGetValue(
            number_ref,
            kCFNumberSInt32Type,
            std::ptr::addr_of_mut!(value).cast(),
        )
    };
    unsafe { core_foundation_sys::base::CFRelease(raw) };
    if !ok {
        return None;
    }
    // locationID is documented as UInt32, but stored as a signed CFNumber.
    // Observed values fit comfortably in i32; reject the rare high-bit case
    // rather than silently aliasing.
    u32::try_from(value).ok()
}

/// Read a 16-bit integer property from an `IORegistry` entry by key name.
///
/// Returns `None` if the property is absent, cannot be created as a CF string,
/// or the underlying CF number does not fit in a u16 (which would indicate an
/// unexpected value from the OS — safer to reject than truncate).
pub unsafe fn read_u16_property(entry: io_registry_entry_t, key: &str) -> Option<u16> {
    let cstring_key = std::ffi::CString::new(key).ok()?;
    let cf_key = unsafe {
        CFStringCreateWithCString(
            std::ptr::null(),
            cstring_key.as_ptr(),
            kCFStringEncodingUTF8,
        )
    };
    if cf_key.is_null() {
        return None;
    }
    let raw = unsafe { IORegistryEntryCreateCFProperty(entry, cf_key, std::ptr::null(), 0) };
    unsafe { core_foundation_sys::base::CFRelease(cf_key.cast()) };
    if raw.is_null() {
        return None;
    }
    let number_ref = raw.cast::<core_foundation_sys::number::__CFNumber>();
    let mut value: i32 = 0;
    let ok = unsafe {
        CFNumberGetValue(
            number_ref,
            kCFNumberSInt32Type,
            std::ptr::addr_of_mut!(value).cast(),
        )
    };
    unsafe { core_foundation_sys::base::CFRelease(raw) };
    if !ok {
        return None;
    }
    u16::try_from(value).ok()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceClass {
    QuadStick(VidPid),
    HoriPs4,
    Other,
}

#[must_use]
pub fn classify(vid_pid: VidPid) -> DeviceClass {
    if QUADSTICK_VID_PIDS.contains(&vid_pid) {
        DeviceClass::QuadStick(vid_pid)
    } else if vid_pid == HORI_PS4_VID_PID {
        DeviceClass::HoriPs4
    } else {
        DeviceClass::Other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_quadstick_primary() {
        assert_eq!(
            classify(VidPid {
                vendor: 0x16D0,
                product: 0x092B
            }),
            DeviceClass::QuadStick(VidPid {
                vendor: 0x16D0,
                product: 0x092B
            })
        );
    }

    #[test]
    fn classify_hori_ps4() {
        assert_eq!(classify(HORI_PS4_VID_PID), DeviceClass::HoriPs4);
    }

    #[test]
    fn classify_random_device() {
        assert_eq!(
            classify(VidPid {
                vendor: 0x1234,
                product: 0x5678
            }),
            DeviceClass::Other
        );
    }
}
