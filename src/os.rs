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

use crate::mimalloc_types::{MI_KiB, MI_MiB};

// page size (initialized properly in `os_init`)
pub static OS_PAGE_SIZE: AtomicU32 = AtomicU32::new(4096);

// minimal allocation granularity
pub static OS_ALLOC_GRANULARITY: AtomicU32 = AtomicU32::new(4096);

// if non-zero, use large page allocation
pub static LARGE_OS_PAGE_SIZE: AtomicU32 = AtomicU32::new(0);

/* -----------------------------------------------------------
  OS aligned allocation with an offset. This is used
  for large alignments > MI_ALIGNMENT_MAX. We use a large mimalloc
  page where the object can be aligned at an offset from the start of the segment.
  As we may need to overallocate, we need to free such pointers using `mi_free_aligned`
  to use the actual start of the memory region.
----------------------------------------------------------- */

fn _mi_os_alloc_aligned_offset(
    size: usize,
    alignment: usize,
    offset: usize,
    commit: bool,
    large: *mut bool, /*mi_stats_t* tld_stats */
) -> *mut c_void {
    debug_assert!(alignment % _mi_os_page_size() == 0);
    if offset == 0 {
        // regular aligned allocation
        return _mi_os_alloc_aligned(size, alignment, commit, large);
    }
    // } else {
    //     // overallocate to align at an offset
    //     const size_t extra = _mi_align_up(offset, alignment) - offset;
    //     const size_t oversize = size + extra;
    //     void* start = _mi_os_alloc_aligned(oversize, alignment, commit, large, tld_stats);
    //     if (start == NULL) return NULL;
    //     void* p = (uint8_t*)start + extra;
    //     mi_assert(_mi_is_aligned((uint8_t*)p + offset, alignment));
    //     // decommit the overallocation at the start
    //     if (commit && extra > _mi_os_page_size()) {
    //       _mi_os_decommit(start, extra, tld_stats);
    //     }
    //     return p;
    // }

    ptr::null_mut()
}

fn _mi_os_alloc_aligned(
    size: usize,
    alignment: usize,
    commit: bool,
    large: *mut bool, /* , mi_stats_t* tld_stats*/
) -> *mut c_void {
    if size == 0 {
        return ptr::null_mut();
    }
    let size = _mi_os_good_alloc_size(size);
    let alignment = _mi_align_up(alignment, _mi_os_page_size());
    let mut allow_large = false;
    if !large.is_null() {
        unsafe {
            allow_large = *large;
            large.write(false);
        }
    }

    if large.is_null() {
        mi_os_mem_alloc_aligned(size, alignment, commit, allow_large, &mut allow_large)
    } else {
        mi_os_mem_alloc_aligned(size, alignment, commit, allow_large, large)
    }
}

// OS (small) page size
fn _mi_os_page_size() -> usize {
    OS_PAGE_SIZE.load(Ordering::Relaxed) as usize
}

// Align upwards
fn _mi_align_up(sz: usize, alignment: usize) -> usize {
    debug_assert!(alignment != 0);
    let mask = alignment - 1;
    if (alignment & mask) == 0 {
        // power of two?
        (sz + mask) & !mask
    } else {
        ((sz + mask) / alignment) * alignment
    }
}

// round to a good OS allocation size (bounded by max 12.5% waste)
fn _mi_os_good_alloc_size(size: usize) -> usize {
    let align_size;
    if size < 512 * MI_KiB as usize {
        align_size = _mi_os_page_size();
    } else if size < 2 * MI_MiB as usize {
        align_size = 64 * MI_KiB as usize;
    } else if size < 8 * MI_MiB as usize {
        align_size = 256 * MI_KiB as usize;
    } else if size < 32 * MI_MiB as usize {
        align_size = MI_MiB as usize;
    } else {
        align_size = 4 * MI_MiB as usize;
    }
    if size >= (usize::MAX - align_size) {
        return size; // possible overflow?
    }
    _mi_align_up(size, align_size)
}

// Primitive aligned allocation from the OS.
// This function guarantees the allocated memory is aligned.
fn mi_os_mem_alloc_aligned(
    size: usize,
    alignment: usize,
    commit: bool,
    allow_large: bool,
    is_large: *mut bool, /*, mi_stats_t* stats*/
) -> *mut c_void {
    let allow_large = if !commit { false } else { allow_large };
    // check alignment is power of 2
    if !(alignment >= _mi_os_page_size() && ((alignment & (alignment - 1)) == 0)) {
        return ptr::null_mut();
    }
    let size = _mi_align_up(size, alignment);

    let mut p = mi_os_mem_alloc(size, alignment, commit, allow_large);
    if p.is_null() {
        return ptr::null_mut();
    }

    // if not aligned, free it, overallocate, and unmap around it
    if p as usize & alignment != 0 {
        mi_os_mem_free(p, size, commit);
        // TODO error log here
        if size >= (usize::MAX - alignment) {
            // overflow
            // TODO error log here
            return ptr::null_mut();
        }
        let over_size = size + alignment;

        if cfg!(Win32) {
            // over-allocate uncommitted (virtual) memory
            p = mi_os_mem_alloc(over_size, 0, false, false);
            if p.is_null() {
                return ptr::null_mut();
            }

            // set p to the aligned part in the full region
            // note: this is dangerous on Windows as VirtualFree needs the actual region pointer
            // but in mi_os_mem_free we handle this (hopefully exceptional) situation.
            p = mi_align_up_ptr(p, alignment);

            if commit {
                _mi_os_commit(p, over_size, ptr::null_mut());
            }
        }
    }

    debug_assert!(p.is_null() || (!p.is_null() && (p as usize % alignment) == 0));

    p
}

fn _mi_os_commit(
    addr: *mut c_void,
    size: usize,
    is_zero: *mut bool, /*, mi_stats_t* tld_stats */
) -> bool {
    // MI_UNUSED(tld_stats);
    // mi_stats_t * stats = &_mi_stats_main;
    mi_os_commitx(addr, size, true, false /* liberal */, is_zero)
    //true
}

fn mi_os_commitx(
    addr: *mut c_void,
    size: usize,
    commit: bool,
    conservative: bool,
    is_zero: *mut bool, /*, mi_stats_t* stats */
) -> bool {
    true
}

fn mi_align_up_ptr(p: *mut c_void, alignment: usize) -> *mut c_void {
    _mi_align_up(p as usize, alignment) as *mut c_void
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

pub fn _mi_os_init() {
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

/* -----------------------------------------------------------
  Free memory
-------------------------------------------------------------- */

fn mi_os_mem_free(
    addr: *mut c_void,
    size: usize,
    was_committed: bool, /* , mi_stats_t* stats*/
) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use std::ptr;

    use crate::os::_mi_os_alloc_aligned_offset;

    use super::{_mi_align_up, _mi_os_good_alloc_size};

    #[test]
    fn test_mi_os_alloc_aligned_offset() {
        let ptr = _mi_os_alloc_aligned_offset(0, 0, 0, false, ptr::null_mut());
        assert!(ptr.is_null());
    }

    #[test]
    fn test_mi_os_good_alloc_size() {
        let res = _mi_os_good_alloc_size(23);
        println!("res: {res}");
    }

    #[test]
    fn test_mi_align_up() {
        assert_eq!(_mi_align_up(17, 7), 21);
        assert_eq!(_mi_align_up(17, 6), 18);
        assert_eq!(_mi_align_up(17, 4), 20);
    }
}
