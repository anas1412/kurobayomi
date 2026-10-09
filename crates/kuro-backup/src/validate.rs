//! Restore preflight (area 21, T-21a-024): sources that are not installed and trackers that are not logged in.
//! Decode failures return the key text, never the internal error (E-12).

use std::collections::BTreeSet;
use std::path::Path;

use crate::creator::SourceRegistry;
use crate::error::BackupError;
use crate::format;
use crate::schema::Backup;

/// Tracker identity and login state (area 27 provides the real one).
pub trait TrackerRegistry {
    /// `None` for an unknown tracker id; such ids are ignored (B-13.4).
    fn name(&self, tracker_id: i32) -> Option<String>;
    fn logged_in(&self, tracker_id: i32) -> bool;
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Preflight {
    pub missing_sources: Vec<String>,
    pub missing_trackers: Vec<String>,
}

/// Label for a missing source (B-13.2, E-17, E-18). A blank or all-digit stored name shows the id text or the stub
/// label; any other stored name is shown as is.
fn source_label(name: &str, id: i64, reg: &dyn SourceRegistry) -> String {
    if name.is_empty() || name.parse::<i64>().is_ok() {
        reg.stub_label(id)
    } else {
        name.to_owned()
    }
}

fn sorted_distinct(list: impl Iterator<Item = String>) -> Vec<String> {
    list.collect::<BTreeSet<_>>().into_iter().collect()
}

/// Missing sources (manga first, then anime; each distinct and sorted) and trackers not logged in.
pub fn preflight(backup: &Backup, sources: &dyn SourceRegistry, trackers: &dyn TrackerRegistry) -> Preflight {
    let manga = sorted_distinct(
        backup
            .sources
            .iter()
            .filter(|s| !sources.is_installed(s.source_id))
            .map(|s| source_label(&s.name, s.source_id, sources)),
    );
    let anime = sorted_distinct(
        backup
            .anime_sources
            .iter()
            .filter(|s| !sources.is_installed(s.source_id))
            .map(|s| source_label(&s.name, s.source_id, sources)),
    );
    let tracker_ids: BTreeSet<i32> = backup
        .manga
        .iter()
        .flat_map(|m| m.tracking.iter().map(|t| t.sync_id))
        .chain(backup.anime.iter().flat_map(|a| a.tracking.iter().map(|t| t.sync_id)))
        .collect();
    let missing_trackers = sorted_distinct(
        tracker_ids
            .into_iter()
            .filter(|id| !trackers.logged_in(*id))
            .filter_map(|id| trackers.name(id)),
    );
    Preflight {
        missing_sources: manga.into_iter().chain(anime).collect(),
        missing_trackers,
    }
}

/// A backup with nothing in it is refused (E-04, a deliberate deviation: Kotlin restores it as zero work). Random
/// text decodes to an almost empty container, so this is the check that keeps a wrong file from restoring silently.
/// Every list the restore writes counts, so a file the Create side writes for any `can_create` option restores (X-112).
/// The source name lists only label missing sources; nothing is restored from them, so they do not count.
fn has_content(b: &Backup) -> bool {
    !(b.manga.is_empty()
        && b.anime.is_empty()
        && b.preferences.is_empty()
        && b.categories.is_empty()
        && b.anime_categories.is_empty()
        && b.extensions.is_empty()
        && b.source_preferences.is_empty()
        && b.manga_extension_repos.is_empty()
        && b.anime_extension_stores.is_empty()
        && b.custom_buttons.is_empty())
}

/// A decoded backup and its preflight. The restore job takes the backup, so the file is decoded once (T-22a-041).
#[derive(Debug, Clone, PartialEq)]
pub struct Checked {
    pub backup: Backup,
    pub preflight: Preflight,
}

/// Decodes the file once and runs the preflight on it. A decode error is returned with its key text (`user_text`).
pub fn check_file(path: &Path, sources: &dyn SourceRegistry, trackers: &dyn TrackerRegistry) -> Result<Checked, BackupError> {
    let backup = format::decode_file(path)?;
    if !has_content(&backup) {
        return Err(BackupError::NoEntries);
    }
    let preflight = preflight(&backup, sources, trackers);
    Ok(Checked { backup, preflight })
}

/// Decodes the file and runs the preflight, keeping only the preflight. A decode error is returned with its key text.
pub fn preflight_file(path: &Path, sources: &dyn SourceRegistry, trackers: &dyn TrackerRegistry) -> Result<Preflight, BackupError> {
    check_file(path, sources, trackers).map(|checked| checked.preflight)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{BackupAnime, BackupAnimeSource, BackupAnimeTracking, BackupManga, BackupSource, BackupTracking};

    struct Sources;
    impl SourceRegistry for Sources {
        fn is_installed(&self, id: i64) -> bool {
            id == 1
        }
        fn name_of(&self, id: i64) -> String {
            if id == 1 { "Installed".into() } else { String::new() }
        }
        fn stub_label(&self, id: i64) -> String {
            if id == 2 { "Stub (EN)".into() } else { id.to_string() }
        }
    }

    struct Trackers;
    impl TrackerRegistry for Trackers {
        fn name(&self, id: i32) -> Option<String> {
            match id {
                1 => Some("Alpha".into()),
                2 => Some("Beta".into()),
                _ => None,
            }
        }
        fn logged_in(&self, id: i32) -> bool {
            id == 2
        }
    }

    fn tracking(sync_id: i32) -> BackupTracking {
        BackupTracking { sync_id, library_id: 0, ..Default::default() }
    }

    #[test]
    fn preflight_missing_manga_source_label() {
        let b = Backup { sources: vec![BackupSource { name: "Ghost".into(), source_id: 9 }, BackupSource { name: "Installed".into(), source_id: 1 }], ..Default::default() };
        assert_eq!(preflight(&b, &Sources, &Trackers).missing_sources, ["Ghost"]);
    }

    #[test]
    fn preflight_numeric_name_is_id() {
        let b = Backup { sources: vec![BackupSource { name: "2".into(), source_id: 2 }], ..Default::default() };
        assert_eq!(preflight(&b, &Sources, &Trackers).missing_sources, ["Stub (EN)"]);
    }

    #[test]
    fn preflight_blank_name_shows_id() {
        let b = Backup { sources: vec![BackupSource { name: String::new(), source_id: 7 }], ..Default::default() };
        let got = preflight(&b, &Sources, &Trackers).missing_sources;
        assert_eq!(got, vec!["7".to_string()], "E-17: never an empty string");
    }

    #[test]
    fn preflight_tracker_not_logged_in() {
        let b = Backup {
            manga: vec![BackupManga { source: 1, url: "u".into(), tracking: vec![tracking(1), tracking(2)], ..Default::default() }],
            ..Default::default()
        };
        assert_eq!(preflight(&b, &Sources, &Trackers).missing_trackers, ["Alpha"]);
    }

    #[test]
    fn preflight_unknown_tracker_ignored() {
        let b = Backup {
            anime: vec![BackupAnime { source: 1, url: "u".into(), tracking: vec![BackupAnimeTracking { sync_id: 999, ..Default::default() }], ..Default::default() }],
            ..Default::default()
        };
        assert!(preflight(&b, &Sources, &Trackers).missing_trackers.is_empty());
    }

    #[test]
    fn preflight_distinct_sorted_manga_before_anime() {
        let b = Backup {
            sources: vec![BackupSource { name: "Zed".into(), source_id: 5 }, BackupSource { name: "Ann".into(), source_id: 6 }, BackupSource { name: "Zed".into(), source_id: 5 }],
            anime_sources: vec![BackupAnimeSource { name: "Mid".into(), source_id: 8 }, BackupAnimeSource { name: "Bob".into(), source_id: 9 }],
            ..Default::default()
        };
        assert_eq!(preflight(&b, &Sources, &Trackers).missing_sources, ["Ann", "Zed", "Bob", "Mid"]);
    }

    #[test]
    fn empty_backup_is_refused_with_key_text() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.tachibk");
        // A decodable container with one unknown field: nothing to restore.
        let mut w = crate::wire::Writer::new();
        w.int64(900, 1);
        std::fs::write(&path, w.finish()).unwrap();
        let err = preflight_file(&path, &Sources, &Trackers).unwrap_err();
        assert!(matches!(err, BackupError::NoEntries));
        assert_eq!(err.user_text(), "Backup does not contain any library entries");
    }

    #[test]
    fn a_file_with_only_one_restorable_list_has_content() {
        use crate::schema::{BackupCustomButtons, BackupExtensionRepos, BackupExtensionStore, BackupSourcePreferences};
        let lists = [
            Backup { custom_buttons: vec![BackupCustomButtons::default()], ..Default::default() },
            Backup { anime_extension_stores: vec![BackupExtensionStore::default()], ..Default::default() },
            Backup { manga_extension_repos: vec![BackupExtensionRepos::default()], ..Default::default() },
            Backup { source_preferences: vec![BackupSourcePreferences::default()], ..Default::default() },
        ];
        for b in &lists {
            assert!(has_content(b));
        }
        assert!(!has_content(&Backup::default()));
    }

    #[test]
    fn preflight_decode_error_message_is_key_text() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.tachibk");
        std::fs::write(&path, b"\x0A\xFF\xFF broken").unwrap();
        let err = preflight_file(&path, &Sources, &Trackers).unwrap_err();
        assert_eq!(err.user_text(), "Backup file is corrupted");
        assert!(!err.user_text().contains("java.io"));
    }
}
