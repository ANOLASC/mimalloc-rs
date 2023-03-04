use crate::mimalloc_types::{
    MiCommitMask, MiSegmentKind, MI_HUGE_BLOCK_SIZE, MI_SEGMENT_MASK, MI_SEGMENT_SIZE,
    MI_SEGMENT_SLICE_SIZE, MI_SIZE_BITS,
};
use std::ffi::c_void;

use crate::{
    init::get_mi_heap_main,
    mimalloc_types::{
        MiHeap, MiPage, MiSegment, MI_PADDING_SIZE, MI_PAGES_DIRECT, MI_SMALL_SIZE_MAX,
    },
};

#[inline]
pub fn get_default_heap() -> Box<MiHeap> {
    // TODO
    Box::new(MiHeap::new())
}

type MiThreadid = usize;

#[inline]
pub fn _mi_thread_id() -> MiThreadid {
    0
}

#[inline]
pub fn _mi_heap_get_free_small_page<'a>(heap: &mut Box<MiHeap>, size: usize) -> Box<MiPage> {
    debug_assert!(size <= (MI_SMALL_SIZE_MAX + MI_PADDING_SIZE));

    let idx = _mi_wsize_from_size(size);

    debug_assert!(idx <= MI_PAGES_DIRECT);

    let ptr: *mut MiPage = heap.pages_free_direct[idx];

    unsafe { Box::from_raw(ptr) }
}

// Align a byte size to a size in _machine words_,
// i.e. byte size == `wsize*sizeof(void*)`.
#[inline]
fn _mi_wsize_from_size(size: usize) -> usize {
    debug_assert!(size <= usize::MAX - std::mem::size_of::<c_void>());
    (size + std::mem::size_of::<c_void>() - 1) / std::mem::size_of::<c_void>()
}

#[inline]
fn mi_heap_is_default(heap: *const MiHeap) -> bool {
    return heap == get_default_heap().as_ref();
}

#[inline]
fn mi_heap_is_backing(heap: *mut MiHeap) -> bool {
    unsafe { (*(*heap).tld).heap_backing == heap }
}

#[inline]
pub fn mi_heap_is_initialized(heap: *const MiHeap) -> bool {
    // debug_assert!(heap.is_null());
    // Currently, Heap are initialized by default
    false
}

#[inline]
fn _mi_ptr_cookie(p: *const c_void) -> usize {
    // extern MiHeap _mi_heap_main;
    debug_assert!(get_mi_heap_main().cookie != 0);
    (p as usize) ^ get_mi_heap_main().cookie
}

pub fn mi_page_is_huge(page: *const MiPage) -> bool {
    unsafe { matches!((*_mi_page_segment(page)).kind, MiSegmentKind::MiSegmentHuge) }
}

// Segment belonging to a page
fn _mi_page_segment(page: *const MiPage) -> *mut MiSegment {
    let segment = _mi_ptr_segment(page.cast());
    // TODO check segment is in range
    debug_assert!(segment.is_null());
    segment
}

// Segment that contains the pointer
// Large aligned blocks may be aligned at N*MI_SEGMENT_SIZE (inside a huge segment > MI_SEGMENT_SIZE),
// and we need align "down" to the segment info which is `MI_SEGMENT_SIZE` bytes before it;
// therefore we align one byte before `p`.
fn _mi_ptr_segment(p: *const c_void) -> *mut MiSegment {
    debug_assert!(p.is_null());
    ((p as usize - 1) & MI_SEGMENT_MASK.reverse_bits()) as *mut MiSegment
}

// Get the usable block size of a page without fixed padding.
// This may still include internal padding due to alignment and rounding up size classes.
pub fn mi_page_usable_block_size(page: *const MiPage) -> usize {
    mi_page_block_size(page) - MI_PADDING_SIZE
}

// Get the block size of a page (special case for huge objects)
fn mi_page_block_size(page: *const MiPage) -> usize {
    let bsize = unsafe { (*page).xblock_size };
    debug_assert!(bsize > 0);
    if bsize < MI_HUGE_BLOCK_SIZE {
        bsize as usize
    } else {
        // TODO
        todo!("unimplement for huge page");
        // let psize;
        // _mi_segment_page_start(_mi_page_segment(page), page, &psize);
        // return psize;
    }
}

// ------------------------------------------------------
// A segment holds a commit mask where a bit is set if
// the corresponding MI_COMMIT_SIZE area is committed.
// The MI_COMMIT_SIZE must be a multiple of the slice
// size. If it is equal we have the most fine grained
// decommit (but setting it higher can be more efficient).
// The MI_MINIMAL_COMMIT_SIZE is the minimal amount that will
// be committed in one go which can be set higher than
// MI_COMMIT_SIZE for efficiency (while the decommit mask
// is still tracked in fine-grained MI_COMMIT_SIZE chunks)
// ------------------------------------------------------

const MI_MINIMAL_COMMIT_SIZE: usize = 16 * MI_SEGMENT_SLICE_SIZE; // 1MiB
const MI_COMMIT_SIZE: usize = MI_SEGMENT_SLICE_SIZE; // 64KiB
const MI_COMMIT_MASK_BITS: usize = MI_SEGMENT_SIZE / MI_COMMIT_SIZE;
const MI_COMMIT_MASK_FIELD_BITS: usize = MI_SIZE_BITS;
const MI_COMMIT_MASK_FIELD_COUNT: usize = MI_COMMIT_MASK_BITS / MI_COMMIT_MASK_FIELD_BITS;

// -------------------------------------------------------------------
// commit mask
// -------------------------------------------------------------------

pub fn mi_commit_mask_create_empty(cm: *mut MiCommitMask) {
    for i in 0..MI_COMMIT_MASK_FIELD_COUNT {
        unsafe {
            (*cm).mask[i] = 0;
        }
    }
}

pub fn mi_commit_mask_create_full(cm: *mut MiCommitMask) {
    for i in 0..MI_COMMIT_MASK_FIELD_COUNT {
        unsafe {
            (*cm).mask[i] = !0;
        }
    }
}

pub fn mi_commit_mask_is_empty(cm: *const MiCommitMask) -> bool {
    for i in 0..MI_COMMIT_MASK_FIELD_COUNT {
        unsafe {
            if (*cm).mask[i] != 0 {
                return false;
            }
        }
    }
    return true;
}

pub fn mi_commit_mask_is_full(cm: *const MiCommitMask) -> bool {
    for i in 0..MI_COMMIT_MASK_FIELD_COUNT {
        unsafe {
            if (*cm).mask[i] != !0 {
                return false;
            }
        }
    }
    return true;
}

// Align downwards
pub fn _mi_align_down(sz: usize, alignment: usize) -> usize {
    debug_assert!(alignment != 0);
    let mask = alignment - 1;
    if (alignment & mask) == 0 {
        // power of two?
        return sz & !mask;
    } else {
        return (sz / alignment) * alignment;
    }
}

// Divide upwards: `s <= _mi_divide_up(s,d)*d < s+d`.
pub fn _mi_divide_up(size: usize, divider: usize) -> usize {
    debug_assert!(divider != 0);
    if divider == 0 {
        size
    } else {
        (size + divider - 1) / divider
    }
}

// Is memory zero initialized?
//   pub fn mi_mem_is_zero( p: *const c_void,  size: usize) -> bool{
//     for i in 0..size {
//         if (((uint8_t*) p )[i] != 0) {return false;}
//     }
//     return true;
//   }
