//! Extension stores and manga repos (area 22, T-22a-026 and T-22a-027).

use std::collections::{BTreeSet, HashSet};

use kuro_db::models::ExtensionRepoRow;
use kuro_db::statements_anime::{
    CustomButtonUpdate, ExtensionStore, create_custom_button, custom_buttons, extension_store, update_custom_button,
    upsert_extension_store,
};
use kuro_db::statements_manga::extension_repos;
use kuro_db::DbResult;
use rusqlite::Connection;

use crate::schema::{BackupCustomButtons, BackupExtensionRepos, BackupExtensionStore, BackupPreference, PrefValue};

/// The old repo URL lists of a backup: `(anime_extension_repos, extension_repos)`, each empty when the backup has no
/// such value. After a restore with the extension stores option on, the app hands them to
/// `kuro_domain::extension_repo::import_legacy_repos` (T-19a-022, T-18a-019).
pub fn legacy_repo_urls(prefs: &[BackupPreference]) -> (BTreeSet<String>, BTreeSet<String>) {
    let set = |key: &str| {
        prefs
            .iter()
            .find(|p| p.key == key)
            .and_then(|p| match &p.value {
                PrefValue::StringSet(set) => Some(set.clone()),
                _ => None,
            })
            .unwrap_or_default()
    };
    (set("anime_extension_repos"), set("extension_repos"))
}

/// Adds the backup's custom buttons (T-22a-028, behaviour 4.6). A button whose name is already in the database, or
/// that repeats an earlier name in the backup, adds nothing (E-17). New rows follow the stored order, and the backup's
/// `sortIndex` order decides the order of the new ones. At most one button becomes favourite: the first favourite in
/// that order, and only when the database has none (D10 fix).
pub fn restore_custom_buttons(c: &Connection, buttons: &[BackupCustomButtons]) -> DbResult<()> {
    if buttons.is_empty() {
        return Ok(());
    }
    let stored = custom_buttons(c)?;
    let mut names: HashSet<String> = stored.iter().map(|b| b.name.clone()).collect();
    let mut has_favorite = stored.iter().any(|b| b.is_favorite);
    let mut ordered: Vec<&BackupCustomButtons> = buttons.iter().collect();
    ordered.sort_by_key(|b| b.sort_index);
    for b in ordered {
        if !names.insert(b.name.clone()) {
            continue;
        }
        let id = create_custom_button(c, &b.name, &b.content, &b.long_press_content, &b.on_startup)?;
        if b.is_favorite && !has_favorite {
            update_custom_button(c, id, &CustomButtonUpdate { is_favorite: Some(true), ..Default::default() })?;
            has_favorite = true;
        }
    }
    Ok(())
}

/// Upserts one anime extension store on `index_url` (T-22a-026). A duplicate is an update, never an error. The badge
/// label falls back to the name, and `isLegacy` defaults to true when the backup has no value.
pub fn restore_anime_store(c: &Connection, s: &BackupExtensionStore) -> rusqlite::Result<()> {
    let store = ExtensionStore {
        index_url: s.index_url.clone(),
        name: s.name.clone(),
        badge_label: s.badge_label.clone().unwrap_or_else(|| s.name.clone()),
        signing_key: s.signing_key.clone(),
        contact_website: s.contact_website.clone(),
        contact_discord: s.contact_discord.clone(),
        is_legacy: s.is_legacy.unwrap_or(true),
        extension_list_url: s.extension_list_url.clone(),
        // The backup does not say which document the data came from, so the key itself is used.
        resolved_index_url: s.index_url.clone(),
    };
    // A repeat restore of the same store writes nothing.
    if extension_store(c, &s.index_url)?.as_ref() == Some(&store) {
        return Ok(());
    }
    upsert_extension_store(c, &store)?;
    Ok(())
}

/// Adds one manga repo (T-22a-027). The same URL with the same fingerprint succeeds without a write (D11 fixed). The
/// same URL with another fingerprint, or another URL with an existing fingerprint, is an error. The error is the log
/// line `Error Adding Manga Repo: <name> : <message>`.
pub fn restore_manga_repo(c: &Connection, r: &BackupExtensionRepos) -> Result<(), String> {
    let line = |message: String| format!("Error Adding Manga Repo: {} : {}", r.name, message);
    let row = ExtensionRepoRow {
        base_url: r.base_url.clone(),
        name: r.name.clone(),
        short_name: r.short_name.clone(),
        website: r.website.clone(),
        signing_key_fingerprint: r.signing_key_fingerprint.clone(),
    };
    let same_url = extension_repos::find_one(c, &row.base_url).map_err(|e| line(e.to_string()))?;
    if let Some(existing) = same_url {
        return if existing.signing_key_fingerprint == row.signing_key_fingerprint {
            Ok(())
        } else {
            Err(line("Already Exists with different signing key fingerprint".into()))
        };
    }
    let same_key = extension_repos::find_one_by_signing_key_fingerprint(c, &row.signing_key_fingerprint)
        .map_err(|e| line(e.to_string()))?;
    if let Some(other) = same_key {
        return Err(line(format!("{} has the same signing key fingerprint", other.name)));
    }
    extension_repos::insert(c, &row).map_err(|e| line(e.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use kuro_db::schema_anime;
    use kuro_db::schema_manga;

    use super::*;

    fn store(index: &str, name: &str) -> BackupExtensionStore {
        BackupExtensionStore {
            index_url: index.into(),
            name: name.into(),
            badge_label: None,
            contact_website: "w".into(),
            signing_key: "k".into(),
            contact_discord: None,
            is_legacy: None,
            extension_list_url: None,
            ..Default::default()
        }
    }

    fn repo(url: &str, name: &str, fp: &str) -> BackupExtensionRepos {
        BackupExtensionRepos {
            base_url: url.into(),
            name: name.into(),
            short_name: None,
            website: "w".into(),
            signing_key_fingerprint: fp.into(),
        }
    }

    fn legacy_pref(urls: &[&str]) -> BackupPreference {
        BackupPreference {
            key: "anime_extension_repos".into(),
            value: PrefValue::StringSet(urls.iter().map(|u| (*u).to_owned()).collect()),
        }
    }

    #[test]
    fn the_old_repo_url_lists_are_read_and_other_keys_are_ignored() {
        let prefs = vec![
            legacy_pref(&["https://b.test/index.min.json", "https://a.test/index.json"]),
            BackupPreference { key: "extension_repos".into(), value: PrefValue::StringSet(["https://m.test".to_owned()].into()) },
            BackupPreference { key: "theme".into(), value: PrefValue::String("dark".into()) },
        ];
        let (anime, manga) = legacy_repo_urls(&prefs);
        assert_eq!(anime.into_iter().collect::<Vec<_>>(), ["https://a.test/index.json", "https://b.test/index.min.json"]);
        assert_eq!(manga.into_iter().collect::<Vec<_>>(), ["https://m.test"]);
        assert_eq!(legacy_repo_urls(&prefs[2..]), (BTreeSet::new(), BTreeSet::new()));
    }

    #[test]
    fn anime_store_upserts_with_defaults_and_never_duplicates() {
        let dir = tempfile::tempdir().unwrap();
        let c = schema_anime::open(&dir.path().join("a.db")).unwrap();
        restore_anime_store(&c, &store("https://x/index.json", "First")).unwrap();
        restore_anime_store(&c, &store("https://x/index.json", "Second")).unwrap();
        let got = kuro_db::statements_anime::extension_store(&c, "https://x/index.json").unwrap().unwrap();
        assert_eq!(got.name, "Second", "re-restore overwrites the metadata");
        assert_eq!(got.badge_label, "Second", "badge label falls back to the name");
        assert!(got.is_legacy, "isLegacy defaults to true");
        assert_eq!(kuro_db::statements_anime::extension_store_count(&c).unwrap(), 1);
    }

    fn button(name: &str, sort_index: i64, favorite: bool) -> BackupCustomButtons {
        BackupCustomButtons {
            name: name.into(),
            is_favorite: favorite,
            sort_index,
            content: format!("{name} content"),
            long_press_content: "lp".into(),
            on_startup: "".into(),
        }
    }

    /// An anime database with the seeded default button removed, so each test sees only its own rows.
    fn buttons_db() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let c = schema_anime::open(&dir.path().join("a.db")).unwrap();
        c.execute("DELETE FROM custom_buttons", []).unwrap();
        (dir, c)
    }

    #[test]
    fn restored_buttons_take_max_plus_one_in_backup_sort_order() {
        let (_dir, c) = buttons_db();
        restore_custom_buttons(&c, &[button("late", 5, false), button("early", 1, false)]).unwrap();
        let rows = custom_buttons(&c).unwrap();
        let order: Vec<(&str, i64)> = rows.iter().map(|b| (b.name.as_str(), b.sort_index)).collect();
        assert_eq!(order, vec![("early", 0), ("late", 1)]);
    }

    #[test]
    fn name_already_in_the_database_is_skipped() {
        let (_dir, c) = buttons_db();
        create_custom_button(&c, "Skip", "stored", "", "").unwrap();
        restore_custom_buttons(&c, &[button("Skip", 0, false), button("New", 1, false)]).unwrap();
        let rows = custom_buttons(&c).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].content, "stored", "no update to the stored button");
    }

    #[test]
    fn duplicate_names_inside_the_backup_make_one_row() {
        let (_dir, c) = buttons_db();
        restore_custom_buttons(&c, &[button("Dup", 0, false), button("Dup", 1, false)]).unwrap();
        assert_eq!(custom_buttons(&c).unwrap().len(), 1);
    }

    #[test]
    fn only_the_first_favourite_of_the_backup_becomes_favourite() {
        let (_dir, c) = buttons_db();
        restore_custom_buttons(&c, &[button("A", 0, true), button("B", 1, true)]).unwrap();
        let favs: Vec<String> = custom_buttons(&c).unwrap().into_iter().filter(|b| b.is_favorite).map(|b| b.name).collect();
        assert_eq!(favs, vec!["A".to_string()]);
    }

    #[test]
    fn no_new_favourite_when_the_database_has_one() {
        let (_dir, c) = buttons_db();
        let id = create_custom_button(&c, "Own", "", "", "").unwrap();
        kuro_db::statements_anime::toggle_custom_button_favorite(&c, id).unwrap();
        restore_custom_buttons(&c, &[button("Imported", 0, true)]).unwrap();
        let imported = custom_buttons(&c).unwrap().into_iter().find(|b| b.name == "Imported").unwrap();
        assert!(!imported.is_favorite);
    }

    #[test]
    fn same_repo_twice_succeeds_without_a_second_row() {
        let dir = tempfile::tempdir().unwrap();
        let c = schema_manga::open(&dir.path().join("m.db")).unwrap();
        let r = repo("https://r/one", "One", "fp1");
        restore_manga_repo(&c, &r).unwrap();
        restore_manga_repo(&c, &r).expect("D11: identical repo is not an error");
        assert_eq!(extension_repos::count(&c).unwrap(), 1);
    }

    #[test]
    fn same_url_with_another_fingerprint_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let c = schema_manga::open(&dir.path().join("m.db")).unwrap();
        restore_manga_repo(&c, &repo("https://r/one", "One", "fp1")).unwrap();
        let err = restore_manga_repo(&c, &repo("https://r/one", "Again", "fp2")).unwrap_err();
        assert_eq!(err, "Error Adding Manga Repo: Again : Already Exists with different signing key fingerprint");
    }

    #[test]
    fn another_url_with_an_existing_fingerprint_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let c = schema_manga::open(&dir.path().join("m.db")).unwrap();
        restore_manga_repo(&c, &repo("https://r/one", "One", "fp1")).unwrap();
        let err = restore_manga_repo(&c, &repo("https://r/two", "Two", "fp1")).unwrap_err();
        assert_eq!(err, "Error Adding Manga Repo: Two : One has the same signing key fingerprint");
        assert_eq!(extension_repos::count(&c).unwrap(), 1);
    }
}
