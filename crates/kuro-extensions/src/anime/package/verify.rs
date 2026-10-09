//! Verification of a downloaded `.kuroext` (T-19a-011, `p1-rust-design.md` section 4.2). The steps run in order and each
//! failure has its own variant: digest, key shape, signer, then signature. The signature covers the raw 32-byte digest.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use ed25519_dalek::{Signature, VerifyingKey};
use kuro_core::hash::sha256_hex;
use sha2::{Digest, Sha256};

use crate::anime::model::{KuroextRef, SignerDigest};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VerifyError {
    #[error("sha256 mismatch")]
    DigestMismatch,
    #[error("signature does not verify")]
    BadSignature,
    #[error("signer {0} is not the store signer")]
    UntrustedSigner(String),
    #[error("public key missing or malformed")]
    BadKey,
    #[error("could not read the download: {0}")]
    Io(String),
}

/// The package is authentic. `signer` is what the installed sidecar records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verified {
    pub signer: SignerDigest,
}

/// Streams `path` through SHA-256.
pub fn file_sha256(path: &Path) -> std::io::Result<[u8; 32]> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().into())
}

/// Checks the download at `path` against the index entry `expect`, for a store whose key is `store_signing_key`.
pub fn verify_download(path: &Path, expect: &KuroextRef, store_signing_key: &str) -> Result<Verified, VerifyError> {
    let actual = file_sha256(path).map_err(|err| VerifyError::Io(err.to_string()))?;
    if actual != expect.sha256 {
        return Err(VerifyError::DigestMismatch);
    }
    let key_bytes: [u8; 32] = expect.public_key.as_slice().try_into().map_err(|_| VerifyError::BadKey)?;
    let key = VerifyingKey::from_bytes(&key_bytes).map_err(|_| VerifyError::BadKey)?;
    if key.is_weak() {
        return Err(VerifyError::BadKey);
    }
    let signer = sha256_hex(&key_bytes);
    if signer != store_signing_key {
        return Err(VerifyError::UntrustedSigner(signer));
    }
    let sig_bytes: [u8; 64] = expect.ed25519.as_slice().try_into().map_err(|_| VerifyError::BadSignature)?;
    let signature = Signature::from_bytes(&sig_bytes);
    key.verify_strict(&expect.sha256, &signature).map_err(|_| VerifyError::BadSignature)?;
    Ok(Verified { signer: SignerDigest(signer) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anime::testkit::MANIFEST;
    use crate::anime::testkit::{Fixture, fixture_key, other_key, signed_package, signer_hex};

    fn write(dir: &Path, bytes: &[u8]) -> std::path::PathBuf {
        let path = dir.join("download.kuroext.part");
        std::fs::write(&path, bytes).unwrap();
        path
    }

    fn decode(text: &str) -> Vec<u8> {
        use base64::Engine;
        base64::prelude::BASE64_STANDARD.decode(text).unwrap()
    }

    fn reference(fixture: &Fixture) -> KuroextRef {
        KuroextRef {
            url: "https://example.org/demo.kuroext".into(),
            sha256: fixture.sha256,
            ed25519: decode(&fixture.ed25519_b64),
            public_key: decode(&fixture.public_key_b64),
        }
    }

    #[test]
    fn a_correctly_signed_download_verifies_and_reports_its_signer() {
        let dir = tempfile::tempdir().unwrap();
        let fixture = signed_package(&fixture_key(), MANIFEST, b"\0asm");
        let path = write(dir.path(), &fixture.bytes);
        let verified = verify_download(&path, &reference(&fixture), &signer_hex(&fixture_key())).expect("verifies");
        assert_eq!(verified.signer.0, fixture.signer_hex);
    }

    #[test]
    fn a_changed_byte_fails_the_digest_check_first() {
        let dir = tempfile::tempdir().unwrap();
        let fixture = signed_package(&fixture_key(), MANIFEST, b"\0asm");
        let mut bytes = fixture.bytes.clone();
        bytes[10] ^= 0xff;
        let path = write(dir.path(), &bytes);
        let result = verify_download(&path, &reference(&fixture), &signer_hex(&fixture_key()));
        assert_eq!(result, Err(VerifyError::DigestMismatch));
    }

    #[test]
    fn a_wrong_signature_is_bad_signature() {
        let dir = tempfile::tempdir().unwrap();
        let fixture = signed_package(&fixture_key(), MANIFEST, b"\0asm");
        let path = write(dir.path(), &fixture.bytes);
        let mut expect = reference(&fixture);
        expect.ed25519[0] ^= 0x01;
        let result = verify_download(&path, &expect, &signer_hex(&fixture_key()));
        assert_eq!(result, Err(VerifyError::BadSignature));
    }

    #[test]
    fn a_package_signed_by_another_key_is_an_untrusted_signer() {
        let dir = tempfile::tempdir().unwrap();
        let fixture = signed_package(&other_key(), MANIFEST, b"\0asm");
        let path = write(dir.path(), &fixture.bytes);
        let result = verify_download(&path, &reference(&fixture), &signer_hex(&fixture_key()));
        assert_eq!(result, Err(VerifyError::UntrustedSigner(fixture.signer_hex.clone())));
    }

    #[test]
    fn a_public_key_that_is_not_32_bytes_is_a_bad_key() {
        let dir = tempfile::tempdir().unwrap();
        let fixture = signed_package(&fixture_key(), MANIFEST, b"\0asm");
        let path = write(dir.path(), &fixture.bytes);
        let mut expect = reference(&fixture);
        expect.public_key.truncate(31);
        assert_eq!(verify_download(&path, &expect, &signer_hex(&fixture_key())), Err(VerifyError::BadKey));
    }

    #[test]
    fn the_all_zero_small_order_key_is_a_bad_key() {
        let dir = tempfile::tempdir().unwrap();
        let fixture = signed_package(&fixture_key(), MANIFEST, b"\0asm");
        let path = write(dir.path(), &fixture.bytes);
        let mut expect = reference(&fixture);
        expect.public_key = vec![0u8; 32];
        let zero_signer = sha256_hex(&[0u8; 32]);
        assert_eq!(verify_download(&path, &expect, &zero_signer), Err(VerifyError::BadKey));
    }
}
