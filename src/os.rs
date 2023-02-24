use std::{
    ffi::c_void,
    mem::transmute,
    ptr,
    sync::atomic::{AtomicU32, AtomicUsize, Ordering},
};

use windows::{
    core::PCSTR,
    s,
    Win32::{
        Foundation::{self, HINSTANCE},
        System::{
            LibraryLoader::{FreeLibrary, GetProcAddress, LoadLibraryA, LoadLibraryW},
            Memory::{
                self, VirtualAlloc, MEM_ADDRESS_REQUIREMENTS, MEM_COMMIT, MEM_EXTENDED_PARAMETER,
                MEM_LARGE_PAGES, MEM_RESERVE, PAGE_PROTECTION_FLAGS, VIRTUAL_ALLOCATION_TYPE,
            },
            SystemInformation::{GetSystemInfo, SYSTEM_INFO},
        },
    },
};

use crate::mimalloc_types::{BitfieldUnit, MI_KiB, MI_MiB};

// page size (initialized properly in `os_init`)
pub static OS_PAGE_SIZE: AtomicU32 = AtomicU32::new(4096);

// minimal allocation granularity
pub static OS_ALLOC_GRANULARITY: AtomicU32 = AtomicU32::new(4096);

// if non-zero, use large page allocation
pub static LARGE_OS_PAGE_SIZE: AtomicU32 = AtomicU32::new(0);

pub type ULONG = ::std::os::raw::c_ulong;

// struct MiMemAddressRequirements {
//     lowest_starting_address: *mut c_void,
//     highest_ending_address: *mut c_void,
//     alignment: usize,
// }

// #[repr(transparent)]
// struct MiMemAddressRequirements(MEM_ADDRESS_REQUIREMENTS);

//  struct MiMemExtendedParameter {
//     Type: struct { u64 Type: 8; DWORD64 Reserved : 56; },
//     union  { DWORD64 ULong64; PVOID Pointer; SIZE_T Size; HANDLE Handle; DWORD ULong; } Arg;
//   } MI_MEM_EXTENDED_PARAMETER;

// struct U<const N:usize>;
// trait UN { type Int; }
// impl UN for U<32> { type Int = u32; }
// impl UN for U<64> { type Int = u64; }
// type UInt<const N:usize> = <U<N> as UN>::Int;
// let a: UInt<{8*std::mem::size_of::<[u8; 8]>()}> = 0u64;
// a

// #[cfg(windows)]
// #[repr(C)]
// enum MiMemExtendedParameterType {
//     MiMemExtendedParameterInvalidType = 0,
//     MiMemExtendedParameterAddressRequirements,
//     MiMemExtendedParameterNumaNode,
//     MiMemExtendedParameterPartitionHandle,
//     MiMemExtendedParameterUserPhysicalHandle,
//     MiMemExtendedParameterAttributeFlags,
//     MiMemExtendedParameterMax,
// }

// type a = MEM_EXTENDED_PARAMETER;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct MiMemExtendedParameter {
    pub ty: MiMemExtendedParameterBindgenTy1,
    pub arg: MiMemExtendedParameterBindgenTy2,
}

// #[repr(transparent)]
// pub struct MiMemExtendedParameter(MEM_EXTENDED_PARAMETER);

#[repr(C)]
// #[repr(align(8))]
#[derive(Debug, Copy, Clone)]
pub struct MiMemExtendedParameterBindgenTy1 {
    pub _bitfield_1: BitfieldUnit<[u8; 8usize], u64>,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union MiMemExtendedParameterBindgenTy2 {
    pub ULong64: DWORD64,
    pub Pointer: PVOID,
    pub Size: SIZE_T,
    pub Handle: HANDLE,
    pub ULong: DWORD,
    _bindgen_union_align: u64,
}

impl MiMemExtendedParameterBindgenTy1 {
    #[inline]
    pub fn Type(&self) -> DWORD64 {
        unsafe { ::std::mem::transmute(self._bitfield_1.get(0usize, 8u8) as u64) }
    }
    #[inline]
    pub fn set_Type(&mut self, val: DWORD64) {
        unsafe {
            let val: u64 = ::std::mem::transmute(val);
            self._bitfield_1.set(0usize, 8u8, val as u64)
        }
    }
    #[inline]
    pub fn Reserved(&self) -> DWORD64 {
        unsafe { ::std::mem::transmute(self._bitfield_1.get(8usize, 56u8) as u64) }
    }
    #[inline]
    pub fn set_Reserved(&mut self, val: DWORD64) {
        unsafe {
            let val: u64 = ::std::mem::transmute(val);
            self._bitfield_1.set(8usize, 56u8, val as u64)
        }
    }
    #[inline]
    pub fn new_bitfield_1(Type: DWORD64, Reserved: DWORD64) -> BitfieldUnit<[u8; 8usize], u64> {
        let mut __bindgen_bitfield_unit: BitfieldUnit<[u8; 8usize], u64> = Default::default();
        __bindgen_bitfield_unit.set(0usize, 8u8, {
            let Type: u64 = unsafe { ::std::mem::transmute(Type) };
            Type as u64
        });
        __bindgen_bitfield_unit.set(8usize, 56u8, {
            let Reserved: u64 = unsafe { ::std::mem::transmute(Reserved) };
            Reserved as u64
        });
        __bindgen_bitfield_unit
    }
}

pub type DWORD64 = ::std::os::raw::c_ulonglong;
pub type PVOID = *mut ::std::os::raw::c_void;
pub type DWORD = ::std::os::raw::c_ulong;
pub type ULONG_PTR = ::std::os::raw::c_ulonglong;
pub type SIZE_T = ULONG_PTR;
pub type HANDLE = *mut ::std::os::raw::c_void;
pub type PHANDLE = *mut HANDLE;

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

    let mut p = mi_os_mem_alloc(size, alignment, commit, allow_large, is_large);
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
            p = mi_os_mem_alloc(over_size, 0, false, false, is_large);
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
    allow_large: bool,
    is_large: *mut bool, /*mi_stats_t* stats*/
) -> *mut c_void {
    debug_assert!(size > 0 && (size & _mi_os_page_size()) == 0);
    if size == 0 {
        return ptr::null_mut();
    }
    let allow_large = if !commit { false } else { allow_large };
    let try_alignment = if try_alignment == 0 { 1 } else { try_alignment };
    let mut p = ptr::null_mut();

    if cfg!(Win32) {
        let mut flags = MEM_RESERVE;
        if commit {
            flags |= MEM_COMMIT;
        }

        p = mi_win_virtual_alloc(
            ptr::null_mut(),
            size,
            try_alignment,
            flags.0,
            false,
            allow_large,
            is_large,
        );
    }

    p
}

fn use_large_os_page(size: usize, alignment: usize) -> bool {
    // // if we have access, check the size and alignment requirements
    // if LARGE_OS_PAGE_SIZE == 0 || !mi_option_is_enabled(mi_option_large_os_pages) {
    //     return false;
    // }
    (size as u32 % LARGE_OS_PAGE_SIZE.load(Ordering::Relaxed)) == 0
        && (alignment as u32 % LARGE_OS_PAGE_SIZE.load(Ordering::Relaxed)) == 0
}

fn mi_win_virtual_alloc(
    addr: *mut c_void,
    size: usize,
    try_alignment: usize,
    flags: u32,
    large_only: bool,
    allow_large: bool,
    is_large: *mut bool,
) -> *mut c_void {
    debug_assert!(!(large_only && !allow_large));
    static large_page_try_ok: AtomicUsize = AtomicUsize::new(0);

    let mut p = ptr::null_mut();
    // Try to allocate large OS pages (2MiB) if allowed or required.
    if (large_only || use_large_os_page(size, try_alignment))
        && allow_large
        && (flags & MEM_COMMIT.0) != 0
        && (flags & MEM_RESERVE.0) != 0
    {
        let try_ok = large_page_try_ok.load(Ordering::Acquire);
        if !large_only && try_ok > 0 {
            if large_page_try_ok
                .compare_exchange(try_ok, try_ok - 1, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
            {}
        } else {
            // large OS pages must always reserve and commit.
            unsafe {
                is_large.write(true);
            }

            p = mi_win_virtual_allocx(addr, size, try_alignment, flags | MEM_LARGE_PAGES.0);

            if large_only {
                return p;
            }

            // fall back to non-large page allocation on error (`p == NULL`).
            if p.is_null() {
                large_page_try_ok.store(10, Ordering::Release);
            }
        }
    }

    // Fall back to regular page allocation
    if p.is_null() {
        unsafe { is_large.write((flags & MEM_LARGE_PAGES.0) != 0) };
        p = mi_win_virtual_allocx(addr, size, try_alignment, flags);
    }

    if p.is_null() {
        // TODO error log here
    }

    p
}

#[cfg(windows)]
fn mi_win_virtual_allocx(
    addr: *mut c_void,
    size: usize,
    try_alignment: usize,
    flags: u32,
) -> *mut c_void {
    use windows::Win32::System::{
        Memory::{MemExtendedParameterAddressRequirements, PAGE_READWRITE},
        Threading::GetCurrentProcess,
    };

    if cfg!(target_pointer_width = "64") {
        // on 64-bit systems, try to use the virtual address area after 2TiB for 4MiB aligned allocations
        if addr.is_null() {
            let hint = mi_os_get_aligned_hint(try_alignment, size);
            if !hint.is_null() {
                let p = unsafe {
                    VirtualAlloc(
                        Some(hint),
                        size,
                        VIRTUAL_ALLOCATION_TYPE(flags),
                        PAGE_READWRITE,
                    )
                };

                if !p.is_null() {
                    return p;
                }
                // TODO error log here
            }
        }
    }

    // on modern Windows try use VirtualAlloc2 for aligned allocation
    if try_alignment > 1
        && (try_alignment % _mi_os_page_size()) == 0
        && unsafe { P_VIRTUAL_ALLOC2.is_some() }
    {
        // let mut reqs = MiMemAddressRequirements {
        //     lowest_starting_address: ptr::null_mut(),
        //     highest_ending_address: ptr::null_mut(),
        //     alignment: try_alignment,
        // };

        let mut reqs = MEM_ADDRESS_REQUIREMENTS {
            Alignment: try_alignment,
            ..Default::default()
        };

        let mut param = MiMemExtendedParameter {
            ty: MiMemExtendedParameterBindgenTy1 {
                _bitfield_1: BitfieldUnit::new([0; 8]),
            },
            arg: MiMemExtendedParameterBindgenTy2 {
                Pointer: ptr::addr_of_mut!(reqs).cast(),
            },
        };
        param
            .ty
            .set_Type(MemExtendedParameterAddressRequirements.0 as u64);

        let p = unsafe {
            P_VIRTUAL_ALLOC2.unwrap()(
                GetCurrentProcess(),
                addr,
                size as SIZE_T,
                flags,
                PAGE_READWRITE.0,
                ptr::addr_of_mut!(param).cast(),
                1,
            )
        };
        if !p.is_null() {
            return p;
        }
        println!("virtual_alloc2 failed");

        // _mi_warning_message("unable to allocate aligned OS memory (%zu bytes, error code: 0x%x, address: %p, alignment: %zu, flags: 0x%x)\n", size, GetLastError(), addr, try_alignment, flags);
        // fall through on error
    }
    // last resort
    unsafe {
        VirtualAlloc(
            Some(addr),
            size,
            VIRTUAL_ALLOCATION_TYPE(flags),
            PAGE_READWRITE,
        )
    }
}

fn mi_os_get_aligned_hint(try_alignment: usize, size: usize) -> *const c_void {
    ptr::null_mut()
}

type PVirtualAlloc2 = unsafe extern "stdcall" fn(
    Foundation::HANDLE,
    PVOID,
    SIZE_T,
    ULONG,
    ULONG,
    *mut MEM_EXTENDED_PARAMETER,
    ULONG,
) -> PVOID;

pub static mut P_VIRTUAL_ALLOC2: Option<PVirtualAlloc2> = None;

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
    let h_dll = unsafe { LoadLibraryA(s!("kernelbase.dll")) };
    if let Ok(h_dll) = h_dll {
        // use VirtualAlloc2FromApp if possible as it is available to Windows store apps
        unsafe {
            P_VIRTUAL_ALLOC2 = transmute(GetProcAddress(h_dll, s!("VirtualAlloc2FromApp")));
            if P_VIRTUAL_ALLOC2.is_none() {
                P_VIRTUAL_ALLOC2 = transmute(GetProcAddress(h_dll, s!("VirtualAlloc2")));
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
    use std::{mem::size_of, os::raw::c_void, ptr};

    use windows::Win32::System::{
        Memory::{MEM_COMMIT, MEM_EXTENDED_PARAMETER, MEM_RESERVE},
        Threading::GetCurrentProcess,
    };

    use crate::os::{
        MiMemExtendedParameter, MiMemExtendedParameterBindgenTy2, _mi_os_alloc_aligned_offset,
    };

    use super::{
        _mi_align_up, _mi_os_good_alloc_size, _mi_os_init, _mi_os_page_size, mi_win_virtual_allocx,
    };

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

    #[test]
    fn test_mi_win_virtual_allocx() {
        let addr = ptr::null_mut();
        _mi_os_init();
        assert!(
            !mi_win_virtual_allocx(addr, 33554432, 33554432, (MEM_COMMIT | MEM_RESERVE).0)
                .is_null()
        );
    }

    #[test]
    fn test_mi_os_page_size() {
        let page_size = _mi_os_page_size();
        println!("page size: {page_size}");
    }

    #[test]
    fn test_memory_layout() {
        assert_eq!(
            size_of::<MEM_EXTENDED_PARAMETER>(),
            size_of::<MiMemExtendedParameter>()
        );
    }

    // #[test]
    // fn bindgen_test_layout_MiMemExtendedParameter__bindgen_ty_2() {
    //     assert_eq!(
    //         ::std::mem::size_of::<MiMemExtendedParameterBindgenTy2>(),
    //         8usize,
    //         concat!(
    //             "Size of: ",
    //             stringify!(MiMemExtendedParameter__bindgen_ty_2)
    //         )
    //     );
    //     assert_eq!(
    //         ::std::mem::align_of::<MiMemExtendedParameterBindgenTy2>(),
    //         8usize,
    //         concat!(
    //             "Alignment of ",
    //             stringify!(MiMemExtendedParameter__bindgen_ty_2)
    //         )
    //     );
    //     assert_eq!(
    //         unsafe {
    //             &(*(::std::ptr::null::<MiMemExtendedParameterBindgenTy2>())).ULong64 as *const _
    //                 as usize
    //         },
    //         0usize,
    //         concat!(
    //             "Offset of field: ",
    //             stringify!(MiMemExtendedParameter__bindgen_ty_2),
    //             "::",
    //             stringify!(ULong64)
    //         )
    //     );
    //     assert_eq!(
    //         unsafe {
    //             &(*(::std::ptr::null::<MiMemExtendedParameterBindgenTy2>())).Pointer as *const _
    //                 as usize
    //         },
    //         0usize,
    //         concat!(
    //             "Offset of field: ",
    //             stringify!(MiMemExtendedParameter__bindgen_ty_2),
    //             "::",
    //             stringify!(Pointer)
    //         )
    //     );
    //     assert_eq!(
    //         unsafe {
    //             &(*(::std::ptr::null::<MiMemExtendedParameterBindgenTy2>())).Size as *const _
    //                 as usize
    //         },
    //         0usize,
    //         concat!(
    //             "Offset of field: ",
    //             stringify!(MiMemExtendedParameter__bindgen_ty_2),
    //             "::",
    //             stringify!(Size)
    //         )
    //     );
    //     assert_eq!(
    //         unsafe {
    //             &(*(::std::ptr::null::<MiMemExtendedParameterBindgenTy2>())).Handle as *const _
    //                 as usize
    //         },
    //         0usize,
    //         concat!(
    //             "Offset of field: ",
    //             stringify!(MiMemExtendedParameter__bindgen_ty_2),
    //             "::",
    //             stringify!(Handle)
    //         )
    //     );
    //     assert_eq!(
    //         unsafe {
    //             &(*(::std::ptr::null::<MiMemExtendedParameterBindgenTy2>())).ULong as *const _
    //                 as usize
    //         },
    //         0usize,
    //         concat!(
    //             "Offset of field: ",
    //             stringify!(MiMemExtendedParameter__bindgen_ty_2),
    //             "::",
    //             stringify!(ULong)
    //         )
    //     );
    // }
    // #[test]
    // fn bindgen_test_layout_MiMemExtendedParameter() {
    //     assert_eq!(
    //         ::std::mem::size_of::<MiMemExtendedParameter>(),
    //         16usize,
    //         concat!("Size of: ", stringify!(MiMemExtendedParameter))
    //     );
    //     assert_eq!(
    //         ::std::mem::align_of::<MiMemExtendedParameter>(),
    //         8usize,
    //         concat!("Alignment of ", stringify!(MiMemExtendedParameter))
    //     );
    //     assert_eq!(
    //         unsafe { &(*(::std::ptr::null::<MiMemExtendedParameter>())).ty as *const _ as usize },
    //         0usize,
    //         concat!(
    //             "Offset of field: ",
    //             stringify!(MiMemExtendedParameter),
    //             "::",
    //             stringify!(Type)
    //         )
    //     );
    //     assert_eq!(
    //         unsafe { &(*(::std::ptr::null::<MiMemExtendedParameter>())).arg as *const _ as usize },
    //         8usize,
    //         concat!(
    //             "Offset of field: ",
    //             stringify!(MiMemExtendedParameter),
    //             "::",
    //             stringify!(Arg)
    //         )
    //     );
    // }
    // #[test]
    // fn bindgen_test_layout_MiMemExtendedParameter__bindgen_ty_1() {
    //     assert_eq!(
    //         ::std::mem::size_of::<MiMemExtendedParameterBindgenTy1>(),
    //         8usize,
    //         concat!(
    //             "Size of: ",
    //             stringify!(MiMemExtendedParameter__bindgen_ty_1)
    //         )
    //     );
    //     assert_eq!(
    //         ::std::mem::align_of::<MiMemExtendedParameterBindgenTy1>(),
    //         8usize,
    //         concat!(
    //             "Alignment of ",
    //             stringify!(MiMemExtendedParameter__bindgen_ty_1)
    //         )
    //     );
    // }
}
