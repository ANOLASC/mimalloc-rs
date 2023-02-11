use ctor::{ctor, dtor};
use windows::Win32::System::Threading::{FlsAlloc, FlsSetValue};

use crate::heap::mi_heap_delete;
use crate::mimalloc_internal::{_mi_thread_id, get_default_heap, mi_heap_is_initialized};
use crate::mimalloc_types::{MiHeap, MiTLD, MiThreadData, MiThreadId};
use crate::random::_mi_heap_random_next;
use std::mem::MaybeUninit;
use std::os::raw::c_void;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Once;

pub fn get_mi_heap_main() -> &'static mut MiHeap {
    static mut MiHeapMain: MaybeUninit<MiHeap> = MaybeUninit::uninit();
    static ONCE: Once = Once::new();
    unsafe {
        ONCE.call_once(|| {
            MiHeapMain.write(MiHeap::new());
        });
        MiHeapMain.assume_init_mut()
    }
}

// Initialize the thread local default heap, called from `mi_thread_init`
fn mi_heap_init() -> bool {
    if mi_heap_is_initialized(get_default_heap().as_ref()) {
        return true;
    }

    if mi_is_main_thread() {
        mi_heap_main_init();
        _mi_heap_set_default_direct(get_mi_heap_main());
    } else {
        let td: *mut MiThreadData = mi_thread_data_alloc();
        // TODO should use Option instead?
        if td.is_null() {
            return false;
        }

        // OS allocated so already zero initialized
        let tld: *mut MiTLD = unsafe { &mut (*td).tld };
        let heap: *mut MiHeap = unsafe { &mut (*td).heap };
        // TODO initialize tld and heap by using memcpy
        // Do I really need to use memcpy?

        unsafe {
            (*heap).thread_id = _mi_thread_id();

            // TODO currently, do not random the heap
            // _mi_random_init(&heap->random);
            // (*heap).cookie = _mi_heap_random_next(heap) | 1;
            // heap->keys[0] = _mi_heap_random_next(heap);
            // heap->keys[1] = _mi_heap_random_next(heap);
            (*heap).tld = tld;
            (*tld).heap_backing = heap;
            (*tld).heaps = heap;
            // (*tld).segments.stats = &(*tld).stats;
            (*tld).segments.os = &mut (*tld).os;
            // (*tld).os.stats = &(*tld).stats;
            // _mi_heap_set_default_direct(heap);
        }
    }

    false
}

fn mi_thread_data_alloc() -> *mut MiThreadData {
    ptr::null_mut()
}

static MI_PROCESS_IS_INITIALIZED: AtomicBool = AtomicBool::new(false);

#[ctor]
fn mi_proces_init() {
    if MI_PROCESS_IS_INITIALIZED.load(Ordering::Acquire) {
        return;
    }

    MI_PROCESS_IS_INITIALIZED.store(true, Ordering::Release);

    // TODO log here
    mi_detect_cpu_feature();
    mi_os_init();

    mi_heap_main_init();
    mi_thread_init();
    if cfg!(Win32) {
        // TODO check lately here
        // FlsSetValue(mi_fls_key, NULL);
    }

    // TODO support option reserve huge os page and reserve os memory
}

fn mi_os_init() {
    // TODO
}

fn mi_detect_cpu_feature() {
    // detect cpu feature
    // TODO to be implemented
}

#[dtor]
// called when process is done
fn mi_process_done() {
    if !MI_PROCESS_IS_INITIALIZED.load(Ordering::Acquire) {
        return;
    }

    static PROCESS_DONE: AtomicBool = AtomicBool::new(false);
    if PROCESS_DONE.load(Ordering::Acquire) {
        return;
    }

    PROCESS_DONE.store(true, Ordering::Release);

    // TODO FlsFree here

    // TODO support feture destroy on exit

    mi_allocator_done();
}

fn mi_allocator_done() {
    // nothing to do
}

// Called once by the process loader
fn mi_process_load() {
    mi_heap_main_init();
    debug_assert!(mi_is_main_thread());
    mi_option_init();
}

// called from `mi_malloc_generic`
fn mi_thread_init() {
    // ensure process has started already
    mi_proces_init();

    if mi_heap_init() {}
}

static THREAD_COUNT: AtomicUsize = AtomicUsize::new(1);

// called by DllMain, currently, do not implement
fn mi_thread_done() {
    _mi_thread_done(get_default_heap().as_mut());
}

fn _mi_thread_done(heap: *mut MiHeap) {
    THREAD_COUNT.fetch_sub(1, Ordering::Relaxed);

    // mi_atomic_decrement_relaxed(&thread_count);
    // _mi_stat_decrease(&_mi_stats_main.threads, 1);

    // check thread-id as on Windows shutdown with FLS the main (exit) thread may call this on thread-local heaps...
    unsafe {
        if (*heap).thread_id != _mi_thread_id() {
            return;
        }
    }

    // abandon the thread local heap
    _mi_heap_done(heap);
}

// Free the thread local default heap (called from `mi_thread_done`)
fn _mi_heap_done(heap: *mut MiHeap) -> bool {
    if !mi_heap_is_initialized(heap) {
        return true;
    }

    // reset default heap
    if mi_is_main_thread() {
        _mi_heap_set_default_direct(get_mi_heap_main());
    } else {
        // TDOO
        // _mi_heap_set_default_direct()
    }

    // delete all non-backing heaps in this thread
    let mut curr = unsafe { (*(*heap).tld).heaps };

    while !curr.is_null() {
        let next = unsafe { (*curr).next };
        if curr != heap {
            mi_heap_delete(curr);
        }
        curr = next;
    }

    // mi_assert_internal(heap->tld->heaps == heap && heap->next == NULL);
    // mi_assert_internal(mi_heap_is_backing(heap));

    //     // collect if not the main thread
    //   if (heap != &_mi_heap_main) {
    //     _mi_heap_collect_abandon(heap);
    //   }

    //   // merge stats
    //   _mi_stats_done(&heap->tld->stats);

    //   // free if not the main thread
    //   if (heap != &_mi_heap_main) {
    //     // the following assertion does not always hold for huge segments as those are always treated
    //     // as abondened: one may allocate it in one thread, but deallocate in another in which case
    //     // the count can be too large or negative. todo: perhaps not count huge segments? see issue #363
    //     // mi_assert_internal(heap->tld->segments.count == 0 || heap->thread_id != _mi_thread_id());
    //     mi_thread_data_free((mi_thread_data_t*)heap);
    //   }
    //   else {
    //     mi_thread_data_collect(); // free cached thread metadata
    //     #if 0
    //     // never free the main thread even in debug mode; if a dll is linked statically with mimalloc,
    //     // there may still be delete/free calls after the mi_fls_done is called. Issue #207
    //     _mi_heap_destroy_pages(heap);
    //     mi_assert_internal(heap->tld->heap_backing == &_mi_heap_main);
    //     #endif
    //   }
    false
}

fn mi_is_main_thread() -> bool {
    get_mi_heap_main().thread_id == 0 || _mi_thread_id() == get_mi_heap_main().thread_id
}

fn mi_heap_main_init() {
    if get_mi_heap_main().cookie == 0 {
        get_mi_heap_main().thread_id = _mi_thread_id();
        get_mi_heap_main().cookie = 1;
        //   #if defined(_WIN32) && !defined(MI_SHARED_LIB)
        //     _mi_random_init_weak(&_mi_heap_main.random);    // prevent allocation failure during bcrypt dll initialization with static linking
        //   #else
        //     _mi_random_init(&_mi_heap_main.random);
        //   #endif
        get_mi_heap_main().cookie = _mi_heap_random_next(&mut get_mi_heap_main().random);
        get_mi_heap_main().keys[0] = _mi_heap_random_next(&mut get_mi_heap_main().random);
        get_mi_heap_main().keys[1] = _mi_heap_random_next(&mut get_mi_heap_main().random);
    }
}

fn mi_option_init() {}
// TODO should MI_FLS_KEY use thread local?
//thread_local! (static MI_FLS_KEY: u32 = u32::MAX);
static mut MI_FLS_KEY: u32 = u32::MAX;

// TODO stdcall or system?
unsafe extern "system" fn mi_fls_done(value: *const c_void) {
    let heap: *mut MiHeap = value.cast_mut().cast();
    if !heap.is_null() {
        _mi_thread_done(heap);
        unsafe {
            // FlsSetValue(MI_FLS_KEY.with(|v| *v.borrow()), None);
            FlsSetValue(MI_FLS_KEY, None);
        } // prevent recursion as _mi_thread_done may set it back to the main heap, issue #672
    }
}

fn mi_process_setup_auto_thread_done() {
    static TLS_INITIALIZED: AtomicBool = AtomicBool::new(false);
    if TLS_INITIALIZED.load(Ordering::Acquire) {
        return;
    }
    TLS_INITIALIZED.store(true, Ordering::Release);

    // TODO should check carefully
    unsafe { MI_FLS_KEY = FlsAlloc(Some(mi_fls_done)) };

    _mi_heap_set_default_direct(get_mi_heap_main());
}

fn _mi_heap_set_default_direct(heap: *mut MiHeap) {
    unsafe {
        FlsSetValue(MI_FLS_KEY, Some(heap.cast()));
    }
}
