use libc::uintptr_t;

use crate::mimalloc_types::{
    MiCommitMask, MiSegmentKind, MI_HUGE_BLOCK_SIZE, MI_INTPTR_BITS, MI_SEGMENT_MASK,
    MI_SEGMENT_SIZE, MI_SEGMENT_SLICE_SIZE, MI_SIZE_BITS,
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
pub fn _mi_ptr_cookie(p: *const c_void) -> usize {
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
pub fn _mi_ptr_segment(p: *const c_void) -> *mut MiSegment {
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
    if (bsize as usize) < MI_HUGE_BLOCK_SIZE {
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

// "bit scan reverse": Return index of the highest bit (or MI_INTPTR_BITS if `x` is zero)
pub fn mi_bsr(x: uintptr_t) -> usize {
    return if x == 0 {
        MI_INTPTR_BITS
    } else {
        MI_INTPTR_BITS - 1 - mi_clz(x)
    };
}

// #include <limits.h>       // LONG_MAX
// #define MI_HAVE_FAST_BITSCAN
fn mi_clz(x: uintptr_t) -> usize {
    //   if x==0 {return MI_INTPTR_BITS;}
    //   let idx;
    // #[cfg(target_pointer_width = "64")]
    // _BitScanReverse64(&idx, x);
    // #[cfg(target_pointer_width = "32")]
    // _BitScanReverse(&idx, x);
    //   return ((MI_INTPTR_BITS - 1) - idx);

    // TODO: it need compiler intrinsics support, cuz https://doc.rust-lang.org/std/intrinsics/fn.ctlz.html is nightly only api

    0
}
fn mi_ctz(x: uintptr_t) -> usize {
    //   if (x==0) return MI_INTPTR_BITS;
    //   unsigned long idx;
    // #if (INTPTR_MAX == LONG_MAX)
    //   _BitScanForward(&idx, x);
    // #else
    //   _BitScanForward64(&idx, x);
    // #endif
    //   return idx;

    0
}

// size of a segment
pub fn mi_segment_size(segment: *const MiSegment) -> usize {
    unsafe { (*segment).segment_slices as usize * MI_SEGMENT_SLICE_SIZE }

    // return *(unsafe { *segment }).segment_slices * MI_SEGMENT_SLICE_SIZE;
}

//   static inline uint8_t* mi_segment_end(mi_segment_t* segment) {
//     return (uint8_t*)segment + mi_segment_size(segment);
//   }

//   // Thread free access
//   static inline mi_block_t* mi_page_thread_free(const mi_page_t* page) {
//     return (mi_block_t*)(mi_atomic_load_relaxed(&((mi_page_t*)page)->xthread_free) & ~3);
//   }

//   static inline mi_delayed_t mi_page_thread_free_flag(const mi_page_t* page) {
//     return (mi_delayed_t)(mi_atomic_load_relaxed(&((mi_page_t*)page)->xthread_free) & 3);
//   }

//   // Heap access
//   static inline mi_heap_t* mi_page_heap(const mi_page_t* page) {
//     return (mi_heap_t*)(mi_atomic_load_relaxed(&((mi_page_t*)page)->xheap));
//   }

//   static inline void mi_page_set_heap(mi_page_t* page, mi_heap_t* heap) {
//     mi_assert_internal(mi_page_thread_free_flag(page) != MI_DELAYED_FREEING);
//     mi_atomic_store_release(&page->xheap,(uintptr_t)heap);
//   }

//   // Thread free flag helpers
//   static inline mi_block_t* mi_tf_block(mi_thread_free_t tf) {
//     return (mi_block_t*)(tf & ~0x03);
//   }
//   static inline mi_delayed_t mi_tf_delayed(mi_thread_free_t tf) {
//     return (mi_delayed_t)(tf & 0x03);
//   }
//   static inline mi_thread_free_t mi_tf_make(mi_block_t* block, mi_delayed_t delayed) {
//     return (mi_thread_free_t)((uintptr_t)block | (uintptr_t)delayed);
//   }
//   static inline mi_thread_free_t mi_tf_set_delayed(mi_thread_free_t tf, mi_delayed_t delayed) {
//     return mi_tf_make(mi_tf_block(tf),delayed);
//   }
//   static inline mi_thread_free_t mi_tf_set_block(mi_thread_free_t tf, mi_block_t* block) {
//     return mi_tf_make(block, mi_tf_delayed(tf));
//   }

//   // are all blocks in a page freed?
//   // note: needs up-to-date used count, (as the `xthread_free` list may not be empty). see `_mi_page_collect_free`.
//   static inline bool mi_page_all_free(const mi_page_t* page) {
//     mi_assert_internal(page != NULL);
//     return (page->used == 0);
//   }

//   // are there any available blocks?
//   static inline bool mi_page_has_any_available(const mi_page_t* page) {
//     mi_assert_internal(page != NULL && page->reserved > 0);
//     return (page->used < page->reserved || (mi_page_thread_free(page) != NULL));
//   }

//   // are there immediately available blocks, i.e. blocks available on the free list.
//   static inline bool mi_page_immediate_available(const mi_page_t* page) {
//     mi_assert_internal(page != NULL);
//     return (page->free != NULL);
//   }

//   // is more than 7/8th of a page in use?
//   static inline bool mi_page_mostly_used(const mi_page_t* page) {
//     if (page==NULL) return true;
//     uint16_t frac = page->reserved / 8U;
//     return (page->reserved - page->used <= frac);
//   }

//   static inline mi_page_queue_t* mi_page_queue(const mi_heap_t* heap, size_t size) {
//     return &((mi_heap_t*)heap)->pages[_mi_bin(size)];
//   }

//   //-----------------------------------------------------------
//   // Page flags
//   //-----------------------------------------------------------
//   static inline bool mi_page_is_in_full(const mi_page_t* page) {
//     return page->flags.x.in_full;
//   }

//   static inline void mi_page_set_in_full(mi_page_t* page, bool in_full) {
//     page->flags.x.in_full = in_full;
//   }

//   static inline bool mi_page_has_aligned(const mi_page_t* page) {
//     return page->flags.x.has_aligned;
//   }

//   static inline void mi_page_set_has_aligned(mi_page_t* page, bool has_aligned) {
//     page->flags.x.has_aligned = has_aligned;
