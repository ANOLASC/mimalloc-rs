use std::{ffi::c_void, ptr};

fn _mi_arena_alloc_aligned(
    size: usize,
    alignment: usize, /* , size_t align_offset, bool* commit, bool* large, bool* is_pinned, bool* is_zero,
                      mi_arena_id_t req_arena_id, size_t* memid, mi_os_tld_t* tld*/
) -> *mut c_void {
    ptr::null_mut()
}
