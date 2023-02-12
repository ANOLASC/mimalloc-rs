use std::{ptr, sync::atomic::AtomicU32};

use crate::mimalloc_types::{MiPage, MiSegment};

fn mi_segment_alloc(
    required: usize,
    page_alignment: usize,
    /* req_arena_id: mi_arena_id_t , mi_segments_tld_t* tld, mi_os_tld_t* os_tld,*/
    huge_page: *mut *mut MiPage,
) -> *mut MiSegment {
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
