use super::*;

use std::str;

use crate::{tests::some, unescape::unescape_trusted, Document};

use serde_json::json;

#[test]
fn read_cases() {
    for input in [
        include_bytes!("../../cases/serilog_embedded.json") as &[u8],
        include_bytes!("../../cases/10kb_event_stacktrace.json") as &[u8],
        include_bytes!("../../cases/600b_event_no_escape.json") as &[u8],
        include_bytes!("../../cases/600b_event_healthcheck_no_escape.json") as &[u8],
    ] {
        // Check the document using the fallback parser
        let expected: serde_json::Value = serde_json::from_slice(input).unwrap();

        let document = Document::scan_trusted_fallback(input);

        assert_eq!(expected, document.to_value());

        // Check the document using the vectorized parser
        test_alignment(input, 32, |input| {
            let document = Document::scan_trusted(input);

            assert_eq!(expected, document.to_value());

            let offsets = document.into_offsets();
            let document = unsafe { offsets.to_document_unchecked(input) };

            assert_eq!(expected, document.to_value());
        });
    }
}

#[test]
fn read_generated() {
    // debug builds are slow, so just run a handful of cases
    let iterations = {
        #[cfg(debug)]
        {
            100
        }

        #[cfg(not(debug))]
        {
            2000
        }
    };

    for _ in 0..iterations {
        // Check the parser against some randomly generated JSON data
        // Fuzzing is good at finding bizarre and invalid almost-JSON
        // but doesn't discover valid JSON very often. This approach
        // stampedes with a bunch of valid combinations of JSON objects
        // to ensure the parser is always correct for all correct JSON
        // objects without internal whitespace
        let input = some::json_object();

        let expected: serde_json::Value = match serde_json::from_str(&input) {
            Ok(v) => v,
            Err(e) => {
                panic!("parsing `{}`: {}", input, e);
            }
        };

        let document = Document::scan_trusted_fallback(input.as_bytes());

        assert_eq!(expected, document.to_value());

        // Check the document using the vectorized parser
        test_alignment(input.as_bytes(), 32, |input| {
            let document = Document::scan_trusted(input);

            assert_eq!(expected, document.to_value());

            let offsets = document.into_offsets();
            let document = unsafe { offsets.to_document_unchecked(input) };

            assert_eq!(expected, document.to_value());
        });
    }
}

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
fn read_arr_of_numbers() {
    let expected = json!({
        "a": [
            34785u64,
            78234.2f64,
        ]
    });

    let document = Document::scan_trusted_fallback(b"{\"a\":[34785,78234.2]}");

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
