/*!
Macro helpers for the parser.

Most of these macros perform checked versions of unsafe operations in tests
or when the `checked` feature is enabled just to try catch any UB early.
*/

macro_rules! offset_deref_unchecked {
    ($slice:expr, $offset:expr) => {{
        #[cfg(any(all(test, debug), checked))]
        {
            *($slice)
                .get($offset as usize)
                .expect("attempt to index out of bounds")
        }

        #[cfg(not(any(all(test, debug), checked)))]
        {
            // SAFETY: the offset must always be within the slice
            #[allow(unused_unsafe)]
            unsafe {
                *($slice).as_ptr().offset($offset)
            }
        }
    }};
}

macro_rules! get_unchecked {
    ($slice:expr, $index:expr) => {{
        #[cfg(any(all(test, debug), checked))]
        {
            ($slice)
                .get($index)
                .expect("attempt to index out of bounds")
        }

        #[cfg(not(any(all(test, debug), checked)))]
        {
            // SAFETY: the index must always be in bounds
            #[allow(unused_unsafe)]
            unsafe {
                ($slice).get_unchecked($index)
            }
        }
    }};
}

macro_rules! get_unchecked_mut {
    ($slice:expr, $index:expr) => {{
        #[cfg(any(all(test, debug), checked))]
        {
            ($slice)
                .get_mut($index)
                .expect("attempt to index out of bounds")
        }

        #[cfg(not(any(all(test, debug), checked)))]
        {
            // SAFETY: the index must always be in bounds
            #[allow(unused_unsafe)]
            unsafe {
                ($slice).get_unchecked_mut($index)
            }
        }
    }};
}

macro_rules! from_utf8_unchecked {
    ($str:expr) => {{
        #[cfg(any(all(test, debug), checked))]
        {
            std::str::from_utf8($str).expect("invalid utf8")
        }

        #[cfg(not(any(all(test, debug), checked)))]
        {
            // SAFETY: the input must always be valid UTF8
            #[allow(unused_unsafe)]
            unsafe {
                std::str::from_utf8_unchecked($str)
            }
        }
    }};
}

macro_rules! owned_from_utf8_unchecked {
    ($str:expr) => {{
        #[cfg(any(all(test, debug), checked))]
        {
            String::from_utf8($str).expect("invalid utf8")
        }

        #[cfg(not(any(all(test, debug), checked)))]
        {
            // SAFETY: the input must always be valid UTF8
            #[allow(unused_unsafe)]
            unsafe {
                String::from_utf8_unchecked($str)
            }
        }
    }};
}

macro_rules! offset_from_raw_parts {
    ($base_ptr:expr, $base_len:expr, $offset:expr, $len:expr) => {{
        #[cfg(any(all(test, debug), checked))]
        {
            let base_ptr = $base_ptr;
            let base_len = $base_len;
            let offset = $offset;
            let len = $len;

            assert!(offset + len <= (base_ptr as usize) + base_len);

            // SAFETY: the input must always be within the slice
            #[allow(unused_unsafe)]
            unsafe {
                std::slice::from_raw_parts((base_ptr).add(offset), len)
            }
        }

        #[cfg(not(any(all(test, debug), checked)))]
        {
            // SAFETY: the input must always be within the slice
            #[allow(unused_unsafe)]
            unsafe {
                std::slice::from_raw_parts(($base_ptr).add($offset), $len)
            }
        }
    }};
}

macro_rules! test_assert {
    ($($tokens:tt)*) => {{
        #[cfg(test)]
        {
            debug_assert!($($tokens)*);
        }
    }};
}

macro_rules! test_assert_eq {
    ($($tokens:tt)*) => {{
        #[cfg(test)]
        {
            debug_assert_eq!($($tokens)*);
        }
    }};
}

macro_rules! test_unreachable {
    ($($tokens:tt)*) => {
        #[cfg(all(debug, test))]
        {
            unreachable!($($tokens)*);
        }
    };
}
