//! The trust rule (T-19a-009, `p1-rust-design.md` section 4.3). A package is trusted when a store signs it (repo rule),
//! or when its exact version is in the stored set (per-version rule). The set lives in app state under
//! `__APP_STATE_trusted_extensions`, and its entries keep the Aniyomi form `<pkg>:<versionCode>:<digest>`.

use std::collections::BTreeSet;

use kuro_prefs::pref::Pref;

use crate::anime::model::{SignerDigest, StoreRow};

/// The `signingKey` of a store that has none. It never matches a real signer.
pub const NO_SIGNING_KEY: &str = "NO_SIGNING_KEY";

/// Reads the trusted set from its app-state preference.
pub struct TrustStore {
    entries: Pref<BTreeSet<String>>,
}

impl TrustStore {
    pub fn new(entries: Pref<BTreeSet<String>>) -> Self {
        Self { entries }
    }

    /// The stored entry for one version: `<pkg>:<versionCode>:<digest>`.
    pub fn version_key(pkg: &str, version_code: i64, signer: &SignerDigest) -> String {
        format!("{pkg}:{version_code}:{}", signer.0)
    }

    /// True by the repo rule or by the per-version rule. `signers` is the verifier's signer, one entry for a package.
    pub fn is_trusted(&self, pkg: &str, version_code: i64, signers: &[SignerDigest], stores: &[StoreRow]) -> bool {
        let repo_trusted = stores.iter().any(|store| {
            store.signing_key != NO_SIGNING_KEY && signers.iter().any(|signer| signer.0 == store.signing_key)
        });
        if repo_trusted {
            return true;
        }
        let entries = self.entries.get();
        signers.iter().any(|signer| entries.contains(&Self::version_key(pkg, version_code, signer)))
    }
}

#[cfg(test)]
mod tests {
    use kuro_prefs::store::PrefStore;

    use super::*;
    use crate::anime::testkit::{fixture_key, other_key, signer_hex};

    fn store(signing_key: &str) -> StoreRow {
        StoreRow {
            index_url: "https://example.org/index.json".into(),
            name: "Example".into(),
            badge_label: "EX".into(),
            signing_key: signing_key.into(),
            contact_website: "https://example.org".into(),
            contact_discord: None,
            is_legacy: false,
            extension_list_url: None,
        }
    }

    fn trust_with(entries: &[String]) -> TrustStore {
        let pref = PrefStore::in_memory().pref("__APP_STATE_trusted_extensions", BTreeSet::new());
        pref.extend(entries.iter().cloned());
        TrustStore::new(pref)
    }

    fn signer() -> SignerDigest {
        SignerDigest(signer_hex(&fixture_key()))
    }

    #[test]
    fn a_store_that_signs_the_package_trusts_every_version() {
        let trust = trust_with(&[]);
        let stores = [store(&signer().0)];
        assert!(trust.is_trusted("p.demo", 1, &[signer()], &stores));
        assert!(trust.is_trusted("p.demo", 99, &[signer()], &stores));
    }

    #[test]
    fn a_store_with_a_different_key_does_not_trust_the_package() {
        let trust = trust_with(&[]);
        let stores = [store(&signer_hex(&other_key()))];
        assert!(!trust.is_trusted("p.demo", 1, &[signer()], &stores));
    }

    #[test]
    fn no_signing_key_never_matches() {
        let trust = trust_with(&[]);
        let stores = [store(NO_SIGNING_KEY)];
        let artificial = SignerDigest(NO_SIGNING_KEY.into());
        assert!(!trust.is_trusted("p.demo", 1, &[artificial], &stores));
    }

    #[test]
    fn a_stored_version_entry_trusts_only_that_version() {
        let key = TrustStore::version_key("p.demo", 7, &signer());
        assert_eq!(key, format!("p.demo:7:{}", signer().0));
        let trust = trust_with(&[key]);
        assert!(trust.is_trusted("p.demo", 7, &[signer()], &[]));
        assert!(!trust.is_trusted("p.demo", 8, &[signer()], &[]));
    }

    #[test]
    fn removing_the_store_drops_the_repo_trust() {
        let trust = trust_with(&[]);
        let stores = [store(&signer().0)];
        assert!(trust.is_trusted("p.demo", 1, &[signer()], &stores));
        assert!(!trust.is_trusted("p.demo", 1, &[signer()], &[]), "no store signs it any more");
    }

    #[test]
    fn an_aniyomi_style_entry_with_another_digest_never_matches() {
        let trust = trust_with(&["p.demo:7:0000000000000000000000000000000000000000000000000000000000000000".into()]);
        assert!(!trust.is_trusted("p.demo", 7, &[signer()], &[]));
    }
}
