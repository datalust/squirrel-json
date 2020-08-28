pub fn de(input: &[u8]) {
    // Make sure we don't panic when reading documents
    let simd = squirrel_json::Document::scan_trusted(input);
    let fallback = squirrel_json::Document::scan_trusted_fallback(input);

    if !simd.is_err() && !fallback.is_err() {
        if serde_json::from_slice::<serde_json::Value>(input).is_ok() {
            // If all parsers manage to read the document then make sure they're equal
            let simd_value = simd.to_value();
            let fallback_value = fallback.to_value();

            assert_eq!(simd_value, fallback_value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::{fs, io::Read};

    #[test]
    fn inputs() {
        if let Ok(inputs) = fs::read_dir("../in") {
            for input in inputs {
                let input = input.expect("invalid file").path();

                println!("input: {:?}", input);

                let mut f = fs::File::open(input).expect("failed to open");
                let mut input = Vec::new();
                f.read_to_end(&mut input).expect("failed to read file");

                // Just make sure we never panic
                de(&input);
            }
        }
    }

    #[test]
    fn crashes() {
        if let Ok(crashes) = fs::read_dir("../../target/fuzz_scan_trusted/crashes") {
            for crash in crashes {
                let crash = crash.expect("invalid file").path();

                println!("repro: {:?}", crash);

                let mut f = fs::File::open(crash).expect("failed to open");
                let mut crash = Vec::new();
                f.read_to_end(&mut crash).expect("failed to read file");

                // Just make sure we never panic
                de(&crash);
            }
        }
    }
}
