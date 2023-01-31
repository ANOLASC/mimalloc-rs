use std::{ffi::c_void, ptr};

use windows::Win32::System::Memory::{
    self, VirtualAlloc, PAGE_PROTECTION_FLAGS, VIRTUAL_ALLOCATION_TYPE,
};

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
