//! The trust rule (T-18a-044, edge cases 12.2): the per-version entries and the repo rule of the anime
//! extension trust store, over an in-memory preference.

use std::collections::BTreeSet;

use kuro_extensions::anime::model::{SignerDigest, StoreRow};
use kuro_extensions::anime::trust::TrustStore;
use kuro_prefs::store::PrefStore;

fn trust_with(entries: &[&str]) -> TrustStore {
    let pref = PrefStore::in_memory().pref("__APP_STATE_trusted_extensions", BTreeSet::new());
    pref.extend(entries.iter().map(|entry| (*entry).to_owned()))
        .expect("the in-memory store accepts the entries");
    TrustStore::new(pref)
}

fn signer(digest: &str) -> SignerDigest {
    SignerDigest(digest.to_owned())
}

fn store(signing_key: &str) -> StoreRow {
    StoreRow {
        index_url: "https://store.example/index.min.json".into(),
        name: "Store".into(),
        badge_label: "Store".into(),
        signing_key: signing_key.into(),
        contact_website: String::new(),
        contact_discord: None,
        is_legacy: false,
        extension_list_url: None,
    }
}

#[test]
fn the_trust_key_is_package_version_and_digest() {
    assert_eq!(
        TrustStore::version_key("a.b", 13, &signer("ab")),
        "a.b:13:ab"
    );
}

#[test]
fn trusting_a_package_keeps_entries_of_a_longer_package_name() {
    let trust = trust_with(&["a.b.c:1:ab"]);
    trust.trust_version("a.b", 2, &signer("ab")).unwrap();
    assert!(trust.is_trusted("a.b.c", 1, &[signer("ab")], &[]));
    assert!(trust.is_trusted("a.b", 2, &[signer("ab")], &[]));
}

#[test]
fn trusting_a_new_version_replaces_the_older_one() {
    let trust = trust_with(&[]);
    trust.trust_version("a.b", 1, &signer("ab")).unwrap();
    trust.trust_version("a.b", 2, &signer("ab")).unwrap();
    assert!(!trust.is_trusted("a.b", 1, &[signer("ab")], &[]));
    assert!(trust.is_trusted("a.b", 2, &[signer("ab")], &[]));
}

#[test]
fn a_store_that_signs_the_package_trusts_every_version() {
    let trust = trust_with(&[]);
    let stores = [store("ab")];
    assert!(trust.is_trusted("a.b", 1, &[signer("ab")], &stores));
    assert!(trust.is_trusted("a.b", 99, &[signer("ab")], &stores));
    assert!(!trust.is_trusted("a.b", 1, &[signer("cd")], &stores));
}

#[test]
fn a_store_without_a_signing_key_trusts_nothing() {
    let trust = trust_with(&[]);
    let stores = [store("NO_SIGNING_KEY")];
    assert!(!trust.is_trusted("a.b", 1, &[signer("NO_SIGNING_KEY")], &stores));
}

#[test]
fn revoking_forgets_the_version_entries_but_not_the_store_rule() {
    let trust = trust_with(&[]);
    trust.trust_version("a.b", 1, &signer("ab")).unwrap();
    trust.revoke_all().unwrap();
    assert!(!trust.is_trusted("a.b", 1, &[signer("ab")], &[]));
    assert!(trust.is_trusted("a.b", 1, &[signer("ab")], &[store("ab")]));
}
