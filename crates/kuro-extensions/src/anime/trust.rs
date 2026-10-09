//! The trust rule (T-19a-009, `p1-rust-design.md` section 4.3). A package is trusted when a store signs it (repo rule),
//! or when its exact version is in the stored set (per-version rule). The set lives in app state under
//! `__APP_STATE_trusted_extensions`, and its entries keep the Aniyomi form `<pkg>:<versionCode>:<digest>`.

use std::collections::BTreeSet;

use kuro_prefs::PrefError;
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

    /// Trusts one version of a package by writing its `<pkg>:<versionCode>:<digest>` entry (T-19a-051). The older
    /// entries of the same package are replaced (E-TRU-01, design 4.3), so a superseded version is no longer trusted.
    /// Entries of other packages and the store trust are not touched.
    pub fn trust_version(&self, pkg: &str, version_code: i64, signer: &SignerDigest) -> Result<(), PrefError> {
        let own = format!("{pkg}:");
        let mut entries = self.entries.get();
        entries.retain(|entry| !entry.starts_with(&own));
        entries.insert(Self::version_key(pkg, version_code, signer));
        self.entries.set(entries)
    }

    /// Forgets every per-version entry (T-19a-010, E-TRU-03). The store rule is not stored here, so it still applies.
    pub fn revoke_all(&self) -> Result<(), PrefError> {
        self.entries.set(BTreeSet::new())
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
    fn trusting_a_version_trusts_only_that_version_and_keeps_other_entries() {
        let trust = trust_with(&["other:1:abc".to_owned()]);
        trust.trust_version("pkg.one", 3, &signer()).unwrap();
        assert!(trust.is_trusted("pkg.one", 3, &[signer()], &[]));
        assert!(!trust.is_trusted("pkg.one", 4, &[signer()], &[]));
        assert!(trust.entries.get().contains("other:1:abc"));
    }

    #[test]
    fn tst_19_092_trusting_n_then_n_plus_1_leaves_only_the_newer_entry() {
        let trust = trust_with_entries_for_replace();
        trust.trust_version("pkg.one", 3, &signer()).unwrap();
        trust.trust_version("pkg.one", 4, &signer()).unwrap();
        assert!(!trust.is_trusted("pkg.one", 3, &[signer()], &[]), "the superseded version is no longer trusted");
        assert!(trust.is_trusted("pkg.one", 4, &[signer()], &[]));
        assert!(trust.entries.get().contains("other:1:abc"), "another package's entry stays");
        assert_eq!(trust.entries.get().len(), 2);
    }

    #[test]
    fn replacing_entries_does_not_touch_a_package_whose_name_starts_the_same() {
        let trust = trust_with_entries_for_replace();
        trust.trust_version("pkg.one", 1, &signer()).unwrap();
        trust.trust_version("pkg.one.extra", 1, &signer()).unwrap();
        trust.trust_version("pkg.one", 2, &signer()).unwrap();
        assert!(trust.is_trusted("pkg.one.extra", 1, &[signer()], &[]));
    }

    fn trust_with_entries_for_replace() -> TrustStore {
        trust_with(&["other:1:abc".to_owned()])
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

    #[test]
    fn tst_19_094_revoke_all_removes_version_entries_and_the_store_rule_still_works() {
        let trust = trust_with(&[]);
        trust.trust_version("p.demo", 7, &signer()).expect("trusted");
        assert!(trust.is_trusted("p.demo", 7, &[signer()], &[]));
        trust.revoke_all().expect("revoked");
        assert!(!trust.is_trusted("p.demo", 7, &[signer()], &[]), "the version entry is gone");
        assert!(trust.is_trusted("p.demo", 7, &[signer()], &[store(&signer().0)]), "the repo rule still works");
    }
}
