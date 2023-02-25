use std::{ptr, sync::atomic::AtomicU32};

use crate::mimalloc_types::MiCommitMask;
use crate::mimalloc_types::MiOption::MiOptionEagerCommitDelay;
use crate::options::mi_option_is_enabled;
use crate::{
    init::_mi_current_thread_count,
    mimalloc_types::{MiArenaIdT, MiHeap, MiOsTLD, MiPage, MiSegment, MiSegmentsTLD},
    options::mi_option_get,
};

// Allocate a segment from the OS aligned to `MI_SEGMENT_SIZE` .
pub fn mi_segment_alloc(
    required: usize,
    page_alignment: usize,
    req_arena_id: MiArenaIdT,
    tld: *mut MiSegmentsTLD,
    os_tld: *mut MiOsTLD,
    huge_page: *mut *mut MiPage,
) -> *mut MiSegment {
    debug_assert!((required == 0 && huge_page.is_null()) || (required > 0 && huge_page.is_null()));

    // calculate needed sizes first
    let mut info_slices: usize = 0;
    let mut pre_size: usize = 0;
    let segment_slices = mi_segment_calculate_slices(
        required,
        ptr::addr_of_mut!(pre_size),
        ptr::addr_of_mut!(info_slices),
    );

    // Commit eagerly only if not the first N lazy segments (to reduce impact of many threads that allocate just a little)
    unsafe {
        let eager_delay: bool = // !_mi_os_has_overcommit() &&             // never delay on overcommit systems
    _mi_current_thread_count() > 1 &&       // do not delay for the first N threads
    (*tld).count < mi_option_get(MiOptionEagerCommitDelay) as u64;

        let eager: bool = !eager_delay && mi_option_is_enabled(MiOptionEagerCommitDelay);
        let commit = eager || (required > 0);
        let is_zero = false;
    }

    let commit_mask: MiCommitMask;
    let decommit_mask: MiCommitMask;
    // mi_commit_mask_create_empty(&commit_mask);
    // mi_commit_mask_create_empty(&decommit_mask);

    ptr::null_mut()
}

fn mi_segment_os_alloc(
    required: usize,
    page_alignment: usize, /* , bool eager_delay, mi_arena_id_t req_arena_id,
                           size_t* psegment_slices, size_t* ppre_size, size_t* pinfo_slices,
                           mi_commit_mask_t* pcommit_mask, mi_commit_mask_t* pdecommit_mask,
                           bool* is_zero, bool* pcommit, mi_segments_tld_t* tld, mi_os_tld_t* os_tld*/
) -> *mut MiSegment {
    // simply get from os
    ptr::null_mut()
}

/* -----------------------------------------------------------
   Reclaim or allocate
----------------------------------------------------------- */
fn mi_segment_reclaim_or_alloc(
    heap: MiHeap,
    needed_slices: usize,
    block_size: usize,
    tld: *mut MiSegmentsTLD,
    os_tld: *mut MiOsTLD,
) -> *mut MiSegment {
    ptr::null_mut()
}

fn mi_segment_calculate_slices(
    required: usize,
    pre_size: *mut usize,
    info_slices: *mut usize,
) -> usize {
    0
}
