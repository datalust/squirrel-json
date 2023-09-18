#![cfg(unstable)]
#![feature(test)]
extern crate test;

use squirrel_json::Document;

use std::str;

#[bench]
fn read_10kb_event_stacktrace_offsets_simd(b: &mut test::Bencher) {
    let input = include_bytes!("../cases/10kb_event_stacktrace.json");

    b.bytes = input.len() as u64;
    b.iter(|| Document::scan_trusted(input))
}

#[bench]
fn read_10kb_event_stacktrace_offsets_simd_from_const_parts(b: &mut test::Bencher) {
    let input = include_bytes!("../cases/10kb_event_stacktrace.json");
    let const_offsets = Document::scan_trusted(input).into_offsets();

    b.bytes = input.len() as u64;
    b.iter(|| unsafe { const_offsets.to_document_unchecked(input) })
}

#[bench]
fn read_10kb_event_stacktrace_offsets_simd_reuse(b: &mut test::Bencher) {
    let input = include_bytes!("../cases/10kb_event_stacktrace.json");
    let mut detached = Some(Document::scan_trusted(input).detach());

    b.bytes = input.len() as u64;
    b.iter(|| {
        let reuse = detached.take().unwrap();

        let doc = Document::scan_trusted_attach(input, reuse);
        test::black_box(&doc);

        detached = Some(doc.detach());
    })
}

#[bench]
fn read_10kb_event_stacktrace_offsets_fallback(b: &mut test::Bencher) {
    let input = include_bytes!("../cases/10kb_event_stacktrace.json");

    b.bytes = input.len() as u64;
    b.iter(|| Document::scan_trusted_fallback(input))
}

#[bench]
fn read_10kb_event_stacktrace_value_serde_json(b: &mut test::Bencher) {
    let input = include_bytes!("../cases/10kb_event_stacktrace.json");

    b.bytes = input.len() as u64;
    b.iter(|| {
        let v: serde_json::Value = serde_json::from_slice(input).unwrap();
        v
    })
}

#[bench]
fn read_10kb_event_stacktrace_value_json(b: &mut test::Bencher) {
    let input = include_bytes!("../cases/10kb_event_stacktrace.json");

    b.bytes = input.len() as u64;
    b.iter(|| {
        let v: json::JsonValue = json::parse(str::from_utf8(input).unwrap()).unwrap();
        v
    })
}

#[bench]
fn read_10kb_event_stacktrace_value_simd_json(b: &mut test::Bencher) {
    let input = include_bytes!("../cases/10kb_event_stacktrace.json");

    b.bytes = input.len() as u64;
    b.iter(|| {
        let mut input = input.to_vec();
        let v = simd_json::to_borrowed_value(&mut input).unwrap();
        test::black_box(v);
    })
}

#[bench]
fn read_10kb_event_stacktrace_value_to_vec(b: &mut test::Bencher) {
    let input = include_bytes!("../cases/10kb_event_stacktrace.json");

    b.bytes = input.len() as u64;
    b.iter(|| input.to_vec())
}

#[bench]
fn read_10kb_event_stacktrace_split(b: &mut test::Bencher) {
    let input = include_str!("../cases/10kb_event_stacktrace.json");

    b.bytes = input.len() as u64;
    b.iter(|| input.split('"').for_each(drop))
}

#[bench]
fn read_10kb_event_stacktrace_validate_utf8(b: &mut test::Bencher) {
    let input = include_bytes!("../cases/10kb_event_stacktrace.json");

    b.bytes = input.len() as u64;
    b.iter(|| str::from_utf8(input).unwrap())
}

#[bench]
#[cfg(feature = "serde_json")]
fn read_10kb_event_stacktrace_offsets_simd_to_serde_json(b: &mut test::Bencher) {
    let input = include_bytes!("../cases/10kb_event_stacktrace.json");

    b.bytes = input.len() as u64;
    b.iter(|| Document::scan_trusted(input).to_value())
}

#[bench]
#[cfg(feature = "serde_json")]
fn convert_10kb_event_stacktrace_offsets_to_serde_json(b: &mut test::Bencher) {
    let input = include_bytes!("../cases/10kb_event_stacktrace.json");

    let doc = Document::scan_trusted(input);

    b.bytes = input.len() as u64;
    b.iter(|| doc.to_value())
}

#[bench]
fn read_10kb_event_stacktrace_offsets_simd_sparse(b: &mut test::Bencher) {
    let input = include_bytes!("../cases/10kb_event_stacktrace.json");

    b.bytes = input.len() as u64;
    b.iter(|| {
        let doc = Document::scan_trusted(input);

        let m = doc
            .as_map()
            .entries()
            .filter_map(|(k, v)| if k.as_raw() == "@m" { Some(v) } else { None })
            .next()
            .unwrap()
            .as_str()
            .unwrap();

        m.to_unescaped().into_owned()
    })
}

#[bench]
fn read_10kb_event_stacktrace_serde_json_sparse(b: &mut test::Bencher) {
    let input = include_bytes!("../cases/10kb_event_stacktrace.json");

    b.bytes = input.len() as u64;
    b.iter(|| {
        let v: serde_json::Value = serde_json::from_slice(input).unwrap();

        let mut doc = match v {
            serde_json::Value::Object(doc) => doc,
            _ => panic!("expected a map"),
        };

        let m = match doc.remove("@m").unwrap() {
            serde_json::Value::String(m) => m,
            _ => panic!("expected a string"),
        };

        m
    })
}

#[bench]
fn unescape_10kb_event_stacktrace(b: &mut test::Bencher) {
    let input = include_bytes!("../cases/10kb_event_stacktrace.json");

    let doc = Document::scan_trusted(input);

    let stacktrace = doc
        .as_map()
        .entries()
        .filter_map(|(k, v)| if k.as_raw() == "@x" { Some(v) } else { None })
        .next()
        .unwrap()
        .as_str()
        .unwrap();

    b.bytes = input.len() as u64;
    b.iter(|| stacktrace.to_unescaped())
}

#[bench]
fn unescape_10kb_event_stacktrace_to_string(b: &mut test::Bencher) {
    let input = include_bytes!("../cases/10kb_event_stacktrace.json");

    let doc = Document::scan_trusted(input);

    let stacktrace = doc
        .as_map()
        .entries()
        .filter_map(|(k, v)| if k.as_raw() == "@x" { Some(v) } else { None })
        .next()
        .unwrap()
        .as_str()
        .unwrap();

    b.bytes = input.len() as u64;
    b.iter(|| stacktrace.as_raw().to_owned())
}

#[bench]
fn iter_top_level_entries_600b_event_no_escape(b: &mut test::Bencher) {
    let input = include_bytes!("../cases/600b_event_no_escape.json");
    let document = Document::scan_trusted(input);

    b.bytes = input.len() as u64;
    b.iter(|| document.as_map().entries().for_each(drop))
}

#[bench]
fn read_600b_event_no_escape_offsets_simd(b: &mut test::Bencher) {
    let input = include_bytes!("../cases/600b_event_no_escape.json");

    b.bytes = input.len() as u64;
    b.iter(|| Document::scan_trusted(input))
}

#[bench]
fn read_600b_event_no_escape_offsets_fallback(b: &mut test::Bencher) {
    let input = include_bytes!("../cases/600b_event_no_escape.json");

    b.bytes = input.len() as u64;
    b.iter(|| Document::scan_trusted_fallback(input))
}

#[bench]
fn read_600b_event_no_escape_value_serde_json(b: &mut test::Bencher) {
    let input = include_bytes!("../cases/600b_event_no_escape.json");

    b.bytes = input.len() as u64;
    b.iter(|| {
        let v: serde_json::Value = serde_json::from_slice(input).unwrap();
        v
    })
}

#[bench]
fn read_600b_event_no_escape_value_json(b: &mut test::Bencher) {
    let input = include_bytes!("../cases/600b_event_no_escape.json");

    b.bytes = input.len() as u64;
    b.iter(|| {
        let v: json::JsonValue = json::parse(str::from_utf8(input).unwrap()).unwrap();
        v
    })
}

#[bench]
fn read_600b_event_no_escape_value_simd_json(b: &mut test::Bencher) {
    let input = include_bytes!("../cases/600b_event_no_escape.json");

    b.bytes = input.len() as u64;
    b.iter(|| {
        let mut input = input.to_vec();
        let v = simd_json::to_borrowed_value(&mut input).unwrap();
        test::black_box(v);
    })
}

#[bench]
fn read_600b_event_no_escape_value_to_vec(b: &mut test::Bencher) {
    let input = include_bytes!("../cases/600b_event_no_escape.json");

    b.bytes = input.len() as u64;
    b.iter(|| input.to_vec())
}

#[bench]
fn read_600b_event_no_escape_split(b: &mut test::Bencher) {
    let input = include_str!("../cases/600b_event_no_escape.json");

    b.bytes = input.len() as u64;
    b.iter(|| input.split('"').for_each(drop))
}

#[bench]
fn read_600b_event_no_escape_validate_utf8(b: &mut test::Bencher) {
    let input = include_bytes!("../cases/600b_event_no_escape.json");

    b.bytes = input.len() as u64;
    b.iter(|| str::from_utf8(input).unwrap())
}
