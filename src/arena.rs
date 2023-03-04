use std::{ffi::c_void, ptr};

use crate::mimalloc_types::{MiArenaIdT, MiOsTLD};

pub fn _mi_arena_alloc_aligned(
    size: usize,
    alignment: usize,
    align_offset: usize,
    commit: *mut bool,
    large: *mut bool,
    is_pinned: *mut bool,
    is_zero: *mut bool,
    req_arena_id: MiArenaIdT,
    memid: *mut usize,
    tld: *mut MiOsTLD,
) -> *mut c_void {
    ptr::null_mut()
}
