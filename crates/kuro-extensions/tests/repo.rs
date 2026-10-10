//! Edge cases 12.7 (T-18a-044): the store table. The manga plan's `extension_repos` (base URL plus signing-key
//! fingerprint, X16 to X22) became the `extension_store` table of the anime database, keyed by index URL and read in
//! insert order. It lives in `kuro-db`, so these tests use that crate's in-memory database.
//!
//! | 12.7 case | here |
//! |---|---|
//! | `upsert_updates_existing_base_url` | `upsert_updates_existing_index_url` |
//! | `unique_fingerprint_conflict_maps_to_repo_save` (X19) | not applicable: `extension_store` has no UNIQUE signing key, two stores may share one (the signing key is shared by design); `two_stores_may_share_a_signing_key` pins that |
//! | `findall_orders_by_rowid` (X22) | `all_stores_come_back_in_insert_order` |
//! | `should_apply_refresh_table` (X16, X17) | not applicable: a refresh does not rewrite the fingerprint, the row is written only by Add store (the `NOFINGERPRINT-n` placeholder does not exist; a preference entry gets `NO_SIGNING_KEY`, `a_preference_entry_has_no_signing_key`) |

use kuro_db::schema_anime::open_in_memory;
use kuro_db::statements_anime::{
    ExtensionStore, delete_extension_store, extension_store, extension_store_count, extension_stores,
    insert_extension_store_from_preference, upsert_extension_store,
};

fn store(url: &str, name: &str, key: &str) -> ExtensionStore {
    ExtensionStore {
        index_url: url.into(),
        name: name.into(),
        badge_label: name.into(),
        signing_key: key.into(),
        contact_website: "https://example.org".into(),
        contact_discord: None,
        is_legacy: false,
        extension_list_url: None,
        resolved_index_url: url.into(),
    }
}

#[test]
fn upsert_updates_existing_index_url() {
    let db = open_in_memory().unwrap();
    upsert_extension_store(&db, &store("https://a.test/index.json", "Old", "key-1")).unwrap();
    upsert_extension_store(&db, &store("https://a.test/index.json", "New", "key-2")).unwrap();
    assert_eq!(extension_store_count(&db).unwrap(), 1, "the same index URL is one row");
    let row = extension_store(&db, "https://a.test/index.json").unwrap().unwrap();
    assert_eq!((row.name.as_str(), row.signing_key.as_str()), ("New", "key-2"));
}

#[test]
fn two_stores_may_share_a_signing_key() {
    let db = open_in_memory().unwrap();
    upsert_extension_store(&db, &store("https://a.test/index.json", "A", "same")).unwrap();
    upsert_extension_store(&db, &store("https://b.test/index.json", "B", "same")).unwrap();
    assert_eq!(extension_store_count(&db).unwrap(), 2);
}

#[test]
fn all_stores_come_back_in_insert_order() {
    let db = open_in_memory().unwrap();
    for (url, name) in [("https://z.test/i.json", "Zed"), ("https://a.test/i.json", "Ay"), ("https://m.test/i.json", "Em")] {
        upsert_extension_store(&db, &store(url, name, "k")).unwrap();
    }
    // updating the first row does not move it
    upsert_extension_store(&db, &store("https://z.test/i.json", "Zed again", "k")).unwrap();
    let names: Vec<_> = extension_stores(&db).unwrap().into_iter().map(|s| s.name).collect();
    assert_eq!(names, ["Zed again", "Ay", "Em"]);
    assert_eq!(delete_extension_store(&db, "https://a.test/i.json").unwrap(), 1);
    assert_eq!(extension_store_count(&db).unwrap(), 2);
}

#[test]
fn a_preference_entry_has_no_signing_key() {
    let db = open_in_memory().unwrap();
    insert_extension_store_from_preference(&db, "https://a.test/index.json", "Mine").unwrap();
    let row = extension_store(&db, "https://a.test/index.json").unwrap().unwrap();
    assert_eq!(row.signing_key, "NO_SIGNING_KEY");
    assert_eq!(row.badge_label, "Mine");
}
