//! Source id: the MD5 of `"{name lowercased}/{lang}/{version_id}"`, first eight bytes big-endian, sign bit cleared
//! (`HttpSource.kt`, T-15a-012). The same id is used for every row a source owns, so it must not change.

use md5::{Digest, Md5};

pub fn generate_id(name: &str, lang: &str, version_id: i32) -> i64 {
    let key = format!("{}/{}/{}", name.to_lowercase(), lang, version_id);
    let digest = Md5::digest(key.as_bytes());
    let id = digest.iter().take(8).fold(0_i64, |acc, &byte| (acc << 8) | i64::from(byte));
    id & i64::MAX
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_test_vectors_match_the_kotlin_function() {
        assert_eq!(generate_id("Example", "en", 1), 4_851_548_778_462_321_757);
        assert_eq!(generate_id("MangaDex", "en", 1), 2_499_283_573_021_220_255);
        assert_eq!(generate_id("Ünïcode Src", "ja", 2), 8_842_957_433_569_127_029);
        assert_eq!(generate_id("Aniyomi Test", "ja", 2), 7_915_262_807_009_061_958);
    }

    #[test]
    fn the_vectors_file_matches_the_function() {
        #[derive(serde::Deserialize)]
        struct Vector {
            name: String,
            lang: String,
            version_id: i32,
            id: i64,
        }
        #[derive(serde::Deserialize)]
        struct Vectors {
            vectors: Vec<Vector>,
        }
        let file: Vectors = serde_json::from_str(include_str!("../tests/fixtures/id/vectors.json")).unwrap();
        assert_eq!(file.vectors.len(), 4);
        for vector in file.vectors {
            assert_eq!(generate_id(&vector.name, &vector.lang, vector.version_id), vector.id, "{}", vector.name);
        }
    }

    #[test]
    fn the_id_is_never_negative() {
        for i in 0..1000 {
            let id = generate_id(&format!("source {i}"), "en", i);
            assert!(id >= 0, "{id}");
        }
    }

    #[test]
    fn the_name_is_lowercased_before_hashing_but_the_language_is_not() {
        assert_eq!(generate_id("EXAMPLE", "en", 1), generate_id("example", "en", 1));
        assert_ne!(generate_id("Example", "EN", 1), generate_id("Example", "en", 1));
    }

    #[test]
    fn a_changed_version_id_gives_a_new_id() {
        assert_ne!(generate_id("Example", "en", 1), generate_id("Example", "en", 2));
    }
}
