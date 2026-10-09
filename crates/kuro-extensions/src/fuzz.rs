//! Entry points of the fuzz targets in `fuzz/` (T-19a-068, TST-19-202, TST-19-203). Each takes arbitrary bytes and must
//! return, never panic. The same functions run in the time-boxed mutation test below, which needs no nightly compiler:
//! `cargo test -p kuro-extensions fuzz_60s -- --ignored`.

use crate::anime::package::manifest::Manifest as PackageManifest;

/// Both manifest parsers: the anime package manifest (JSON, with its validation rules) and the manga `manifest.toml`.
pub fn manifest(bytes: &[u8]) {
    if let Ok(manifest) = PackageManifest::parse(bytes) {
        let _ = manifest.validate(&|name| name.len() % 2 == 0);
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        let _ = crate::Manifest::parse(text);
    }
}

/// Every store and extension-list decoder, JSON and protobuf.
pub fn store_decoders(bytes: &[u8]) {
    crate::anime::store::decode_all(bytes);
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;

    fn xorshift(seed: &mut u64) -> u64 {
        *seed ^= *seed << 13;
        *seed ^= *seed >> 7;
        *seed ^= *seed << 17;
        *seed
    }

    /// Mutates the seeds (truncate and overwrite bytes) and feeds random bytes, for `budget`.
    fn run(target: fn(&[u8]), seeds: &[&[u8]], budget: Duration) -> u64 {
        let (start, mut seed, mut rounds) = (Instant::now(), 0x9E37_79B9_7F4A_7C15_u64, 0_u64);
        while start.elapsed() < budget {
            for _ in 0..1000 {
                let body: Vec<u8> = if rounds % 2 == 0 {
                    let mut bytes = seeds[(xorshift(&mut seed) as usize) % seeds.len()].to_vec();
                    let cut = (xorshift(&mut seed) as usize) % (bytes.len() + 1);
                    bytes.truncate(cut);
                    for _ in 0..(xorshift(&mut seed) % 4) {
                        if !bytes.is_empty() {
                            let at = (xorshift(&mut seed) as usize) % bytes.len();
                            bytes[at] = xorshift(&mut seed) as u8;
                        }
                    }
                    bytes
                } else {
                    (0..(xorshift(&mut seed) % 96)).map(|_| xorshift(&mut seed) as u8).collect()
                };
                target(&body);
                rounds += 1;
            }
        }
        rounds
    }

    const MANIFEST_SEEDS: [&[u8]; 2] =
        [include_bytes!("../fuzz/corpus/manifest/anime.json"), include_bytes!("../fuzz/corpus/manifest/manga.toml")];
    const STORE_SEEDS: [&[u8]; 2] =
        [include_bytes!("../fuzz/corpus/store_decoders/golden_store.pb"), include_bytes!("../fuzz/corpus/store_decoders/store.json")];

    #[test]
    fn the_seed_corpus_is_accepted_by_its_targets() {
        assert!(PackageManifest::parse(MANIFEST_SEEDS[0]).is_ok());
        assert!(crate::Manifest::parse(std::str::from_utf8(MANIFEST_SEEDS[1]).unwrap()).is_ok());
        assert!(crate::anime::store::wire::decode_store_pb(STORE_SEEDS[0]).is_ok());
    }

    #[test]
    fn a_short_mutation_run_never_panics() {
        assert!(run(manifest, &MANIFEST_SEEDS, Duration::from_millis(300)) > 0);
        assert!(run(store_decoders, &STORE_SEEDS, Duration::from_millis(300)) > 0);
    }

    /// TST-19-202 and TST-19-203 without `cargo fuzz`: 60 s each.
    #[test]
    #[ignore = "60 s per target; run on demand"]
    fn fuzz_60s_manifest_and_store_decoders() {
        let manifest_rounds = run(manifest, &MANIFEST_SEEDS, Duration::from_secs(60));
        let store_rounds = run(store_decoders, &STORE_SEEDS, Duration::from_secs(60));
        eprintln!("fuzz_60s: manifest {manifest_rounds} inputs, store decoders {store_rounds} inputs, no panic");
    }
}
