/*
The behavior of invalid inputs isn't guaranteed, but we want to make sure we avoid:

- attempting to index out-of-bounds
- infinite loops when iterating
- returning invalid UTF8 strings

everything else is fair game.

There are two kinds of tests here:

- `err_*` tests that are inputs that should be detected as erroneous.
- `invalid_*` tests that are inputs that are erroneous but are accepted.

Many of these cases come from fuzz testing the parser and deciding on semantics when things break.
*/

use crate::{unescape::unescape_trusted, Document};

#[test]
fn err_internal_whitespace() {
    // documents with internal whitespace are detected and considered invalid
    // this is a perfectly valid JSON document, but we optimize for knowing what
    // the next kind of value is based off the byte following a `:`, `[`, or `,`
    let input = b"{\"a\": 42}";

    let document: Document = assert_test_panics!(Document::scan_trusted_fallback(input));

    assert!(document.is_err());
}

#[test]
fn err_incomplete_string() {
    // strings that aren't finished are considered invalid
    let input = b"{\"a\":\"this string is not finished}";

    let document: Document = assert_test_panics!(Document::scan_trusted_fallback(input));

    assert!(document.is_err());
}

#[test]
fn err_incomplete_string_escape() {
    // strings that aren't finished are considered invalid
    // this string ends with an odd number of escapes so isn't
    // considered terminated. This is important for unescaping,
    // which assumes it can always look ahead on an unescaped `\`
    let input = b"{\"a\":\"\\\\\\u\\\\\\\"}";

    let document: Document = assert_test_panics!(Document::scan_trusted_fallback(input));

    assert!(document.is_err());
}

#[test]
fn err_root_level_arr_terminate() {
    // an attempt to terminate an array or map early is considered invalid
    let input = b"{\"a\"],42}";

    let document: Document = assert_test_panics!(Document::scan_trusted_fallback(input));

    assert!(document.is_err());
}

#[test]
fn invalid_escape() {
    // unknown escape sequences are passed through
    let input = b"{\"a\":\"this string as an invalid \\j escape in it\"}";

    let document = Document::scan_trusted_fallback(input);
    drop(document.to_value());
}

#[test]
fn invalid_map_terminated_as_arr() {
    // maps that are terminated with a `]` instead of a `}` are not detected
    // `a` will probably be considered an array now of just the string keys in the map
    let document = Document::scan_trusted_fallback(b"{\"a\":{\"b\":123]}");
    drop(document.to_value());
}

#[test]
fn invalid_arr_terminated_as_map() {
    // arrays that are terminated with a `}` instead of a `]` are not detected
    // `a` will probably be considered a map that interleaves keys and values in the elements
    let document = Document::scan_trusted_fallback(b"{\"a\":[\"b\",\"c\",\"d\"}}");
    drop(document.to_value());
}

#[test]
fn invalid_arr_single_elem_terminated_as_map() {
    // arrays that are terminated with a `}` instead of a `]` are not detected
    // `a` will probably be considered an empty map because it doesn't have at least 2 elements
    let document = Document::scan_trusted_fallback(b"{\"a\":[\"b\"}}");
    drop(document.to_value());
}

#[test]
fn invalid_arr_terminated_as_map_non_string() {
    // arrays that are terminated with a `}` instead of a `]` are not detected
    // `a` will probably be considered an empty map because it doesn't have string keys
    let document = Document::scan_trusted_fallback(b"{\"a\":[{},{}}}");
    drop(document.to_value());
}

#[test]
fn invalid_map_with_missing_key() {
    // documents with a missing key before their value are not detected
    // the document will probably be considered an empty map because it only has one offset
    let document = Document::scan_trusted_fallback(b"{:42e10}");
    drop(document.to_value());
}

#[test]
fn invalid_unescape_unknown() {
    drop(unsafe { unescape_trusted("\\j") });
}

#[test]
fn invalid_unescape_unknown_multibyte() {
    drop(unsafe { unescape_trusted("\\😄 and some more") });
}

#[test]
fn invalid_unescape_utf8_truncated() {
    drop(unsafe { unescape_trusted("\\u58") });
}

#[test]
fn invalid_unescape_utf8_no_escape() {
    drop(unsafe { unescape_trusted("\\u") });
}

#[test]
fn invalid_unescape_utf8_non_digit() {
    drop(unsafe { unescape_trusted("\\u58\\r") });
}

#[test]
fn invalid_unescape_non_digit_multibyte() {
    drop(unsafe { unescape_trusted("\\u壁") });
}

#[test]
fn invalid_unescape_multibyte_non_digit_all_slash() {
    drop(unsafe { unescape_trusted("\\\\\\u\\\\") });
}

#[test]
fn invalid_unescape_surrogate_pair_truncated() {
    drop(unsafe { unescape_trusted("\\ud83d\\ude") });
}

#[test]
fn invalid_unescape_surrogate_pair_non_digit() {
    drop(unsafe { unescape_trusted("\\ud83d\\ude\\r") });
}

#[test]
fn invalid_unescape_surrogate_pair_split() {
    drop(unsafe { unescape_trusted("\\ud83dsome bytes \\ude04") });
}

#[test]
fn invalid_unescape_surrogate_pair() {
    drop(unsafe { unescape_trusted("\\uffff\\uffff") });
}
