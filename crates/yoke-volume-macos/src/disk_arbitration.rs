#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    dead_code
)]

use core_foundation_sys::base::CFAllocatorRef;
use core_foundation_sys::dictionary::CFDictionaryRef;
use core_foundation_sys::runloop::CFRunLoopRef;
use core_foundation_sys::string::CFStringRef;
use libc::{c_char, c_void};

// CFRunLoopMode is CFStringRef in the Apple headers
pub type CFRunLoopMode = CFStringRef;

pub type DASessionRef = *const c_void;
pub type DADiskRef = *const c_void;

pub type DADiskAppearedCallback = unsafe extern "C" fn(disk: DADiskRef, context: *mut c_void);
pub type DADiskDisappearedCallback = unsafe extern "C" fn(disk: DADiskRef, context: *mut c_void);
pub type DADiskDescriptionChangedCallback =
    unsafe extern "C" fn(disk: DADiskRef, keys: CFDictionaryRef, context: *mut c_void);

#[link(name = "DiskArbitration", kind = "framework")]
unsafe extern "C" {
    pub fn DASessionCreate(allocator: CFAllocatorRef) -> DASessionRef;

    pub fn DASessionScheduleWithRunLoop(
        session: DASessionRef,
        run_loop: CFRunLoopRef,
        run_loop_mode: CFRunLoopMode,
    );

    pub fn DASessionUnscheduleFromRunLoop(
        session: DASessionRef,
        run_loop: CFRunLoopRef,
        run_loop_mode: CFRunLoopMode,
    );

    pub fn DARegisterDiskAppearedCallback(
        session: DASessionRef,
        match_dict: CFDictionaryRef,
        callback: DADiskAppearedCallback,
        context: *mut c_void,
    );

    pub fn DARegisterDiskDisappearedCallback(
        session: DASessionRef,
        match_dict: CFDictionaryRef,
        callback: DADiskDisappearedCallback,
        context: *mut c_void,
    );

    pub fn DARegisterDiskDescriptionChangedCallback(
        session: DASessionRef,
        match_dict: CFDictionaryRef,
        watch_dict: CFDictionaryRef,
        callback: DADiskDescriptionChangedCallback,
        context: *mut c_void,
    );

    pub fn DADiskCopyDescription(disk: DADiskRef) -> CFDictionaryRef;

    pub fn DADiskCreateFromVolumePath(
        allocator: CFAllocatorRef,
        session: DASessionRef,
        volume_path: *const c_void,
    ) -> DADiskRef;

    pub fn DADiskGetBSDName(disk: DADiskRef) -> *const c_char;
}
