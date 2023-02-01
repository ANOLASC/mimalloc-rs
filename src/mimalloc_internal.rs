use std::ffi::c_void;

use crate::mimalloc_types::{MiHeap, MiPage, MI_PADDING_SIZE, MI_PAGES_DIRECT, MI_SMALL_SIZE_MAX};

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
