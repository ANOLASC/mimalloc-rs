use crate::mimalloc_types::{MiSegmentKind, MI_HUGE_BLOCK_SIZE, MI_SEGMENT_MASK};
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
