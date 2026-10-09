//! TST-19-202: the manifest parsers (the anime package manifest and the manga `manifest.toml`) never panic.
//! Run: `cargo +nightly fuzz run manifest -- -max_total_time=60` from `crates/kuro-extensions`.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| kuro_extensions::fuzz::manifest(data));
