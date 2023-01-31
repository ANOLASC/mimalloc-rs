use std::{ffi::c_void, ptr};

use crate::mimalloc_types::MiHeap;

fn _mi_malloc_generic(
    heap: Box<MiHeap>,
    size: usize,
    zero: bool,
    huge_alignment: usize,
) -> *mut c_void {
    ptr::null_mut()
}
