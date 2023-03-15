use std::ptr;

use libc::c_void;

use crate::mimalloc_types::{MiArenaIdT, MiCommitMask, MiOsTLD, MiSegment};

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
    return mi_segment_cache_pop_ex(
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
    );
}

pub fn _mi_segment_map_allocated_at(segment: *const MiSegment) {
    // size_t bitidx;
    // size_t index = mi_segment_map_index_of(segment, &bitidx);
    // mi_assert_internal(index <= MI_SEGMENT_MAP_WSIZE);
    // if (index==MI_SEGMENT_MAP_WSIZE) return;
    // uintptr_t mask = mi_atomic_load_relaxed(&mi_segment_map[index]);
    // uintptr_t newmask;
    // do {
    //   newmask = (mask | ((uintptr_t)1 << bitidx));
    // } while (!mi_atomic_cas_weak_release(&mi_segment_map[index], &mask, newmask));
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
    // if (p == NULL) return NULL;
    // mi_segment_t* segment = _mi_ptr_segment(p);
    // mi_assert_internal(segment != NULL);
    // size_t bitidx;
    // size_t index = mi_segment_map_index_of(segment, &bitidx);
    // // fast path: for any pointer to valid small/medium/large object or first MI_SEGMENT_SIZE in huge
    // const uintptr_t mask = mi_atomic_load_relaxed(&mi_segment_map[index]);
    // if mi_likely((mask & ((uintptr_t)1 << bitidx)) != 0) {
    //   return segment; // yes, allocated by us
    // }
    // if (index==MI_SEGMENT_MAP_WSIZE) return NULL;

    // // TODO: maintain max/min allocated range for efficiency for more efficient rejection of invalid pointers?

    // // search downwards for the first segment in case it is an interior pointer
    // // could be slow but searches in MI_INTPTR_SIZE * MI_SEGMENT_SIZE (512MiB) steps trough
    // // valid huge objects
    // // note: we could maintain a lowest index to speed up the path for invalid pointers?
    // size_t lobitidx;
    // size_t loindex;
    // uintptr_t lobits = mask & (((uintptr_t)1 << bitidx) - 1);
    // if (lobits != 0) {
    //   loindex = index;
    //   lobitidx = mi_bsr(lobits);    // lobits != 0
    // }
    // else if (index == 0) {
    //   return NULL;
    // }
    // else {
    //   mi_assert_internal(index > 0);
    //   uintptr_t lomask = mask;
    //   loindex = index;
    //   do {
    //     loindex--;
    //     lomask = mi_atomic_load_relaxed(&mi_segment_map[loindex]);
    //   } while (lomask != 0 && loindex > 0);
    //   if (lomask == 0) return NULL;
    //   lobitidx = mi_bsr(lomask);    // lomask != 0
    // }
    // mi_assert_internal(loindex < MI_SEGMENT_MAP_WSIZE);
    // // take difference as the addresses could be larger than the MAX_ADDRESS space.
    // size_t diff = (((index - loindex) * (8*MI_INTPTR_SIZE)) + bitidx - lobitidx) * MI_SEGMENT_SIZE;
    // segment = (mi_segment_t*)((uint8_t*)segment - diff);

    // if (segment == NULL) return NULL;
    // mi_assert_internal((void*)segment < p);
    // bool cookie_ok = (_mi_ptr_cookie(segment) == segment->cookie);
    // mi_assert_internal(cookie_ok);
    // if mi_unlikely(!cookie_ok) return NULL;
    // if (((uint8_t*)segment + mi_segment_size(segment)) <= (uint8_t*)p) return NULL; // outside the range
    // mi_assert_internal(p >= (void*)segment && (uint8_t*)p < (uint8_t*)segment + mi_segment_size(segment));
    // return segment;

    ptr::null_mut()
}

// Is this a valid pointer in our heap?
fn mi_is_valid_pointer(p: *const c_void) -> bool {
    return !_mi_segment_of(p).is_null();
}

fn mi_is_in_heap_region(p: *const c_void) -> bool {
    return mi_is_valid_pointer(p);
}
