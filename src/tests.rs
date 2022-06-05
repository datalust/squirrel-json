// There are code paths that panic in tests, but return an error in regular builds
#![allow(unreachable_code, unused_variables)]

use std::{iter, panic, panic::AssertUnwindSafe};

mod some;

macro_rules! assert_test_panics {
    ($e:expr) => {{
        #[cfg(debug)]
        {
            if std::panic::catch_unwind(|| std::panic::AssertUnwindSafe($e)).is_ok() {
                panic!("expected a panic");
            } else {
                return;
            }
        }

        #[cfg(not(debug))]
        {
            $e
        }
    }};
}

fn test_alignment(input: &[u8], align_up_to: usize, mut f: impl FnMut(&[u8])) {
    for align in 0..align_up_to {
        let mut buf = Vec::<u8>::with_capacity(input.len() + (align_up_to * 4));

        let pad = buf.as_ptr().align_offset(align_up_to) + align_up_to + align;
        buf.extend(iter::repeat(0u8).take(pad));

        let start_alignment =
            unsafe { (buf.last_mut().unwrap() as *mut u8).offset(1) }.align_offset(align_up_to);
        if align == 0 {
            assert_eq!(0, start_alignment);
        } else {
            assert_eq!(32 - align, start_alignment);
        }

        buf.extend(input);

        if let Err(e) = panic::catch_unwind(AssertUnwindSafe(|| f(&buf[pad..]))) {
            panic!("failed at alignment {}", align);
        }
    }
}

mod invalid;
mod valid;
