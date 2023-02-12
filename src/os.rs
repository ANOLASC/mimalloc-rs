use std::{
    ffi::c_void,
    ptr,
    sync::atomic::{AtomicU32, Ordering},
};

use windows::{
    core::PCSTR,
    Win32::{
        Foundation::HINSTANCE,
        System::{
            LibraryLoader::{FreeLibrary, GetProcAddress, LoadLibraryA},
            Memory::{self, VirtualAlloc, PAGE_PROTECTION_FLAGS, VIRTUAL_ALLOCATION_TYPE},
            SystemInformation::{GetSystemInfo, SYSTEM_INFO},
        },
    },
};

// page size (initialized properly in `os_init`)
pub static OS_PAGE_SIZE: AtomicU32 = AtomicU32::new(4096);

// minimal allocation granularity
pub static OS_ALLOC_GRANULARITY: AtomicU32 = AtomicU32::new(4096);

// if non-zero, use large page allocation
pub static LARGE_OS_PAGE_SIZE: AtomicU32 = AtomicU32::new(0);

fn _mi_os_alloc_aligned_offset(
    size: usize,
    alignment: usize,
    offset: usize,
    commit: bool, /*, bool* large, mi_stats_t* tld_stats */
) -> *mut c_void {
    ptr::null_mut()
}

fn _mi_os_alloc_aligned(
    size: usize,
    alignment: usize,
    commit: bool, /*, bool* large, mi_stats_t* tld_stats */
) -> *mut c_void {
    ptr::null_mut()
}

fn mi_os_mem_alloc_aligned(
    size: usize,
    alignment: usize,
    commit: bool,
    allow_large: bool, /* bool* is_large, mi_stats_t* stats*/
) -> *mut c_void {
    ptr::null_mut()
}

fn mi_os_mem_alloc(
    size: usize,
    try_alignment: usize,
    commit: bool,
    allow_large: bool, /*bool* is_large, mi_stats_t* stats*/
) -> *mut c_void {
    if cfg!(windows) {}

    ptr::null_mut()
}

fn mi_win_virtual_alloc(
    addr: *mut c_void,
    size: usize,
    try_alignment: usize,
    flags: usize,
    large_only: bool,
    allow_large: bool, /* , bool* is_large */
) -> *mut c_void {
    unsafe {
        VirtualAlloc(
            Some(addr.cast_const()),
            size,
            VIRTUAL_ALLOCATION_TYPE(flags as u32),
            PAGE_PROTECTION_FLAGS(0x04),
        )
    }
}

fn _mi_os_init() {
    let mut si = SYSTEM_INFO::default();
    unsafe {
        GetSystemInfo(&mut si);
    }

    if si.dwPageSize > 0 {
        OS_PAGE_SIZE.store(si.dwPageSize, Ordering::Relaxed);
    }

    if si.dwAllocationGranularity > 0 {
        OS_ALLOC_GRANULARITY.store(si.dwAllocationGranularity, Ordering::Relaxed);
    }

    // TODO What if use win32 crate VirtualAlloc?
    let h_dll = unsafe { LoadLibraryA::<PCSTR>(PCSTR::from_raw("kernelbase.dll".as_ptr())) };
    if let Ok(h_dll) = h_dll {
        // use VirtualAlloc2FromApp if possible as it is available to Windows store apps
        unsafe {
            let mut pVirtualAlloc2 =
                GetProcAddress(h_dll, PCSTR::from_raw("VirtualAlloc2FromApp".as_ptr()));
            if pVirtualAlloc2.is_none() {
                pVirtualAlloc2 = GetProcAddress(h_dll, PCSTR::from_raw("VirtualAlloc2".as_ptr()));
            }
            FreeLibrary(h_dll);
        }
    }
}
