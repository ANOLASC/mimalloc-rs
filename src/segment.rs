use std::mem::size_of;
use std::sync::atomic::Ordering;
use std::{ptr, sync::atomic::AtomicU32};

use libc::{c_void, memset};
use memoffset::offset_of;

use crate::arena::{self, _mi_arena_alloc_aligned};
use crate::mimalloc_internal::{
    _mi_divide_up, mi_commit_mask_create_empty, mi_commit_mask_create_full, mi_commit_mask_is_full,
};
use crate::mimalloc_types::MiOption::{self, MiOptionEagerCommitDelay};
use crate::mimalloc_types::{
    MiCommitMask, MiSlice, MI_COMMIT_MASK_BITS, MI_COMMIT_MASK_FIELD_BITS,
    MI_COMMIT_MASK_FIELD_COUNT, MI_COMMIT_SIZE, MI_HUGE_BLOCK_SIZE, MI_LARGE_OBJ_SIZE_MAX,
    MI_SECURE, MI_SEGMENT_ALIGN, MI_SEGMENT_SIZE, MI_SEGMENT_SLICE_SIZE,
};
use crate::options::{mi_option_get_clamp, mi_option_is_enabled};
use crate::os::{_mi_align_up, _mi_os_commit};
use crate::segment::MiOption::MiOptionMaxSegmentReclaim;
use crate::segment_cache::{_mi_segment_cache_pop, _mi_segment_map_allocated_at};
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
    let mut segment_slices = mi_segment_calculate_slices(
        required,
        ptr::addr_of_mut!(pre_size),
        ptr::addr_of_mut!(info_slices),
    );

    // Commit eagerly only if not the first N lazy segments (to reduce impact of many threads that allocate just a little)
    let eager_delay: bool = // !_mi_os_has_overcommit() &&             // never delay on overcommit systems
    _mi_current_thread_count() > 1 &&       // do not delay for the first N threads
    unsafe { (*tld).count} < mi_option_get(MiOptionEagerCommitDelay) as u64;

    let eager: bool = !eager_delay && mi_option_is_enabled(MiOptionEagerCommitDelay);
    let mut commit = eager || (required > 0);
    let mut is_zero = false;

    let mut commit_mask = MiCommitMask { mask: [0; 8] };
    let mut decommit_mask = MiCommitMask { mask: [0; 8] };
    mi_commit_mask_create_empty(&mut commit_mask);
    mi_commit_mask_create_empty(&mut decommit_mask);

    // Allocate the segment from the OS
    let segment = mi_segment_os_alloc(
        required,
        page_alignment,
        eager_delay,
        req_arena_id,
        &mut segment_slices,
        &mut pre_size,
        &mut info_slices,
        &mut commit_mask,
        &mut decommit_mask,
        &mut is_zero,
        &mut commit,
        tld,
        os_tld,
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
        (*segment).allow_decommit = mi_option_is_enabled(MiOption::MiOptionAllowDecommit)
            && !(*segment).mem_is_pinned
            && !(*segment).mem_is_large;
        if (*segment).allow_decommit {
            (*segment).decommit_expire = 0; // don't decommit just committed memory // _mi_clock_now() + mi_option_get(mi_option_decommit_delay);
            (*segment).decommit_mask = decommit_mask;
            debug_assert!(mi_commit_mask_all_set(
                &(*segment).commit_mask,
                &(*segment).decommit_mask
            ));
            // #if MI_DEBUG>2
            if cfg!(Debug) {
                let commit_needed =
                    _mi_divide_up(info_slices * MI_SEGMENT_SLICE_SIZE, MI_COMMIT_SIZE);
                let mut commit_needed_mask = MiCommitMask { mask: [0; 8] };
                mi_commit_mask_create(0, commit_needed, &mut commit_needed_mask);
                debug_assert!(mi_commit_mask_any_set(
                    &(*segment).decommit_mask,
                    &commit_needed_mask,
                ));
            }

            // #endif
        }
    }

    ptr::null_mut()
}

fn mi_segment_os_alloc(
    required: usize,
    page_alignment: usize,
    eager_delay: bool,
    req_arena_id: MiArenaIdT,
    psegment_slices: *mut usize,
    ppre_size: *mut usize,
    pinfo_slices: *mut usize,
    pcommit_mask: *mut MiCommitMask,
    pdecommit_mask: *mut MiCommitMask,
    is_zero: *mut bool,
    pcommit: *mut bool,
    tld: *mut MiSegmentsTLD,
    os_tld: *mut MiOsTLD,
) -> *mut MiSegment {
    // Allocate the segment from the OS
    let mut mem_large = !eager_delay && (MI_SECURE == 0); // only allow large OS pages once we are no longer lazy
    let mut is_pinned = false;
    let mut memid: usize = 0;
    let mut align_offset = 0;
    let mut alignment = MI_SEGMENT_ALIGN;

    if page_alignment > 0 {
        // debug_assert!(huge_page != NULL);
        debug_assert!(page_alignment >= MI_SEGMENT_ALIGN);
        alignment = page_alignment;
        let info_size = unsafe { (*pinfo_slices) * MI_SEGMENT_SLICE_SIZE };
        align_offset = _mi_align_up(info_size, MI_SEGMENT_ALIGN);
        let extra = align_offset - info_size;
        // recalculate due to potential guard pages
        unsafe {
            *psegment_slices =
                mi_segment_calculate_slices(required + extra, ppre_size, pinfo_slices);
        }
        //segment_size += _mi_align_up(align_offset - info_size, MI_SEGMENT_SLICE_SIZE);
        //segment_slices = segment_size / MI_SEGMENT_SLICE_SIZE;
    }

    let segment_size = unsafe { (*psegment_slices) * MI_SEGMENT_SLICE_SIZE };
    let mut segment: *mut MiSegment = ptr::null_mut();

    // get from cache?
    if page_alignment == 0 {
        segment = _mi_segment_cache_pop(
            segment_size,
            pcommit_mask,
            pdecommit_mask,
            &mut mem_large,
            &mut is_pinned,
            is_zero,
            req_arena_id,
            &mut memid,
            os_tld,
        )
        .cast();
    }

    // get from OS
    if segment.is_null() {
        segment = _mi_arena_alloc_aligned(
            segment_size,
            alignment,
            align_offset,
            pcommit,
            &mut mem_large,
            &mut is_pinned,
            is_zero,
            req_arena_id,
            &mut memid,
            os_tld,
        )
        .cast();
        if segment.is_null() {
            return ptr::null_mut(); // failed to allocate
        }
        unsafe {
            if *pcommit {
                mi_commit_mask_create_full(pcommit_mask);
            } else {
                mi_commit_mask_create_empty(pcommit_mask);
            }
        }
    }
    debug_assert!(!segment.is_null() && segment as usize % MI_SEGMENT_SIZE == 0);

    let commit_needed =
        unsafe { _mi_divide_up((*pinfo_slices) * MI_SEGMENT_SLICE_SIZE, MI_COMMIT_SIZE) };
    debug_assert!(commit_needed > 0);
    let mut commit_needed_mask = MiCommitMask { mask: [0; 8] };
    mi_commit_mask_create(0, commit_needed, &mut commit_needed_mask);
    if !mi_commit_mask_all_set(pcommit_mask, &commit_needed_mask) {
        // at least commit the info slices
        unsafe {
            debug_assert!(commit_needed * MI_COMMIT_SIZE >= (*pinfo_slices) * MI_SEGMENT_SLICE_SIZE)
        };
        let ok = _mi_os_commit(
            segment.cast(),
            commit_needed * MI_COMMIT_SIZE,
            is_zero, /*unsafe { (*tld).stats }*/
        );
        if !ok {
            return ptr::null_mut(); // failed to commit
        }
        mi_commit_mask_set(pcommit_mask, &commit_needed_mask);
    }
    //   mi_track_mem_undefined(segment,commit_needed);
    unsafe {
        (*segment).memid = memid;
        (*segment).mem_is_pinned = is_pinned;
        (*segment).mem_is_large = mem_large;
        (*segment).mem_is_committed = mi_commit_mask_is_full(pcommit_mask);
        (*segment).mem_alignment = alignment;
        (*segment).mem_align_offset = align_offset;
    }
    mi_segments_track_size(segment_size as i32, tld);
    _mi_segment_map_allocated_at(segment);
    return segment;
}

fn mi_segment_try_reclaim(
    heap: *mut MiHeap,
    needed_slices: usize,
    block_size: usize,
    reclaimed: *mut bool,
    tld: *mut MiSegmentsTLD,
) -> *mut MiSegment {
    //   unsafe { *reclaimed = false ;
    //   let segment;
    //   let mut max_tries = mi_option_get_clamp(MiOptionMaxSegmentReclaim, 8, 1024);     // limit the work to bound allocation times
    //   while ((max_tries-- > 0) && (!(segment = mi_abandoned_pop()).is_null())) {
    //     (*segment).abandoned_visits += 1;
    //     // todo: an arena exclusive heap will potentially visit many abandoned unsuitable segments
    //     // and push them into the visited list and use many tries. Perhaps we can skip non-suitable ones in a better way?
    //     let is_suitable = _mi_heap_memid_is_suitable(heap, (*segment).memid);
    //     let has_page = mi_segment_check_free(segment,needed_slices,block_size,tld); // try to free up pages (due to concurrent frees)
    //     if (*segment).used == 0 {
    //       // free the segment (by forced reclaim) to make it available to other threads.
    //       // note1: we prefer to free a segment as that might lead to reclaiming another
    //       // segment that is still partially used.
    //       // note2: we could in principle optimize this by skipping reclaim and directly
    //       // freeing but that would violate some invariants temporarily)
    //       mi_segment_reclaim(segment, heap, 0, ptr::null_mut(), tld);
    //     }
    //     else if (has_page && is_suitable) {
    //       // found a large enough free span, or a page of the right block_size with free space
    //       // we return the result of reclaim (which is usually `segment`) as it might free
    //       // the segment due to concurrent frees (in which case `NULL` is returned).
    //       return mi_segment_reclaim(segment, heap, block_size, reclaimed, tld);
    //     }
    //     else if (*segment).abandoned_visits > 3 && is_suitable {
    //       // always reclaim on 3rd visit to limit the abandoned queue length.
    //       mi_segment_reclaim(segment, heap, 0, NULL, tld);
    //     }
    //     else {
    //       // otherwise, push on the visited list so it gets not looked at too quickly again
    //     //   mi_segment_delayed_decommit(segment, true /* force? */, (*tld).stats); // forced decommit if needed as we may not visit soon again
    //     //   mi_abandoned_visited_push(segment);
    //     }
    //   }
    //   }
    ptr::null_mut()
}

/* -----------------------------------------------------------
   Reclaim or allocate
----------------------------------------------------------- */
fn mi_segment_reclaim_or_alloc(
    heap: *mut MiHeap,
    needed_slices: usize,
    block_size: usize,
    tld: *mut MiSegmentsTLD,
    os_tld: *mut MiOsTLD,
) -> *mut MiSegment {
    debug_assert!(block_size < MI_HUGE_BLOCK_SIZE);
    debug_assert!(block_size <= MI_LARGE_OBJ_SIZE_MAX);

    // 1. try to reclaim an abandoned segment
    let mut reclaimed = false;
    let segment = mi_segment_try_reclaim(heap, needed_slices, block_size, &mut reclaimed, tld);
    if reclaimed {
        // reclaimed the right page right into the heap
        debug_assert!(!segment.is_null());
        return ptr::null_mut(); // pretend out-of-memory as the page will be in the page queue of the heap with available blocks
    } else if !segment.is_null() {
        // reclaimed a segment with a large enough empty span in it
        return segment;
    }
    // 2. otherwise allocate a fresh segment
    mi_segment_alloc(
        0,
        0,
        unsafe { (*heap).arena_id },
        tld,
        os_tld,
        ptr::null_mut(),
    )
}

fn mi_segment_calculate_slices(
    required: usize,
    pre_size: *mut usize,
    info_slices: *mut usize,
) -> usize {
    0
}

type MiTaggedSegment = u64;

// // Pop from the abandoned list
// fn  mi_abandoned_pop() -> *mut MiSegment {
//     let segment;
//     // Check efficiently if it is empty (or if the visited list needs to be moved)
//     let ts = mi_atomic_load_relaxed(&abandoned);
//     segment = mi_tagged_segment_ptr(ts);
//     if mi_likely(segment == NULL) {
//       if mi_likely(!mi_abandoned_visited_revisit()) { // try to swap in the visited list on NULL
//         return NULL;
//       }
//     }

//     // Do a pop. We use a reader count to prevent
//     // a segment to be decommitted while a read is still pending,
//     // and a tagged pointer to prevent A-B-A link corruption.
//     // (this is called from `region.c:_mi_mem_free` for example)
//     mi_atomic_increment_relaxed(&abandoned_readers);  // ensure no segment gets decommitted
//     mi_tagged_segment_t next = 0;
//     ts = mi_atomic_load_acquire(&abandoned);
//     do {
//       segment = mi_tagged_segment_ptr(ts);
//       if (segment != NULL) {
//         mi_segment_t* anext = mi_atomic_load_ptr_relaxed(mi_segment_t, &segment->abandoned_next);
//         next = mi_tagged_segment(anext, ts); // note: reads the segment's `abandoned_next` field so should not be decommitted
//       }
//     } while (segment != NULL && !mi_atomic_cas_weak_acq_rel(&abandoned, &ts, next));
//     mi_atomic_decrement_relaxed(&abandoned_readers);  // release reader lock
//     if (segment != NULL) {
//       mi_atomic_store_ptr_release(mi_segment_t, &segment->abandoned_next, NULL);
//       mi_atomic_decrement_relaxed(&abandoned_count);
//     }
//     return segment;
//   }

// Reclaim an abandoned segment; returns NULL if the segment was freed
// set `right_page_reclaimed` to `true` if it reclaimed a page of the right `block_size` that was not full.
// static mi_segment_t* mi_segment_reclaim(mi_segment_t* segment, mi_heap_t* heap, size_t requested_block_size, bool* right_page_reclaimed, mi_segments_tld_t* tld) {
//     mi_assert_internal(mi_atomic_load_ptr_relaxed(mi_segment_t, &segment->abandoned_next) == NULL);
//     mi_assert_expensive(mi_segment_is_valid(segment, tld));
//     if (right_page_reclaimed != NULL) { *right_page_reclaimed = false; }

//     segment->thread_id = _mi_thread_id();
//     segment->abandoned_visits = 0;
//     mi_segments_track_size((long)mi_segment_size(segment), tld);
//     mi_assert_internal(segment->next == NULL);
//     _mi_stat_decrease(&tld->stats->segments_abandoned, 1);

//     // for all slices
//     const mi_slice_t* end;
//     mi_slice_t* slice = mi_slices_start_iterate(segment, &end);
//     while (slice < end) {
//       mi_assert_internal(slice->slice_count > 0);
//       mi_assert_internal(slice->slice_offset == 0);
//       if (mi_slice_is_used(slice)) {
//         // in use: reclaim the page in our heap
//         mi_page_t* page = mi_slice_to_page(slice);
//         mi_assert_internal(!page->is_reset);
//         mi_assert_internal(page->is_committed);
//         mi_assert_internal(mi_page_thread_free_flag(page)==MI_NEVER_DELAYED_FREE);
//         mi_assert_internal(mi_page_heap(page) == NULL);
//         mi_assert_internal(page->next == NULL && page->prev==NULL);
//         _mi_stat_decrease(&tld->stats->pages_abandoned, 1);
//         segment->abandoned--;
//         // set the heap again and allow delayed free again
//         mi_page_set_heap(page, heap);
//         _mi_page_use_delayed_free(page, MI_USE_DELAYED_FREE, true); // override never (after heap is set)
//         _mi_page_free_collect(page, false); // ensure used count is up to date
//         if (mi_page_all_free(page)) {
//           // if everything free by now, free the page
//           slice = mi_segment_page_clear(page, tld);   // set slice again due to coalesceing
//         }
//         else {
//           // otherwise reclaim it into the heap
//           _mi_page_reclaim(heap, page);
//           if (requested_block_size == page->xblock_size && mi_page_has_any_available(page)) {
//             if (right_page_reclaimed != NULL) { *right_page_reclaimed = true; }
//           }
//         }
//       }
//       else {
//         // the span is free, add it to our page queues
//         slice = mi_segment_span_free_coalesce(slice, tld); // set slice again due to coalesceing
//       }
//       mi_assert_internal(slice->slice_count>0 && slice->slice_offset==0);
//       slice = slice + slice->slice_count;
//     }

//     mi_assert(segment->abandoned == 0);
//     if (segment->used == 0) {  // due to page_clear
//       mi_assert_internal(right_page_reclaimed == NULL || !(*right_page_reclaimed));
//       mi_segment_free(segment, false, tld);
//       return NULL;
//     }
//     else {
//       return segment;
//     }
//   }

// -------------------------------------------------------------------
// commit mask
// -------------------------------------------------------------------

fn mi_commit_mask_all_set(commit: *const MiCommitMask, cm: *const MiCommitMask) -> bool {
    for i in 0..MI_COMMIT_MASK_FIELD_COUNT {
        unsafe {
            if ((*commit).mask[i] & (*cm).mask[i]) != (*cm).mask[i] {
                return false;
            }
        }
    }

    true
}

fn mi_commit_mask_any_set(commit: *const MiCommitMask, cm: *const MiCommitMask) -> bool {
    for i in 0..MI_COMMIT_MASK_FIELD_COUNT {
        unsafe {
            if ((*commit).mask[i] & (*cm).mask[i]) != 0 {
                return true;
            }
        }
    }

    false
}

fn mi_commit_mask_create_intersect(
    commit: *const MiCommitMask,
    cm: *const MiCommitMask,
    res: *mut MiCommitMask,
) {
    for i in 0..MI_COMMIT_MASK_FIELD_COUNT {
        unsafe {
            (*res).mask[i] = (*commit).mask[i] & (*cm).mask[i];
        }
    }
}

fn mi_commit_mask_clear(res: *mut MiCommitMask, cm: *const MiCommitMask) {
    for i in 0..MI_COMMIT_MASK_FIELD_COUNT {
        unsafe {
            (*res).mask[i] &= !((*cm).mask[i]);
        }
    }
}

fn mi_commit_mask_set(res: *mut MiCommitMask, cm: *const MiCommitMask) {
    for i in 0..MI_COMMIT_MASK_FIELD_COUNT {
        unsafe {
            (*res).mask[i] |= (*cm).mask[i];
        }
    }
}

fn mi_commit_mask_create(bitidx: usize, mut bitcount: usize, cm: *mut MiCommitMask) {
    debug_assert!(bitidx < MI_COMMIT_MASK_BITS);
    debug_assert!((bitidx + bitcount) <= MI_COMMIT_MASK_BITS);
    if bitcount == MI_COMMIT_MASK_BITS {
        debug_assert!(bitidx == 0);
        mi_commit_mask_create_full(cm);
    } else if bitcount == 0 {
        mi_commit_mask_create_empty(cm);
    } else {
        mi_commit_mask_create_empty(cm);
        let mut i = bitidx / MI_COMMIT_MASK_FIELD_BITS;
        let mut ofs = bitidx % MI_COMMIT_MASK_FIELD_BITS;
        while bitcount > 0 {
            debug_assert!(i < MI_COMMIT_MASK_FIELD_COUNT);
            let avail = MI_COMMIT_MASK_FIELD_BITS - ofs;
            let count = if bitcount > avail { avail } else { bitcount };
            let mask = if count >= MI_COMMIT_MASK_FIELD_BITS {
                !0
            } else {
                ((1 << count) - 1) << ofs
            };
            unsafe {
                (*cm).mask[i] = mask;
            }
            bitcount -= count;
            ofs = 0;
            i += 1;
        }
    }
}

fn _mi_commit_mask_committed_size(cm: *const MiCommitMask, total: usize) -> usize {
    debug_assert!((total % MI_COMMIT_MASK_BITS) == 0);
    let mut count = 0;
    for i in 0..MI_COMMIT_MASK_FIELD_COUNT {
        let mut mask = unsafe { (*cm).mask[i] };
        if !mask == 0 {
            count += MI_COMMIT_MASK_FIELD_BITS;
        } else {
            while mask != 0 {
                if (mask & 1) != 0 {
                    count += 1;
                }

                mask >>= 1;
            }

            // for (; mask != 0; mask >>= 1) {  // todo: use popcount
            //   if ((mask&1)!=0) count++;
            // }
        }
    }
    // we use total since for huge segments each commit bit may represent a larger size
    (total / MI_COMMIT_MASK_BITS) * count
}

fn _mi_commit_mask_next_run(cm: *const MiCommitMask, idx: *mut usize) -> usize {
    let mut i = unsafe { (*idx) / MI_COMMIT_MASK_FIELD_BITS };
    let mut ofs = unsafe { (*idx) % MI_COMMIT_MASK_FIELD_BITS };
    let mut mask = 0;
    // find first ones
    while i < MI_COMMIT_MASK_FIELD_COUNT {
        mask = unsafe { (*cm).mask[i] };
        mask >>= ofs;
        if mask != 0 {
            while (mask & 1) == 0 {
                mask >>= 1;
                ofs += 1;
            }
            break;
        }
        i += 1;
        ofs = 0;
    }
    if i >= MI_COMMIT_MASK_FIELD_COUNT {
        // not found
        unsafe { *idx = MI_COMMIT_MASK_BITS };
        0
    } else {
        // found, count ones
        let mut count = 0;
        unsafe {
            *idx = (i * MI_COMMIT_MASK_FIELD_BITS) + ofs;
        }

        loop {
            debug_assert!(ofs < MI_COMMIT_MASK_FIELD_BITS && (mask & 1) == 1);
            loop {
                count += 1;
                mask >>= 1;
                if (mask & 1) != 1 {
                    break;
                }
            }
            if ((unsafe { *idx } + count) % MI_COMMIT_MASK_FIELD_BITS) == 0 {
                i += 1;
                if i >= MI_COMMIT_MASK_FIELD_COUNT {
                    break;
                }
                mask = unsafe { (*cm).mask[i] };
                ofs = 0;
            }

            if (mask & 1) != 1 {
                break;
            }
        }

        debug_assert!(count > 0);
        count
    }
}

/* ----------------------------------------------------------------------------
Segment caches
We keep a small segment cache per thread to increase local
reuse and avoid setting/clearing guard pages in secure mode.
------------------------------------------------------------------------------- */

fn mi_segments_track_size(segment_size: i32, tld: *mut MiSegmentsTLD) {
    // if (segment_size>=0) _mi_stat_increase(&tld->stats->segments,1);
    //                 else _mi_stat_decrease(&tld->stats->segments,1);
    unsafe {
        if segment_size >= 0 {
            (*tld).count += 1
        } else {
            (*tld).count -= 1
        };
        if (*tld).count > (*tld).peak_count {
            (*tld).peak_count = (*tld).count;
        }
        (*tld).current_size += segment_size as u64;
        if (*tld).current_size > (*tld).peak_size {
            (*tld).peak_size = (*tld).current_size;
        }
    }
}

pub fn mi_segments_page_find_and_allocate(
    slice_count: usize,
    req_arena_id: MiArenaIdT,
    tld: MiSegmentsTLD,
) -> *mut MiPage {
    debug_assert!(slice_count * MI_SEGMENT_SLICE_SIZE <= MI_LARGE_OBJ_SIZE_MAX);
    // search from best fit up

    ptr::null_mut()
}
