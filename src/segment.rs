use std::mem::size_of;
use std::sync::atomic::Ordering;
use std::{ptr, sync::atomic::AtomicU32};

use libc::{c_void, memset};
use memoffset::offset_of;

use crate::mimalloc_internal::mi_commit_mask_create_empty;
use crate::mimalloc_types::MiOption::MiOptionEagerCommitDelay;
use crate::mimalloc_types::{MiCommitMask, MiSlice};
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
    let eager_delay: bool = // !_mi_os_has_overcommit() &&             // never delay on overcommit systems
    _mi_current_thread_count() > 1 &&       // do not delay for the first N threads
    unsafe { (*tld).count} < mi_option_get(MiOptionEagerCommitDelay) as u64;

    let eager: bool = !eager_delay && mi_option_is_enabled(MiOptionEagerCommitDelay);
    let commit = eager || (required > 0);
    let is_zero = false;

    let mut commit_mask = MiCommitMask { mask: [0; 8] };
    let mut decommit_mask = MiCommitMask { mask: [0; 8] };
    mi_commit_mask_create_empty(&mut commit_mask);
    mi_commit_mask_create_empty(&mut decommit_mask);

    // Allocate the segment from the OS
    let segment = mi_segment_os_alloc(
        required,
        page_alignment,
        // eager_delay,
        // req_arena_id,
        // &segment_slices,
        // &pre_size,
        // &info_slices,
        // &commit_mask,
        // &decommit_mask,
        // &is_zero,
        // &commit,
        // tld,
        // os_tld,
    );

    if segment.is_null() {
        return ptr::null_mut();
    }

    // zero the segment info? -- not always needed as it may be zero initialized from the OS
    unsafe {
        (*segment)
            .abandoned_next
            .store(ptr::null_mut(), Ordering::Release);
    }

    if !is_zero {
        let ofs = offset_of!(MiSegment, next);
        let prefix = offset_of!(MiSegment, slices) - ofs;
        unsafe {
            memset(
                (segment as usize + ofs) as *mut c_void,
                0,
                prefix + size_of::<MiSlice>() * (segment_slices + 1),
            ); // one more
        }
    }

    unsafe {
        (*segment).commit_mask = commit_mask; // on lazy commit, the initial part is always committed
                                              // (*segment).allow_decommit = mi_option_is_enabled(mi_option_allow_decommit)
                                              //  &&!(*segment).mem_is_pinned && !(*segment).mem_is_large;
        if (*segment).allow_decommit {
            (*segment).decommit_expire = 0; // don't decommit just committed memory // _mi_clock_now() + mi_option_get(mi_option_decommit_delay);
            (*segment).decommit_mask = decommit_mask;
            // debug_assert(mi_commit_mask_all_set(&segment).commit_mask, &(*segment).decommit_mask);
            // #if MI_DEBUG>2
            // let commit_needed = _mi_divide_up(info_slices * MI_SEGMENT_SLICE_SIZE, MI_COMMIT_SIZE);
            // let commit_needed_mask;
            // mi_commit_mask_create(0, commit_needed, &commit_needed_mask);
            // debug_assert(
            //     !mi_commit_mask_any_set(&segment).decommit_mask,
            //     &commit_needed_mask,
            // );
            // #endif
        }
    }

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
