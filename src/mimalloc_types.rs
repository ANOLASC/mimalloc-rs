use std::{collections::LinkedList, ffi::c_void, ptr, sync::atomic::AtomicPtr};

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

pub const MI_PADDING_SIZE: usize = std::mem::size_of::<MiPadding>();
pub const MI_PADDING_WSIZE: usize = (MI_PADDING_SIZE + MI_INTPTR_SIZE - 1) / MI_INTPTR_SIZE;
pub const MI_PAGES_DIRECT: usize = MI_SMALL_WSIZE_MAX + MI_PADDING_WSIZE + 1;
pub const MI_BIN_HUGE: usize = 73;
pub const MI_BIN_FULL: usize = MI_BIN_HUGE + 1;

type MiMsecs = i64;
pub type SizeT = ::std::os::raw::c_ulonglong;
pub type MiThreadId = SizeT;
pub type MiSlice = MiPage;

const MI_SEGMENT_SLICE_SHIFT: usize = 13 + MI_INTPTR_SHIFT; // 64KiB  (32KiB on 32-bit)
const MI_SEGMENT_SLICE_SIZE: usize = 1 << MI_SEGMENT_SLICE_SHIFT;

#[cfg(target_pointer_width = "32")]
const MI_SEGMENT_SHIFT: usize = 7 + MI_SEGMENT_SLICE_SHIFT;
#[cfg(not(target_pointer_width = "32"))]
const MI_SEGMENT_SHIFT: usize = 9 + MI_SEGMENT_SLICE_SHIFT;

const MI_SEGMENT_SIZE: usize = 1 << MI_SEGMENT_SHIFT;
const MI_SLICES_PER_SEGMENT: usize = MI_SEGMENT_SIZE / MI_SEGMENT_SLICE_SIZE; // 1024

pub type MiArenaIdT = ::std::os::raw::c_int;

pub const MI_SEGMENT_ALIGN: usize = MI_SEGMENT_SIZE;
pub const MI_SEGMENT_MASK: usize = MI_SEGMENT_ALIGN - 1;
// may change in other debug mode
pub const MI_DEBUG_UNINIT: u8 = 0xD0;

pub const MI_KiB: u32 = 1024;
pub const MI_MiB: u32 = MI_KiB * MI_KiB;
pub const MI_GiB: u32 = MI_MiB * MI_KiB;

// Used as a special value to encode block sizes in 32 bits.
pub const MI_HUGE_BLOCK_SIZE: u32 = 2 * MI_GiB;

#[repr(C)]
struct MiPadding {
    canary: u32, // encoded block value to check validity of the padding (in case of overflow)
    delta: u32, // padding bytes before the block. (mi_usable_size(p) - delta == exact allocated bytes)
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct MiRandomCtx {
    pub input: [u32; 16usize],
    pub output: [u32; 16usize],
    pub output_available: ::std::os::raw::c_int,
    pub weak: bool,
}

#[repr(C)]
pub struct MiHeap {
    pub tld: *mut MiTLD,
    pub pages_free_direct: [*mut MiPage; MI_PAGES_DIRECT], // optimize: array where every entry points a page with possibly free blocks in the corresponding queue for that size.
    pub pages: [MiPageQueue; MI_BIN_FULL + 1], // queue of pages for each size class (or "bin")
    // the same in-memory representation as raw pointer
    pub thread_delayed_free: AtomicPtr<MiBlock>,
    pub thread_id: usize,        // thread this heap belongs to
    pub arena_id: MiArenaIdT,    // arena id if the heap belongs to a specific arena (or 0)
    pub cookie: usize,           // random cookie to verify pointers (see `_mi_ptr_cookie`)
    pub keys: [usize; 2],        // two random keys used to encode the `thread_delayed_free` list
    pub random: MiRandomCtx,     // random number context used for secure allocation
    pub page_count: usize,       // total number of pages in the `pages` queues.
    pub page_retired_min: usize, // smallest retired index (retired pages are fully free, but still in the page queues)
    pub page_retired_max: usize, // largest retired index into the `pages` array.
    pub next: *mut MiHeap,       // list of heaps per thread
    pub no_reclaim: bool,        // `true` if this heap should not reclaim abandoned pages
}

impl Default for MiHeap {
    fn default() -> Self {
        Self {
            tld: ptr::null_mut(),
            pages_free_direct: [ptr::null_mut(); MI_PAGES_DIRECT],
            pages: [Default::default(); MI_BIN_FULL + 1],
            thread_delayed_free: Default::default(),
            thread_id: Default::default(),
            arena_id: Default::default(),
            cookie: Default::default(),
            keys: Default::default(),
            random: Default::default(),
            page_count: Default::default(),
            page_retired_min: Default::default(),
            page_retired_max: Default::default(),
            next: ptr::null_mut(),
            no_reclaim: Default::default(),
        }
    }
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
            arena_id: 0,
            cookie: 0,
            keys: [0, 0],
            random: MiRandomCtx {
                input: [0; 16],
                output: [0; 16],
                output_available: 0,
                weak: false,
            },
            tld: ptr::null_mut(),
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

impl Default for MiPageQueue {
    fn default() -> Self {
        Self {
            first: ptr::null_mut(),
            last: ptr::null_mut(),
            block_size: Default::default(),
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union MiPageFlags {
    pub full_aligned: u8,
    pub x: PageFlag,
    _bindgen_union_align: u8,
}

impl Default for MiPageFlags {
    fn default() -> Self {
        Self { full_aligned: 0 }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
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

impl Default for MiPage {
    fn default() -> Self {
        Self {
            slice_count: Default::default(),
            slice_offset: Default::default(),
            bitfield_1: Default::default(),
            capacity: Default::default(),
            reserved: Default::default(),
            flags: Default::default(),
            bitfield_2: Default::default(),
            free: Default::default(),
            used: Default::default(),
            xblock_size: Default::default(),
            local_free: ptr::null_mut(),
            xthread_free: Default::default(),
            xheap: Default::default(),
            next: ptr::null_mut(),
            prev: ptr::null_mut(),
        }
    }
}

impl MiPage {
    pub fn new() -> Self {
        Self {
            slice_count: 0,
            slice_offset: 0,
            bitfield_1: BitfieldUnit::new([0]),
            capacity: 0,
            reserved: 0,
            flags: MiPageFlags { full_aligned: 0 },
            bitfield_2: BitfieldUnit::new([0]),
            free: LinkedList::new(),
            used: 0,
            xblock_size: 0,
            local_free: ptr::null_mut(),
            xthread_free: AtomicPtr::new(ptr::null_mut()),
            xheap: AtomicPtr::new(ptr::null_mut()),
            next: ptr::null_mut(),
            prev: ptr::null_mut(),
        }
    }

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

impl Default for MiBlock {
    fn default() -> Self {
        Self {
            next: Default::default(),
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BitfieldUnit<Storage, Align> {
    storage: Storage,
    align: [Align; 0],
}

impl<Storage, Align> BitfieldUnit<Storage, Align> {
    #[inline]
    pub const fn new(storage: Storage) -> Self {
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

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct MiCommitMask {
    pub mask: [SizeT; 8usize],
}

#[repr(C)]
enum MiPageKind {
    MiPageSmall,  // small blocks go into 64KiB pages inside a segment
    MiPageMedium, // medium blocks go into medium pages inside a segment
    MiPageLarge,  // larger blocks go into a page of just one block
    MiPageHuge,   // huge blocks (> 16 MiB) are put into a single page in a single segment.
}

#[repr(C)]
pub enum MiSegmentKind {
    MiSegmentNormal, // MI_SEGMENT_SIZE size with pages inside.
    MiSegmentHuge,   // > MI_LARGE_SIZE_MAX segment with just one huge page inside.
}

#[repr(C)]
pub struct MiSegment {
    pub memid: usize,            // memory id for arena allocation
    pub mem_is_pinned: bool, // `true` if we cannot decommit/reset/protect in this memory (i.e. when allocated using large OS pages)
    pub mem_is_large: bool,  // in large/huge os pages?
    pub mem_is_committed: bool, // `true` if the whole segment is eagerly committed
    pub mem_alignment: usize, // page alignment for huge pages (only used for alignment > MI_ALIGNMENT_MAX)
    pub mem_align_offset: usize, // offset for huge page alignment (only used for alignment > MI_ALIGNMENT_MAX)

    pub allow_decommit: bool,
    pub decommit_expire: MiMsecs,
    pub decommit_mask: MiCommitMask,
    pub commit_mask: MiCommitMask,
    pub abandoned_next: AtomicPtr<MiSegment>,
    // // from here is zero initialized
    pub next: *mut MiSegment, // the list of freed segments in the cache (must be first field, see `segment.c:mi_segment_init`)
    pub abandoned: SizeT, // abandoned pages (i.e. the original owning thread stopped) (`abandoned <= used`)
    pub abandoned_visits: SizeT, // count how often this segment is visited in the abandoned list (to force reclaim it it is too long)
    pub used: SizeT,             // count of pages in use
    pub cookie: usize, // verify addresses in debug mode: `mi_ptr_cookie(segment) == segment->cookie`

    pub segment_slices: SizeT, // for huge segments this may be different from `MI_SLICES_PER_SEGMENT`
    pub segment_info_slices: SizeT, // initial slices we are using segment info and possible guard pages.

    // // layout like this to optimize access in `mi_free`
    pub kind: MiSegmentKind,
    pub slice_entries: SizeT, // entries in the `slices` array, at most `MI_SLICES_PER_SEGMENT`
    pub thread_id: AtomicPtr<MiThreadId>, // unique id of the thread owning this segment

    pub slices: [MiSlice; MI_SLICES_PER_SEGMENT + 1], // one more for huge blocks with large alignment
}

// ------------------------------------------------------
// Thread Local data
// ------------------------------------------------------

// A "span" is is an available range of slices. The span queues keep
// track of slice spans of at most the given `slice_count` (but more than the previous size class).
#[repr(C)]
#[derive(Clone, Copy)] // May happen deep copy here, if use clone in MiSpanQueue
pub struct MiSpanQueue {
    pub first: *mut MiSlice,
    pub last: *mut MiSlice,
    pub slice_count: SizeT,
}

impl Default for MiSpanQueue {
    fn default() -> Self {
        Self {
            first: ptr::null_mut(),
            last: ptr::null_mut(),
            slice_count: Default::default(),
        }
    }
}

const MI_SEGMENT_BIN_MAX: usize = 35; // 35 == mi_segment_bin(MI_SLICES_PER_SEGMENT)

// OS thread local data
#[repr(C)]
#[derive(Clone)]
pub struct MiOsTLD {
    pub region_idx: SizeT, // start point for next allocation
                           //mi_stats_t*           stats,       // points to tld stats
}

impl Default for MiOsTLD {
    fn default() -> Self {
        Self {
            region_idx: Default::default(),
        }
    }
}

// Segments thread local data
#[repr(C)]
#[derive(Clone)]
pub struct MiSegmentsTLD {
    pub spans: [MiSpanQueue; MI_SEGMENT_BIN_MAX + 1], // free slice spans inside segments
    pub count: SizeT,                                 // current number of segments
    pub peak_count: SizeT,                            // peak number of segments
    pub current_size: SizeT,                          // current size of all segments
    pub peak_size: SizeT,                             // peak size of all segments
    // pub stats                      : mi_stats_t*      ,                    // points to tld stats
    pub os: *mut MiOsTLD, // points to os stats
}

impl Default for MiSegmentsTLD {
    fn default() -> Self {
        Self {
            spans: [Default::default(); MI_SEGMENT_BIN_MAX + 1],
            count: Default::default(),
            peak_count: Default::default(),
            current_size: Default::default(),
            peak_size: Default::default(),
            os: ptr::null_mut(),
        }
    }
}

// Thread local data
#[repr(C)]
#[derive(Clone)]
pub struct MiTLD {
    pub heartbeat: std::os::raw::c_ulonglong, // monotonic heartbeat count
    pub recurse: bool, // true if deferred was called, used to prevent infinite recursion.
    pub heap_backing: *mut MiHeap, // backing heap of this thread (cannot be deleted)
    pub heaps: *mut MiHeap, // list of heaps in this thread (so we can abandon all when the thread terminates)
    pub segments: MiSegmentsTLD, // segment tld
    pub os: MiOsTLD,        // os tld
                            //pub stats          : mi_stats_t,         // statistics
}

impl Default for MiTLD {
    fn default() -> Self {
        Self {
            heartbeat: Default::default(),
            recurse: Default::default(),
            heap_backing: ptr::null_mut(),
            heaps: ptr::null_mut(),
            segments: Default::default(),
            os: Default::default(),
        }
    }
}

#[repr(C)]
// note: in x64 in release build `sizeof(mi_thread_data_t)` is under 4KiB (= OS page size).
pub struct MiThreadData {
    pub heap: MiHeap, // must come first due to cast in `_mi_heap_done`
    pub tld: MiTLD,
}

impl Default for MiThreadData {
    fn default() -> Self {
        Self {
            heap: Default::default(),
            tld: Default::default(),
        }
    }
}

impl MiThreadData {
    pub fn new() -> Self {
        Default::default()
    }
}
