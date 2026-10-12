//! Old repo addresses of a backup (area 22, T-22a-026 and T-22a-027). The custom buttons are not restored: the Lua
//! scripts that ran them are cut.

use std::collections::BTreeSet;

use crate::schema::{Backup, PrefValue};

/// The preference keys that hold old repo address lists.
const LEGACY_REPO_KEYS: [&str; 2] = ["anime_extension_repos", "extension_repos"];

/// Every old repo address of a backup: the two repo lists of its preferences, the manga repo rows and the anime store
/// rows. After a restore with the extension stores option on, the app hands them to
/// `kuro_domain::extension_repo::import_legacy_repos`, which keeps the ones that are repository links (T-19a-022, T-18a-019).
pub fn legacy_repo_urls(backup: &Backup) -> BTreeSet<String> {
    let from_prefs = backup
        .preferences
        .iter()
        .filter(|p| LEGACY_REPO_KEYS.contains(&p.key.as_str()))
        .filter_map(|p| match &p.value {
            PrefValue::StringSet(set) => Some(set.iter().cloned().collect::<Vec<_>>()),
            _ => None,
        })
        .flatten();
    let rows = backup
        .manga_extension_repos
        .iter()
        .map(|r| r.base_url.clone())
        .chain(backup.anime_extension_stores.iter().map(|s| s.index_url.clone()));
    from_prefs.chain(rows).filter(|url| !url.is_empty()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_urls_come_from_the_repo_prefs_and_the_old_rows_and_other_keys_are_ignored() {
        use crate::schema::BackupPreference;
        let backup = Backup {
            preferences: vec![
                BackupPreference { key: "extension_repos".into(), value: PrefValue::StringSet(["https://m.test".to_owned()].into()) },
                BackupPreference { key: "theme".into(), value: PrefValue::String("dark".into()) },
            ],
            manga_extension_repos: vec![crate::schema::BackupExtensionRepos { base_url: "https://row.test/index.min.json".into(), ..Default::default() }],
            ..Default::default()
        };
        let urls: Vec<String> = legacy_repo_urls(&backup).into_iter().collect();
        assert_eq!(urls, ["https://m.test", "https://row.test/index.min.json"]);
    }
}
