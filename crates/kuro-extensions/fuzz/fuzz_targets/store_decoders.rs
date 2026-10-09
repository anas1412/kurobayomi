//! TST-19-203: the store and extension-list decoders (JSON and protobuf) never panic.
//! Run: `cargo +nightly fuzz run store_decoders -- -max_total_time=60` from `crates/kuro-extensions`.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| kuro_extensions::fuzz::store_decoders(data));
