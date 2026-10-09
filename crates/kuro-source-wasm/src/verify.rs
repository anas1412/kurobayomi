//! Digest and signature check of a downloaded package (design 6.3, T-18a-047).

use ed25519_dalek::{Signature, VerifyingKey};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum VerifyError {
    #[error("digest mismatch")]
    DigestMismatch,
    #[error("the signing key does not match the repo fingerprint")]
    FingerprintMismatch,
    #[error("the signing key is not a valid ed25519 key")]
    BadKey,
    #[error("the signature is not 64 bytes")]
    BadSignatureLength,
    #[error("signature invalid")]
    BadSignature,
}

/// The lowercase hex SHA-256 of a public key, the form a repo stores as `signingKeyFingerprint` (design 6.1).
pub fn fingerprint(public_key: &[u8; 32]) -> String {
    hex(&Sha256::digest(public_key))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Proof that [`verify_package`] accepted a package, and the key that signed it. Only that function builds one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Verified {
    public_key: [u8; 32],
}

impl Verified {
    pub fn public_key(&self) -> &[u8; 32] {
        &self.public_key
    }
}

/// Checks, in order: the SHA-256 of `bytes` equals `index_sha256`; the fingerprint of `public_key` equals
/// `repo_fingerprint` (lowercase hex only); `verify_strict` accepts `signature` over the raw digest.
pub fn verify_package(
    bytes: &[u8],
    index_sha256: &[u8; 32],
    public_key: &[u8; 32],
    repo_fingerprint: &str,
    signature: &[u8],
) -> Result<Verified, VerifyError> {
    let digest: [u8; 32] = Sha256::digest(bytes).into();
    if &digest != index_sha256 {
        return Err(VerifyError::DigestMismatch);
    }
    if fingerprint(public_key) != repo_fingerprint {
        return Err(VerifyError::FingerprintMismatch);
    }
    let key = VerifyingKey::from_bytes(public_key).map_err(|_| VerifyError::BadKey)?;
    let signature = Signature::from_slice(signature).map_err(|_| VerifyError::BadSignatureLength)?;
    key.verify_strict(&digest, &signature).map_err(|_| VerifyError::BadSignature)?;
    Ok(Verified { public_key: *public_key })
}

#[cfg(test)]
pub(crate) mod fixtures {
    use ed25519_dalek::{Signer, SigningKey};
    use sha2::{Digest, Sha256};

    /// What a repo index lists for one package.
    pub struct Signed {
        pub sha256: [u8; 32],
        pub public_key: [u8; 32],
        pub fingerprint: String,
        pub signature: Vec<u8>,
    }

    pub fn key(seed: u8) -> SigningKey {
        SigningKey::from_bytes(&[seed; 32])
    }

    pub fn sign(bytes: &[u8], signing: &SigningKey) -> Signed {
        let sha256: [u8; 32] = Sha256::digest(bytes).into();
        let public_key = signing.verifying_key().to_bytes();
        Signed {
            sha256,
            public_key,
            fingerprint: super::fingerprint(&public_key),
            signature: signing.sign(&sha256).to_bytes().to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::*;
    use super::*;

    const BYTES: &[u8] = b"package bytes";

    fn check(bytes: &[u8], s: &Signed) -> Result<Verified, VerifyError> {
        verify_package(bytes, &s.sha256, &s.public_key, &s.fingerprint, &s.signature)
    }

    #[test]
    fn a_valid_signature_is_accepted_and_names_the_signer() {
        let signed = sign(BYTES, &key(1));
        assert_eq!(check(BYTES, &signed).unwrap().public_key(), &signed.public_key);
    }

    #[test]
    fn a_digest_mismatch_is_reported_first() {
        let signed = sign(BYTES, &key(1));
        assert_eq!(check(b"other bytes", &signed), Err(VerifyError::DigestMismatch));
        // Even with a wrong fingerprint and a bad signature, the digest is the first check.
        let mut worse = sign(BYTES, &key(1));
        worse.fingerprint = "0".repeat(64);
        worse.signature = vec![0; 3];
        assert_eq!(check(b"other bytes", &worse), Err(VerifyError::DigestMismatch));
    }

    #[test]
    fn a_key_that_is_not_the_repo_key_is_refused_before_the_signature_is_read() {
        let mut signed = sign(BYTES, &key(1));
        let other = sign(BYTES, &key(2));
        signed.public_key = other.public_key;
        signed.signature = vec![0; 3];
        assert_eq!(check(BYTES, &signed), Err(VerifyError::FingerprintMismatch));
        // An uppercase fingerprint is not the repo's form.
        let mut upper = sign(BYTES, &key(1));
        upper.fingerprint = upper.fingerprint.to_uppercase();
        assert_eq!(check(BYTES, &upper), Err(VerifyError::FingerprintMismatch));
    }

    #[test]
    fn a_signature_by_another_key_with_the_right_fingerprint_fails() {
        let mut signed = sign(BYTES, &key(1));
        signed.signature = sign(BYTES, &key(2)).signature;
        assert_eq!(check(BYTES, &signed), Err(VerifyError::BadSignature));
        signed.signature.truncate(63);
        assert_eq!(check(BYTES, &signed), Err(VerifyError::BadSignatureLength));
    }

    #[test]
    fn a_malleable_signature_is_rejected() {
        // S + L with the same R verifies the same equation modulo L, but is not canonical.
        const L: [u8; 32] = [
            0xed, 0xd3, 0xf5, 0x5c, 0x1a, 0x63, 0x12, 0x58, 0xd6, 0x9c, 0xf7, 0xa2, 0xde, 0xf9, 0xde, 0x14, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x10,
        ];
        let mut signed = sign(BYTES, &key(1));
        assert!(check(BYTES, &signed).is_ok());
        let mut carry = 0u16;
        for (byte, l) in signed.signature[32..].iter_mut().zip(L) {
            let sum = *byte as u16 + l as u16 + carry;
            *byte = sum as u8;
            carry = sum >> 8;
        }
        assert_eq!(carry, 0);
        assert_eq!(check(BYTES, &signed), Err(VerifyError::BadSignature));
    }
}
