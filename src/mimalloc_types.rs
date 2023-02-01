use std::{
    collections::LinkedList,
    ffi::c_void,
    ops::{Deref, DerefMut},
    ptr,
    sync::{atomic::AtomicPtr, Arc},
};

pub const MI_SMALL_WSIZE_MAX: usize = 128;
pub const MI_SMALL_SIZE_MAX: usize = MI_SMALL_WSIZE_MAX * std::mem::size_of::<c_void>();

// for now, only support 32 or 64 bits
#[cfg(target_pointer_width = "64")]
pub const MI_INTPTR_SHIFT: usize = 3;
#[cfg(target_pointer_width = "32")]
pub const MI_INTPTR_SHIFT: usize = 2;

pub const MI_INTPTR_SIZE: usize = 1 << MI_INTPTR_SHIFT;
pub const MI_INTPTR_BITS: usize = MI_INTPTR_SIZE * 8;

#[cfg(debug_assertions)]
pub const MI_PADDING: usize = 1;
#[cfg(not(debug_assertions))]
pub const MI_PADDING: usize = 0;

#[repr(C)]
struct MiPadding {
    canary: u32, // encoded block value to check validity of the padding (in case of overflow)
    delta: u32, // padding bytes before the block. (mi_usable_size(p) - delta == exact allocated bytes)
}

pub const MI_PADDING_SIZE: usize = std::mem::size_of::<MiPadding>();
pub const MI_PADDING_WSIZE: usize = (MI_PADDING_SIZE + MI_INTPTR_SIZE - 1) / MI_INTPTR_SIZE;
pub const MI_PAGES_DIRECT: usize = MI_SMALL_WSIZE_MAX + MI_PADDING_WSIZE + 1;
pub const MI_BIN_HUGE: usize = 73;
pub const MI_BIN_FULL: usize = MI_BIN_HUGE + 1;

#[repr(C)]
pub struct MiHeap {
    // pub tld: *mut mi_tld_t,
    pub pages_free_direct: [*mut MiPage; MI_PAGES_DIRECT], // optimize: array where every entry points a page with possibly free blocks in the corresponding queue for that size.
    pub pages: [MiPageQueue; MI_BIN_FULL + 1], // queue of pages for each size class (or "bin")
    // the same in-memory representation as raw pointer
    pub thread_delayed_free: AtomicPtr<MiBlock>,
    pub thread_id: usize, // thread this heap belongs to
    // mi_arena_id_t         arena_id;                            // arena id if the heap belongs to a specific arena (or 0)
    // uintptr_t             cookie;                              // random cookie to verify pointers (see `_mi_ptr_cookie`)
    // uintptr_t             keys[2];                             // two random keys used to encode the `thread_delayed_free` list
    // mi_random_ctx_t       random;                              // random number context used for secure allocation
    pub page_count: usize,       // total number of pages in the `pages` queues.
    pub page_retired_min: usize, // smallest retired index (retired pages are fully free, but still in the page queues)
    pub page_retired_max: usize, // largest retired index into the `pages` array.
    pub next: *mut MiHeap,       // list of heaps per thread
    pub no_reclaim: bool,        // `true` if this heap should not reclaim abandoned pages
}

impl MiHeap {
    pub fn new() -> Self {
        Self {
            pages_free_direct: [ptr::null_mut(); MI_PAGES_DIRECT],
            page_count: 0,
            page_retired_min: 0,
            page_retired_max: 0,
            no_reclaim: false,
            thread_id: 0,
            pages: [MiPageQueue {
                first: ptr::null_mut(),
                last: ptr::null_mut(),
                block_size: 0,
            }; MI_BIN_FULL + 1],
            next: ptr::null_mut(),
            thread_delayed_free: AtomicPtr::new(ptr::null_mut()),
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct MiPageQueue {
    pub first: *mut MiPage,
    pub last: *mut MiPage,
    pub block_size: usize,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union MiPageFlags {
    pub full_aligned: u8,
    pub x: PageFlag,
    _bindgen_union_align: u8,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct PageFlag {
    pub bitfield_1: BitfieldUnit<[u8; 1], u8>,
}

impl PageFlag {
    #[inline]
    pub fn in_full(&self) -> u8 {
        self.bitfield_1.get(0usize, 1u8) as u8
    }
    #[inline]
    pub fn set_in_full(&mut self, val: u8) {
        self.bitfield_1.set(0usize, 1u8, val as u64)
    }
    #[inline]
    pub fn has_aligned(&self) -> u8 {
        self.bitfield_1.get(1usize, 1u8) as u8
    }
    #[inline]
    pub fn set_has_aligned(&mut self, val: u8) {
        self.bitfield_1.set(1usize, 1u8, val as u64)
    }
    #[inline]
    pub fn new_bitfield_1(in_full: u8, has_aligned: u8) -> BitfieldUnit<[u8; 1usize], u8> {
        let mut bitfield_unit: BitfieldUnit<[u8; 1usize], u8> = Default::default();
        bitfield_unit.set(0usize, 1u8, in_full as u64);
        bitfield_unit.set(1usize, 1u8, has_aligned as u64);
        bitfield_unit
    }
}

pub type MiThreadFree = usize;
#[repr(C)]
pub struct MiPage {
    // "owned" by the segment
    pub slice_count: u32,  // slices in this page (0 if not a page)
    pub slice_offset: u32, // distance from the actual page data slice (0 if a page)
    pub bitfield_1: BitfieldUnit<[u8; 1], u8>,

    // layout like this to optimize access in `mi_malloc` and `mi_free`
    pub capacity: u16, // number of blocks committed, must be the first field, see `segment.c:page_clear`
    pub reserved: u16, // number of blocks reserved in memory
    pub flags: MiPageFlags, // `in_full` and `has_aligned` flags (8 bits)
    pub bitfield_2: BitfieldUnit<[u8; 1], u8>,

    pub free: LinkedList<MiBlock>, // list of available free blocks (`malloc` allocates from this list)
    pub used: u32, // number of blocks in use (including blocks in `local_free` and `thread_free`)
    pub xblock_size: u32, // size available in each block (always `>0`)
    pub local_free: *mut MiBlock, // list of deferred free blocks by this thread (migrates to `free`)

    // #ifdef MI_ENCODE_FREELIST
    // uintptr_t             keys[2];           // two random keys to encode the free lists (see `_mi_block_next`)
    // #endif
    pub xthread_free: AtomicPtr<MiThreadFree>, // list of deferred free blocks freed by other threads
    pub xheap: AtomicPtr<usize>,
    pub next: *mut MiPage, // next page owned by this thread with the same `block_size`
    pub prev: *mut MiPage, // previous page owned by this thread with the same `block_size`

                           // // 64-bit 9 words, 32-bit 12 words, (+2 for secure)
                           // #if MI_INTPTR_SIZE==8
                           // uintptr_t padding[1];
                           // #endif
}

impl MiPage {
    #[inline]
    // `true` if the page memory was reset
    pub fn is_reset(&self) -> u8 {
        self.bitfield_1.get(0usize, 1u8) as u8
    }
    #[inline]
    pub fn set_is_reset(&mut self, val: u8) {
        self.bitfield_1.set(0usize, 1u8, val as u64)
    }

    // `true` if the page virtual memory is committed
    #[inline]
    pub fn is_committed(&self) -> u8 {
        self.bitfield_1.get(0, 1) as u8
    }

    #[inline]
    pub fn set_is_committed(&mut self, val: u8) {
        self.bitfield_1.set(1, 1, val as u64);
    }

    // `true` if the page was zero initialized
    #[inline]
    pub fn is_zero_init(&self) -> u8 {
        self.bitfield_1.get(2usize, 1u8) as u8
    }
    #[inline]
    pub fn set_is_zero_init(&mut self, val: u8) {
        self.bitfield_1.set(2usize, 1u8, val as u64)
    }

    #[inline]
    pub fn new_bitfield_1(
        is_reset: u8,
        is_committed: u8,
        is_zero_init: u8,
    ) -> BitfieldUnit<[u8; 1usize], u8> {
        let mut bitfield_unitbitfield_unit: BitfieldUnit<[u8; 1usize], u8> = Default::default();
        bitfield_unitbitfield_unit.set(0usize, 1u8, is_reset as u64);
        bitfield_unitbitfield_unit.set(1usize, 1u8, is_committed as u64);
        bitfield_unitbitfield_unit.set(2usize, 1u8, is_zero_init as u64);
        bitfield_unitbitfield_unit
    }

    // `true` if the blocks in the free list are zero initialized
    #[inline]
    pub fn is_zero(&self) -> u8 {
        self.bitfield_2.get(0usize, 1u8) as u8
    }
    #[inline]
    pub fn set_is_zero(&mut self, val: u8) {
        self.bitfield_2.set(0usize, 1u8, val as u64)
    }

    // expiration count for retired blocks
    #[inline]
    pub fn retire_expire(&self) -> u8 {
        self.bitfield_2.get(1usize, 7u8) as u8
    }
    #[inline]
    pub fn set_retire_expire(&mut self, val: u8) {
        self.bitfield_2.set(1usize, 7u8, val as u64)
    }
    #[inline]
    pub fn new_bitfield_2(is_zero: u8, retire_expire: u8) -> BitfieldUnit<[u8; 1usize], u8> {
        let mut bitfield_unitbitfield_unit: BitfieldUnit<[u8; 1usize], u8> = Default::default();
        bitfield_unitbitfield_unit.set(0usize, 1u8, is_zero as u64);
        bitfield_unitbitfield_unit.set(1usize, 7u8, retire_expire as u64);
        bitfield_unitbitfield_unit
    }
}

type MiEncoded = usize;
// free lists contain blocks
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct MiBlock {
    next: MiEncoded,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BitfieldUnit<Storage, Align> {
    storage: Storage,
    align: [Align; 0],
}

impl<Storage, Align> BitfieldUnit<Storage, Align> {
    #[inline]
    pub fn new(storage: Storage) -> Self {
        Self { storage, align: [] }
    }
}

impl<Storage, Align> BitfieldUnit<Storage, Align>
where
    Storage: AsRef<[u8]> + AsMut<[u8]>,
{
    #[inline]
    pub fn get_bit(&self, index: usize) -> bool {
        debug_assert!(index / 8 < self.storage.as_ref().len());
        let byte_index = index / 8;
        let byte = self.storage.as_ref()[byte_index];
        let bit_index = if cfg!(target_endian = "big") {
            7 - (index % 8)
        } else {
            index % 8
        };
        let mask = 1 << bit_index;
        byte & mask == mask
    }

    #[inline]
    pub fn set_bit(&mut self, index: usize, val: bool) {
        debug_assert!(index / 8 < self.storage.as_ref().len());
        let byte_index = index / 8;
        let byte = &mut self.storage.as_mut()[byte_index];
        let bit_index = if cfg!(target_endian = "big") {
            7 - (index % 8)
        } else {
            index % 8
        };
        let mask = 1 << bit_index;
        if val {
            *byte |= mask;
        } else {
            *byte &= !mask;
        }
    }

    #[inline]
    pub fn get(&self, bit_offset: usize, bit_width: u8) -> u64 {
        debug_assert!(bit_width <= 64);
        debug_assert!(bit_offset / 8 <= self.storage.as_ref().len());
        debug_assert!((bit_offset + (bit_width as usize)) / 8 <= self.storage.as_ref().len());

        // TODO which one is faster?
        // (0..bit_width as usize).fold(0, |mut acc, x| {
        //     if self.get_bit(x + bit_offset) {
        //         let index = if cfg!(target_endian = "big") {
        //             bit_width as usize - 1 - x
        //         } else {
        //             x
        //         };
        //         acc |= 1 << index;
        //     }
        //     acc
        // });

        let mut val = 0;
        for i in 0..(bit_width as usize) {
            if self.get_bit(i + bit_offset) {
                let index = if cfg!(target_endian = "big") {
                    bit_width as usize - 1 - i
                } else {
                    i
                };
                val |= 1 << index;
            }
        }
        val
    }

    #[inline]
    pub fn set(&mut self, bit_offset: usize, bit_width: u8, val: u64) {
        debug_assert!(bit_width <= 64);
        debug_assert!(bit_offset / 8 <= self.storage.as_ref().len());
        debug_assert!((bit_offset + (bit_width as usize)) / 8 <= self.storage.as_ref().len());

        // TODO which one is faster?
        // (0..bit_width as usize).for_each(|x| {
        //     let mask = 1 << x;
        //     let val_bit_is_set = val & mask == mask;
        //     let index = if cfg!(target_endian = "big") {
        //         bit_width as usize - 1 - x
        //     } else {
        //         x
        //     };
        //     self.set_bit(index + bit_offset, val_bit_is_set);
        // });

        for i in 0..(bit_width as usize) {
            let mask = 1 << i;
            let val_bit_is_set = val & mask == mask;
            let index = if cfg!(target_endian = "big") {
                bit_width as usize - 1 - i
            } else {
                i
            };
            self.set_bit(index + bit_offset, val_bit_is_set);
        }
    }
}

pub struct MiSegment {
    // size_t            memid;              // memory id for arena allocation
    // bool              mem_is_pinned;      // `true` if we cannot decommit/reset/protect in this memory (i.e. when allocated using large OS pages)
    // bool              mem_is_large;       // in large/huge os pages?
    // bool              mem_is_committed;   // `true` if the whole segment is eagerly committed
    // size_t            mem_alignment;      // page alignment for huge pages (only used for alignment > MI_ALIGNMENT_MAX)
    // size_t            mem_align_offset;   // offset for huge page alignment (only used for alignment > MI_ALIGNMENT_MAX)

    // bool              allow_decommit;
    // mi_msecs_t        decommit_expire;
    // mi_commit_mask_t  decommit_mask;
    // mi_commit_mask_t  commit_mask;

    // _Atomic(struct mi_segment_s*) abandoned_next;

    // // from here is zero initialized
    // struct mi_segment_s* next;            // the list of freed segments in the cache (must be first field, see `segment.c:mi_segment_init`)

    // size_t            abandoned;          // abandoned pages (i.e. the original owning thread stopped) (`abandoned <= used`)
    // size_t            abandoned_visits;   // count how often this segment is visited in the abandoned list (to force reclaim it it is too long)
    // size_t            used;               // count of pages in use
    // uintptr_t         cookie;             // verify addresses in debug mode: `mi_ptr_cookie(segment) == segment->cookie`

    // size_t            segment_slices;      // for huge segments this may be different from `MI_SLICES_PER_SEGMENT`
    // size_t            segment_info_slices; // initial slices we are using segment info and possible guard pages.

    // // layout like this to optimize access in `mi_free`
    // mi_segment_kind_t kind;
    // size_t            slice_entries;       // entries in the `slices` array, at most `MI_SLICES_PER_SEGMENT`
    // _Atomic(mi_threadid_t) thread_id;      // unique id of the thread owning this segment

    // mi_slice_t        slices[MI_SLICES_PER_SEGMENT+1];  // one more for huge blocks with large alignment
}
