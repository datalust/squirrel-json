fn main() {
    #![allow(unreachable_code)]

    #[cfg(not(checked))]
    panic!("fuzz tests need to be run in `checked` mode by setting the `SQUIRRELJSON_CHECKED` environment variable.");

    #[cfg(not(feature = "afl"))]
    panic!("fuzz tests need to be run with the `afl` Cargo feature.");

    #[cfg(feature = "afl")]
    afl::fuzz!(|input: &[u8]| { fuzz_scan_trusted::de(input) });
}
