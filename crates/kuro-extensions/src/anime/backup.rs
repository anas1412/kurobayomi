//! The anime store entries of a backup (T-19a-054, `p1-data-and-config.md` section 9). The backup file format is area 21
//! and the restore flow is area 22. This module holds the entry message with its protobuf tags, and the rules that turn
//! an entry into a stored row: the defaults, the upsert, the option gate, and the error text of a failed row.

use crate::anime::model::StoreRow;

/// One store in a backup. The tags are the ones Aniyomi writes (`BackupExtensionStore`, E-BCK-01).
#[derive(Clone, PartialEq, prost::Message)]
pub struct BackupStore {
    #[prost(string, tag = "1")]
    pub index_url: String,
    #[prost(string, tag = "2")]
    pub name: String,
    #[prost(string, optional, tag = "3")]
    pub badge_label: Option<String>,
    #[prost(string, tag = "4")]
    pub contact_website: String,
    #[prost(string, tag = "5")]
    pub signing_key: String,
    #[prost(string, optional, tag = "6")]
    pub contact_discord: Option<String>,
    #[prost(bool, optional, tag = "7")]
    pub is_legacy: Option<bool>,
    #[prost(string, optional, tag = "8")]
    pub extension_list_url: Option<String>,
}

/// The stores a backup holds. With the `extensionStores` option off the list is empty (E-BCK-05).
pub fn backup_entries(rows: &[StoreRow], extension_stores: bool) -> Vec<BackupStore> {
    if !extension_stores {
        return Vec::new();
    }
    rows.iter()
        .map(|row| BackupStore {
            index_url: row.index_url.clone(),
            name: row.name.clone(),
            badge_label: Some(row.badge_label.clone()),
            contact_website: row.contact_website.clone(),
            signing_key: row.signing_key.clone(),
            contact_discord: row.contact_discord.clone(),
            is_legacy: Some(row.is_legacy),
            extension_list_url: row.extension_list_url.clone(),
        })
        .collect()
}

/// The row a backup entry restores to. A missing badge is the name, and a missing `isLegacy` is `true` (E-BCK-02,
/// E-STO-22).
pub fn restore_row(entry: &BackupStore) -> Result<StoreRow, String> {
    if entry.index_url.trim().is_empty() {
        return Err("the store has no index URL".into());
    }
    Ok(StoreRow {
        index_url: entry.index_url.clone(),
        name: entry.name.clone(),
        badge_label: entry
            .badge_label
            .clone()
            .unwrap_or_else(|| entry.name.clone()),
        signing_key: entry.signing_key.clone(),
        contact_website: entry.contact_website.clone(),
        contact_discord: entry.contact_discord.clone(),
        is_legacy: entry.is_legacy.unwrap_or(true),
        extension_list_url: entry.extension_list_url.clone(),
    })
}

/// Stores `row`, replacing the row with the same index URL (E-BCK-03).
pub fn upsert(rows: &mut Vec<StoreRow>, row: StoreRow) {
    match rows
        .iter_mut()
        .find(|stored| stored.index_url == row.index_url)
    {
        Some(stored) => *stored = row,
        None => rows.push(row),
    }
}

/// Restores every entry. A failed entry adds `Error Adding Anime Repo: <name> : <message>` to the returned list, and the
/// others still restore. With the `extensionStores` option off nothing is written (E-BCK-04, TST-19-114).
pub fn restore(
    rows: &mut Vec<StoreRow>,
    entries: &[BackupStore],
    extension_stores: bool,
) -> Vec<String> {
    if !extension_stores {
        return Vec::new();
    }
    let mut failures = Vec::new();
    for entry in entries {
        match restore_row(entry) {
            Ok(row) => upsert(rows, row),
            Err(message) => failures.push(format!(
                "Error Adding Anime Repo: {} : {message}",
                entry.name
            )),
        }
    }
    failures
}

#[cfg(test)]
mod tests {
    use prost::Message;

    use super::*;

    fn row(index_url: &str, name: &str) -> StoreRow {
        StoreRow {
            index_url: index_url.into(),
            name: name.into(),
            badge_label: "EX".into(),
            signing_key: "ab".repeat(32),
            contact_website: "https://example.org".into(),
            contact_discord: Some("https://discord.gg/x".into()),
            is_legacy: false,
            extension_list_url: Some("https://example.org/list.json".into()),
        }
    }

    fn full() -> BackupStore {
        BackupStore {
            index_url: "https://example.org/index.json".into(),
            name: "Example".into(),
            badge_label: Some("EX".into()),
            contact_website: "https://example.org".into(),
            signing_key: "ab".repeat(32),
            contact_discord: Some("https://discord.gg/x".into()),
            is_legacy: Some(false),
            extension_list_url: Some("https://example.org/list.json".into()),
        }
    }

    #[test]
    fn tst_19_110_the_entry_uses_the_kotlin_tags_one_to_eight() {
        let entry = full();
        let encoded = entry.encode_to_vec();
        let decoded = BackupStore::decode(encoded.as_slice()).expect("the entry decodes");
        assert_eq!(decoded, entry, "the round trip keeps every field");

        // Each field alone encodes with its own key byte: tag << 3 | wire type (2 for strings, 0 for the bool).
        let alone = [
            (
                1,
                BackupStore {
                    index_url: "x".into(),
                    ..Default::default()
                },
            ),
            (
                2,
                BackupStore {
                    name: "x".into(),
                    ..Default::default()
                },
            ),
            (
                3,
                BackupStore {
                    badge_label: Some("x".into()),
                    ..Default::default()
                },
            ),
            (
                4,
                BackupStore {
                    contact_website: "x".into(),
                    ..Default::default()
                },
            ),
            (
                5,
                BackupStore {
                    signing_key: "x".into(),
                    ..Default::default()
                },
            ),
            (
                6,
                BackupStore {
                    contact_discord: Some("x".into()),
                    ..Default::default()
                },
            ),
            (
                7,
                BackupStore {
                    is_legacy: Some(true),
                    ..Default::default()
                },
            ),
            (
                8,
                BackupStore {
                    extension_list_url: Some("x".into()),
                    ..Default::default()
                },
            ),
        ];
        for (tag, only) in alone {
            let bytes = only.encode_to_vec();
            let wire_type = if tag == 7 { 0 } else { 2 };
            assert_eq!(bytes[0], (tag << 3) | wire_type, "tag {tag}");
        }
    }

    #[test]
    fn tst_19_111_a_missing_badge_restores_as_the_name() {
        let entry = BackupStore {
            badge_label: None,
            ..full()
        };
        assert_eq!(restore_row(&entry).expect("a row").badge_label, "Example");
    }

    #[test]
    fn tst_19_050_a_missing_is_legacy_restores_as_legacy() {
        let entry = BackupStore {
            is_legacy: None,
            ..full()
        };
        assert!(restore_row(&entry).expect("a row").is_legacy);
        let entry = BackupStore {
            is_legacy: Some(false),
            ..full()
        };
        assert!(!restore_row(&entry).expect("a row").is_legacy);
    }

    #[test]
    fn tst_19_112_a_duplicate_index_url_is_upserted_to_one_row() {
        let mut rows = vec![row("https://example.org/index.json", "Old")];
        restore(&mut rows, &[full(), full()], true);
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].name, "Example",
            "the restored row replaces the stored one"
        );
    }

    #[test]
    fn tst_19_113_a_failed_row_reports_its_error_and_the_others_still_restore() {
        let bad = BackupStore {
            index_url: "  ".into(),
            name: "Broken".into(),
            ..full()
        };
        let good = BackupStore {
            index_url: "https://ok.example/index.json".into(),
            name: "Good".into(),
            ..full()
        };
        let mut rows = Vec::new();
        let failures = restore(&mut rows, &[bad, good], true);
        assert_eq!(
            failures,
            ["Error Adding Anime Repo: Broken : the store has no index URL"]
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "Good");
    }

    #[test]
    fn tst_19_114_the_extension_stores_option_off_writes_nothing() {
        let mut rows = vec![row("https://keep.example/index.json", "Keep")];
        let failures = restore(&mut rows, &[full()], false);
        assert!(failures.is_empty());
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "Keep");
        assert!(backup_entries(&rows, false).is_empty());
        assert_eq!(backup_entries(&rows, true).len(), 1);
    }

    #[test]
    fn a_backup_entry_round_trips_through_its_row() {
        let stored = row("https://example.org/index.json", "Example");
        let entries = backup_entries(std::slice::from_ref(&stored), true);
        let restored = restore_row(&entries[0]).expect("a row");
        assert_eq!(restored, stored);
    }
}
