use crate::mimalloc_types::MiOption;

enum MiInit {
    UNINIT,      // not yet initialized
    DEFAULTED,   // not found in the environment, use default value
    INITIALIZED, // found in environment or set explicitly
}

struct MiOptionDesc {
    pub value: i64,          // the value
    pub init: MiInit,        // is it initialized yet? (from the environment)
    pub option: MiOption,    // for debugging: the option index should match the option
    pub name: String,        // option name without `mimalloc_` prefix
    pub legacy_name: String, // potential legacy v1.x option name
}

// should deprecated if mem::variant_count is stable [https://github.com/rust-lang/rust/issues/73662]
const MI_OPTION_LAST: usize = 3;

// should carefully design it
//   static options: [MiOptionDesc; MI_OPTION_LAST] =
//   {
//     // stable options
//     // #if MI_DEBUG || defined(MI_SHOW_ERRORS)
//     // { 1, UNINIT, MI_OPTION(show_errors) },
//     // #else
//     // { 0, UNINIT, MI_OPTION(show_errors) },
//     // #endif
//     // { 0, UNINIT, MI_OPTION(show_stats) },
//     // { 0, UNINIT, MI_OPTION(verbose) },

//     // // Some of the following options are experimental and not all combinations are valid. Use with care.
//     // { 1, UNINIT, MI_OPTION(eager_commit) },        // commit per segment directly (8MiB)  (but see also `eager_commit_delay`)
//     // { 0, UNINIT, MI_OPTION(deprecated_eager_region_commit) },
//     // { 0, UNINIT, MI_OPTION(deprecated_reset_decommits) },
//     // { 0, UNINIT, MI_OPTION(large_os_pages) },      // use large OS pages, use only with eager commit to prevent fragmentation of VMA's
//     // { 0, UNINIT, MI_OPTION(reserve_huge_os_pages) },  // per 1GiB huge pages
//     // { -1, UNINIT, MI_OPTION(reserve_huge_os_pages_at) }, // reserve huge pages at node N
//     // { 0, UNINIT, MI_OPTION(reserve_os_memory)     },
//     // { 0, UNINIT, MI_OPTION(deprecated_segment_cache) },  // cache N segments per thread
//     // { 0, UNINIT, MI_OPTION(page_reset) },          // reset page memory on free
//     // { 0, UNINIT, MI_OPTION_LEGACY(abandoned_page_decommit, abandoned_page_reset) },// decommit free page memory when a thread terminates
//     // { 0, UNINIT, MI_OPTION(deprecated_segment_reset) },
//     // #if defined(__NetBSD__)
//     // { 0, UNINIT, MI_OPTION(eager_commit_delay) },  // the first N segments per thread are not eagerly committed
//     // #elif defined(_WIN32)
//     // { 4, UNINIT, MI_OPTION(eager_commit_delay) },  // the first N segments per thread are not eagerly committed (but per page in the segment on demand)
//     // #else
//     // { 1, UNINIT, MI_OPTION(eager_commit_delay) },  // the first N segments per thread are not eagerly committed (but per page in the segment on demand)
//     // #endif
//     // { 25,   UNINIT, MI_OPTION_LEGACY(decommit_delay, reset_delay) }, // page decommit delay in milli-seconds
//     // { 0,    UNINIT, MI_OPTION(use_numa_nodes) },    // 0 = use available numa nodes, otherwise use at most N nodes.
//     // { 0,    UNINIT, MI_OPTION(limit_os_alloc) },    // 1 = do not use OS memory for allocation (but only reserved arenas)
//     // { 100,  UNINIT, MI_OPTION(os_tag) },            // only apple specific for now but might serve more or less related purpose
//     // { 16,   UNINIT, MI_OPTION(max_errors) },        // maximum errors that are output
//     // { 16,   UNINIT, MI_OPTION(max_warnings) },      // maximum warnings that are output
//     // { 8,    UNINIT, MI_OPTION(max_segment_reclaim)},// max. number of segment reclaims from the abandoned segments per try.
//     // { 1,    UNINIT, MI_OPTION(allow_decommit) },    // decommit slices when no longer used (after decommit_delay milli-seconds)
//     // { 500,  UNINIT, MI_OPTION(segment_decommit_delay) }, // decommit delay in milli-seconds for freed segments
//     // { 1,    UNINIT, MI_OPTION(decommit_extend_delay) },
//     // { 0,    UNINIT, MI_OPTION(destroy_on_exit)}     // release all OS memory on process exit; careful with dangling pointer or after-exit frees!
//   };

pub fn mi_option_get(option: MiOption) -> i64 {
    // if option < 0 || option >= _mi_option_last {return 0;}
    // let desc = &options[option];
    // debug_assert!(desc->option == option);  // index should match the option
    // if desc->init == UNINIT {
    //   mi_option_init(desc);
    // }
    // return desc->value;
    0
}

pub fn mi_option_is_enabled(option: MiOption) -> bool {
    true
    // return mi_option_get(option) != 0;
}

pub fn mi_option_get_clamp(option: MiOption, min: i64, max: i64) -> i64 {
    let x = mi_option_get(option);

    if x < min {
        min
    } else if x > max {
        max
    } else {
        x
    }
}
