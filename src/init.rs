use crate::mimalloc_types::MiHeap;
use std::mem::MaybeUninit;
use std::sync::{Mutex, Once};

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

fn mi_heap_init() {

}

fn mi_heap_done() {

}

fn mi_proces_init() {

}

fn mi_process_done() {

}

// Called once by the process loader
fn mi_process_load() {
    
}

fn mi_thread_init() {

}

fn mi_thread_done() {

}

fn mi_is_main_thread() {

}