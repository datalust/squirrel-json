use super::*;

use std::str;

use crate::{unescape::unescape_trusted, Document};

use serde_json::json;

#[test]
fn read_empty() {
    let document = Document::scan_trusted_fallback(b"");

    assert_eq!(json!({}), document.to_value());
}

#[test]
fn read_empty_map() {
    let document = Document::scan_trusted_fallback(b"{}");

    assert_eq!(json!({}), document.to_value());
}

#[test]
fn read_map_with_trailing_num() {
    let expected = json!({
        "a": 123
    });

    let document = Document::scan_trusted_fallback(b"{\"a\":123}");

    assert_eq!(expected, document.to_value());
}

#[test]
fn read_arr_of_empty_maps() {
    let expected = json!({
        "a": [{},{},{}]
    });

    let document = Document::scan_trusted_fallback(b"{\"a\":[{},{},{}]}");

    assert_eq!(expected, document.to_value());
}

#[test]
fn read_10kb_event_stacktrace_simd_align_start() {
    let input = include_bytes!("../../cases/10kb_event_stacktrace.json") as &[u8];

    let expected: serde_json::Value = serde_json::from_slice(input).unwrap();

    test_alignment(input, 32, |input| {
        let document = Document::scan_trusted(input);

        assert_eq!(expected, document.to_value());

        let offsets = document.into_offsets();
        let document = unsafe { offsets.to_document_unchecked(input) };

        assert_eq!(expected, document.to_value());
    });
}

#[test]
fn read_10kb_event_stacktrace_fallback() {
    let input = include_bytes!("../../cases/10kb_event_stacktrace.json") as &[u8];

    let expected: serde_json::Value = serde_json::from_slice(input).unwrap();

    let document = Document::scan_trusted_fallback(input);

    assert_eq!(expected, document.to_value());
}

#[test]
fn read_600b_event_no_escape_fallback() {
    let input = include_bytes!("../../cases/600b_event_no_escape.json") as &[u8];

    let expected: serde_json::Value = serde_json::from_slice(input).unwrap();

    let document = Document::scan_trusted_fallback(input);

    assert_eq!(expected, document.to_value());
}

#[test]
fn read_600b_event_healthcheck_no_escape_fallback() {
    let input = include_bytes!("../../cases/600b_event_healthcheck_no_escape.json") as &[u8];

    let expected: serde_json::Value = serde_json::from_slice(input).unwrap();

    let document = Document::scan_trusted_fallback(input);

    assert_eq!(expected, document.to_value());
}

#[test]
fn unescape_empty() {
    let input = "";

    let unescaped = unsafe { unescape_trusted(input) };

    assert_eq!(input, unescaped);
}

#[test]
fn unescape_no_escapes() {
    let input = "This string has no escapes";

    let unescaped = unsafe { unescape_trusted(input) };

    assert_eq!(input, unescaped);
}

#[test]
fn unescape_align_start() {
    let input = b"This string has a lot of content \xf0\x9f\x98\x84\\nYou can think of it \\u58c1 like a really big stacktrace.\\nThere are so \\\"many\\\" errors \\ud83d\\ude04 and escaped \\\\ chars in it.\\n";
    let expected = "This string has a lot of content 😄\nYou can think of it 壁 like a really big stacktrace.\nThere are so \"many\" errors 😄 and escaped \\ chars in it.\n";
    test_alignment(input, 32, |input| {
        let unescaped = unsafe { unescape_trusted(str::from_utf8(input).unwrap()) };

        assert_eq!(expected, unescaped);
    });
}

#[test]
fn unescape_tiny() {
    let input = "\\\\";

    let unescaped = unsafe { unescape_trusted(input) };

    assert_eq!("\\", unescaped);
}

#[test]
fn unescape_simple() {
    let input = "this string is escaped\\nit has a newline in it";

    let unescaped = unsafe { unescape_trusted(input) };

    assert_eq!("this string is escaped\nit has a newline in it", unescaped);
}

#[test]
fn unescape_utf8() {
    let input = "\\u58c1";

    let unescaped = unsafe { unescape_trusted(input) };

    assert_eq!("壁", unescaped);
}

#[test]
fn unescape_surrogate_pair() {
    let input = "\\ud83d\\ude04";

    let unescaped = unsafe { unescape_trusted(input) };

    assert_eq!("😄", unescaped);
}

#[cfg(wasm)]
mod wasm {
    use wasm_bindgen_test::*;

    use js_sys::JSON;
    use wasm_bindgen::prelude::*;

    use super::*;

    #[wasm_bindgen_test]
    fn read_stringified_js_object() {
        let expected = json!({
            "a": 42,
            "b": [true, "nah"]
        });

        // Convert the JSON value into a JavaScript object
        let obj = JsValue::from_serde(&expected).expect("failed to build JavaScript object");

        // Call JSON.stringify on our JavaScript object
        let stringified: String = JSON::stringify(&obj).expect("failed to stringify").into();

        // Ensure the output is readable by `scan_trusted`
        let document = Document::scan_trusted(stringified.as_bytes());

        assert_eq!(expected, document.to_value());
    }
}
