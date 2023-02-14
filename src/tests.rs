// #[cfg(test)]
// use paste::paste;

// use crate::mimalloc_types::MiHeap;

use crate::init::_mi_heap_init;

macro_rules! test_layout {
    ($type: ty, $size: expr, $align: expr) => {
        paste! {
            #[test]
            fn [<test_layout_ $type:snake>]() {
                assert_eq!(
                    ::std::mem::size_of::<$type>(),
                    [<$size usize>],
                    concat!("Size of: ", stringify!($type))
                );
                assert_eq!(
                    ::std::mem::align_of::<$type>(),
                    [<$align usize>],
                    concat!("Alignment of ", stringify!($type))
                );
            }
        }
    };
}

// wrong test, should not pass
// test_layout!(MiHeap, 12, 12);

fn test_init() {
    _mi_heap_init();
}

#[test]
fn success() {
    test_init();
    assert_eq!(1, 1);
}

// fn bindgen_test_layout_mi_random_cxt_s() {
//     assert_eq!(
//         ::std::mem::size_of::<mi_random_cxt_s>(),
//         136usize,
//         concat!("Size of: ", stringify!(mi_random_cxt_s))
//     );
//     assert_eq!(
//         ::std::mem::align_of::<mi_random_cxt_s>(),
//         4usize,
//         concat!("Alignment of ", stringify!(mi_random_cxt_s))
//     );
//     assert_eq!(
//         unsafe { &(*(::std::ptr::null::<mi_random_cxt_s>())).input as *const _ as usize },
//         0usize,
//         concat!(
//             "Offset of field: ",
//             stringify!(mi_random_cxt_s),
//             "::",
//             stringify!(input)
//         )
//     );
//     assert_eq!(
//         unsafe { &(*(::std::ptr::null::<mi_random_cxt_s>())).output as *const _ as usize },
//         64usize,
//         concat!(
//             "Offset of field: ",
//             stringify!(mi_random_cxt_s),
//             "::",
//             stringify!(output)
//         )
//     );
//     assert_eq!(
//         unsafe {
//             &(*(::std::ptr::null::<mi_random_cxt_s>())).output_available as *const _ as usize
//         },
//         128usize,
//         concat!(
//             "Offset of field: ",
//             stringify!(mi_random_cxt_s),
//             "::",
//             stringify!(output_available)
//         )
//     );
//     assert_eq!(
//         unsafe { &(*(::std::ptr::null::<mi_random_cxt_s>())).weak as *const _ as usize },
//         132usize,
//         concat!(
//             "Offset of field: ",
//             stringify!(mi_random_cxt_s),
//             "::",
//             stringify!(weak)
//         )
//     );
// }
