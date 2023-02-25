// api

use crate::{mimalloc_internal::mi_page_usable_block_size, mimalloc_types::MI_DEBUG_UNINIT};
use std::{ffi::c_void, ptr, sync::Arc};

use crate::{
    mimalloc_internal::{
        _mi_heap_get_free_small_page, _mi_thread_id, get_default_heap, mi_page_is_huge,
    },
    mimalloc_types::{MiBlock, MiHeap, MiPage, MI_PADDING, MI_PADDING_SIZE, MI_SMALL_SIZE_MAX},
};

#[no_mangle]
pub extern "C" fn mi_malloc(size: usize) -> *mut c_void {
    mi_heap_malloc(get_default_heap(), size)
}

#[no_mangle]
pub extern "C" fn mi_calloc(count: usize, size: usize) -> *mut c_void {
    ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn mi_free(p: *mut c_void) {}

#[inline]
fn mi_heap_malloc(heap: Box<MiHeap>, size: usize) -> *mut c_void {
    _mi_heap_malloc_zero(heap, size, false)
}

#[inline]
fn _mi_heap_malloc_zero(heap: Box<MiHeap>, size: usize, zero: bool) -> *mut c_void {
    _mi_heap_malloc_zero_ex(heap, size, zero, 0)
}

#[inline]
fn _mi_heap_malloc_zero_ex(
    heap: Box<MiHeap>,
    size: usize,
    zero: bool,
    huge_alignment: usize,
) -> *mut c_void {
    if size <= MI_SMALL_SIZE_MAX {
        // fast path
        debug_assert!(huge_alignment == 0);
        mi_heap_malloc_small_zero(heap, size, zero)
    } else {
        // slow path
        // TODO To be implemented
        todo!("slow path does not implement now");
    }
}

#[inline]
fn mi_heap_malloc_small_zero(mut heap: Box<MiHeap>, size: usize, zero: bool) -> *mut c_void {
    if cfg!(debug_assertions) {
        let tid = _mi_thread_id();
        debug_assert!(heap.thread_id == 0 || heap.thread_id == tid);
    }

    let size = if MI_PADDING == 1 && size == 0 {
        std::mem::size_of::<c_void>()
    } else {
        size
    };

    let page = _mi_heap_get_free_small_page(&mut heap, size + MI_PADDING_SIZE);
    _mi_page_malloc(&heap, page, size, zero)
}

// ------------------------------------------------------
// Allocation
// ------------------------------------------------------

// Fast allocation in a page: just pop from the free list.
// Fall back to generic allocation only if the list is empty.
fn _mi_page_malloc<'a>(
    heap: &Box<MiHeap>,
    mut page: Box<MiPage>,
    size: usize,
    zero: bool,
) -> *mut c_void {
    // TODO check size for huge page
    debug_assert!(page.xblock_size == 0);

    let block = page.free.pop_front();

    if block.is_none() && page.free.is_empty() {
        // slow path
        todo!("malloc generic, to be implemented");
    }

    let mut block = block.unwrap();

    // TODO check is in that block located in page
    debug_assert!(!page.free.is_empty());

    page.used += 1;

    if zero {
        // TODO zero the block
        todo!("zero the block, to be implemented");
    }

    if page.is_zero() != 0 && !zero && !mi_page_is_huge(page.as_ref()) {
        unsafe {
            ptr::write_bytes(
                &mut block,
                MI_DEBUG_UNINIT,
                mi_page_usable_block_size(page.as_ref()),
            );
        }
    }

    let ptr: *mut MiBlock = &mut block;
    ptr as *mut c_void
}
