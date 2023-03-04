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
