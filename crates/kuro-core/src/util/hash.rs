//! Hash helpers (port of `Hash.kt` and `DiskUtil.hashKeyForDisk`). Output is lowercase hex.

use md5::{Digest, Md5};
use sha1::Sha1;
use sha2::Sha256;

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn md5_hex(data: &[u8]) -> String {
    to_hex(&Md5::digest(data))
}

pub fn sha1_hex(data: &[u8]) -> String {
    to_hex(&Sha1::digest(data))
}

pub fn sha256_hex(data: &[u8]) -> String {
    to_hex(&Sha256::digest(data))
}

/// Disk cache key for a string: the md5 of its UTF-8 bytes.
pub fn disk_key(key: &str) -> String {
    md5_hex(key.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn md5_known_vectors() {
        assert_eq!(md5_hex(b""), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(md5_hex(b"abc"), "900150983cd24fb0d6963f7d28e17f72");
    }

    #[test]
    fn disk_key_is_md5_of_the_string() {
        assert_eq!(disk_key("abc"), "900150983cd24fb0d6963f7d28e17f72");
    }

    #[test]
    fn sha1_known_vectors() {
        assert_eq!(sha1_hex(b""), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(sha1_hex(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
    }

    #[test]
    fn sha256_known_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn output_is_lowercase_hex_of_the_right_length() {
        for (hex, len) in [
            (md5_hex(b"x"), 32),
            (sha1_hex(b"x"), 40),
            (sha256_hex(b"x"), 64),
        ] {
            assert_eq!(hex.len(), len);
            assert!(
                hex.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            );
        }
    }
}
