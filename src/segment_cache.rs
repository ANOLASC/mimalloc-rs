use std::{
    ptr::{self, addr_of},
    sync::atomic::AtomicUsize,
};

use crate::mimalloc_internal::mi_segment_size;

use libc::{c_void, size_t, uintptr_t};

use crate::{
    mimalloc_internal::{_mi_ptr_cookie, _mi_ptr_segment, mi_bsr},
    mimalloc_types::{
        MiArenaIdT, MiCommitMask, MiOsTLD, MiSegment, MI_INTPTR_BITS, MI_INTPTR_SIZE,
        MI_SEGMENT_SIZE,
    },
};

// #if (MI_INTPTR_SIZE==8)
// TODO support only 64bit for now
const MI_MAX_ADDRESS: usize = 20 << 40; // 20TB
                                        // #else
                                        // #define MI_MAX_ADDRESS    ((size_t)2 << 30)   // 2Gb
                                        // #endif

const MI_SEGMENT_MAP_BITS: usize = MI_MAX_ADDRESS / MI_SEGMENT_SIZE;
const MI_SEGMENT_MAP_SIZE: usize = MI_SEGMENT_MAP_BITS / 8;
const MI_SEGMENT_MAP_WSIZE: usize = MI_SEGMENT_MAP_SIZE / MI_INTPTR_SIZE;

// static _Atomic(uintptr_t) mi_segment_map[MI_SEGMENT_MAP_WSIZE + 1];  // 2KiB per TB with 64MiB segments
const INIT: AtomicUsize = AtomicUsize::new(0);
static mi_segment_map: [AtomicUsize; MI_SEGMENT_MAP_WSIZE + 1] = [INIT; MI_SEGMENT_MAP_WSIZE + 1];

pub fn _mi_segment_cache_pop(
    size: usize,
    commit_mask: *mut MiCommitMask,
    decommit_mask: *mut MiCommitMask,
    large: *mut bool,
    is_pinned: *mut bool,
    is_zero: *mut bool,
    _req_arena_id: MiArenaIdT,
    memid: *mut usize,
    tld: *mut MiOsTLD,
) -> *mut c_void {
    mi_segment_cache_pop_ex(
        false,
        size,
        commit_mask,
        decommit_mask,
        large,
        is_pinned,
        is_zero,
        _req_arena_id,
        memid,
        tld,
    )
}

fn mi_segment_map_index_of(segment: *const MiSegment, bitidx: *mut size_t) -> size_t {
    debug_assert!(unsafe { _mi_ptr_segment(segment.add(1).cast()) == segment.cast_mut() }); // is it aligned on MI_SEGMENT_SIZE?
    if segment as usize >= MI_MAX_ADDRESS {
        unsafe {
            *bitidx = 0;
        }
        return MI_SEGMENT_MAP_WSIZE;
    } else {
        let segindex = (segment) as usize / MI_SEGMENT_SIZE;
        unsafe {
            *bitidx = segindex % MI_INTPTR_BITS;
        }
        let mapindex = segindex / MI_INTPTR_BITS;
        debug_assert!(mapindex < MI_SEGMENT_MAP_WSIZE);
        return mapindex;
    }
}

pub fn _mi_segment_map_allocated_at(segment: *const MiSegment) {
    let mut bitidx: size_t = 0;
    let index: size_t = mi_segment_map_index_of(segment, &mut bitidx);
    debug_assert!(index <= MI_SEGMENT_MAP_WSIZE);
    if index == MI_SEGMENT_MAP_WSIZE {
        return;
    }
    let mask = mi_segment_map[index].load(std::sync::atomic::Ordering::Relaxed);
    let mut newmask: uintptr_t;
    // do {
    //   newmask = (mask | ((uintptr_t)1 << bitidx));
    // } while (!mi_atomic_cas_weak_release(&mi_segment_map[index], &mask, newmask));

    loop {
        newmask = mask | (1 << bitidx);

        if mi_segment_map[index]
            .compare_exchange_weak(
                // Strange here
                addr_of!(mask) as usize,
                newmask,
                std::sync::atomic::Ordering::Release,
                std::sync::atomic::Ordering::Relaxed,
            )
            .is_ok()
        {
            break;
        }
    }
}

fn mi_segment_cache_pop_ex(
    all_suitable: bool,
    size: usize,
    commit_mask: *mut MiCommitMask,
    decommit_mask: *mut MiCommitMask,
    large: *mut bool,
    is_pinned: *mut bool,
    is_zero: *mut bool,
    _req_arena_id: MiArenaIdT,
    memid: *mut usize,
    tld: *mut MiOsTLD,
) -> *mut c_void {
    ptr::null_mut()
}

// Determine the segment belonging to a pointer or NULL if it is not in a valid segment.
fn _mi_segment_of(p: *const c_void) -> *mut MiSegment {
    if p.is_null() {
        return ptr::null_mut();
    }
    let mut segment = _mi_ptr_segment(p);
    debug_assert!(!segment.is_null());
    let mut bitidx: usize = 0;
    let index = mi_segment_map_index_of(segment, &mut bitidx);
    // fast path: for any pointer to valid small/medium/large object or first MI_SEGMENT_SIZE in huge
    let mask = mi_segment_map[index].load(std::sync::atomic::Ordering::Relaxed);
    if (mask & (1 << bitidx)) != 0 {
        return segment; // yes, allocated by us
    }
    if index == MI_SEGMENT_MAP_WSIZE {
        return ptr::null_mut();
    }

    // // TODO: maintain max/min allocated range for efficiency for more efficient rejection of invalid pointers?

    // // search downwards for the first segment in case it is an interior pointer
    // // could be slow but searches in MI_INTPTR_SIZE * MI_SEGMENT_SIZE (512MiB) steps trough
    // // valid huge objects
    // // note: we could maintain a lowest index to speed up the path for invalid pointers?
    let mut lobitidx;
    let mut loindex;
    let lobits = mask & ((1 << bitidx) - 1);
    if lobits != 0 {
        loindex = index;
        lobitidx = mi_bsr(lobits); // lobits != 0
    } else if index == 0 {
        return ptr::null_mut();
    } else {
        debug_assert!(index > 0);
        let mut lomask = mask;
        loindex = index;

        loop {
            loindex -= 1;
            lomask = mi_segment_map[loindex].load(std::sync::atomic::Ordering::Relaxed);
            if lomask != 0 && loindex > 0 {
                continue;
            } else {
                break;
            }
        }

        if lomask == 0 {
            return ptr::null_mut();
        }
        lobitidx = mi_bsr(lomask); // lomask != 0
    }
    debug_assert!(loindex < MI_SEGMENT_MAP_WSIZE);
    // take difference as the addresses could be larger than the MAX_ADDRESS space.
    let diff = (((index - loindex) * (8 * MI_INTPTR_SIZE)) + bitidx - lobitidx) * MI_SEGMENT_SIZE;
    // segment = ((uint8_t*)segment - diff);
    segment = (segment as usize - diff) as *mut MiSegment;

    if segment.is_null() {
        return ptr::null_mut();
    }
    debug_assert!(segment.lt(&(p as *mut MiSegment)));
    let cookie_ok = _mi_ptr_cookie(segment as *const c_void) == unsafe { (*segment).cookie };
    debug_assert!(cookie_ok);
    if !cookie_ok {
        return ptr::null_mut();
    }
    if unsafe {
        ((segment as *mut u8).add(mi_segment_size(segment.cast_const()))) <= (p as *mut u8)
    } {
        return ptr::null_mut();
    } // outside the range
    debug_assert!(unsafe {
        p >= (segment as *const c_void)
            && (p as *const u8).lt(&(segment as *const u8).add(mi_segment_size(segment)))
    });

    segment
}

// Is this a valid pointer in our heap?
fn mi_is_valid_pointer(p: *const c_void) -> bool {
    !_mi_segment_of(p).is_null()
}

fn mi_is_in_heap_region(p: *const c_void) -> bool {
    mi_is_valid_pointer(p)
}
