//! The Manage notifications row of Advanced settings (task T-47a-072). It opens the KDE notification settings page
//! (`systemsettings kcm_notifications`). The row is hidden when `systemsettings` is not on the search path.
//!
//! Decision: the acceptance also names a `org.freedesktop.Notifications` settings URI through the desktop opener. No such
//! URI is defined, so only the KDE page is opened and the other desktops get no row.
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::pref_model::model::PreferenceItem;
use crate::settings_ui::rows::{multi_choice, text};

pub const TITLE: &str = "Manage notifications";
pub const PROGRAM: &str = "systemsettings";
pub const PAGE: &str = "kcm_notifications";

/// The first file named `program` in the directories of `path_var` (a `PATH` value), if any.
pub fn find_in(path_var: Option<&OsStr>, program: &str) -> Option<PathBuf> {
    std::env::split_paths(path_var?).map(|dir| dir.join(program)).find(|candidate| candidate.is_file())
}

/// The row for a search path: enabled when the settings program is on it.
pub fn row_with(path_var: Option<&OsStr>) -> PreferenceItem {
    let mut row = text(TITLE, None, find_in(path_var, PROGRAM).is_some());
    if let PreferenceItem::Text { on_click, .. } = &mut row {
        *on_click = Some(Rc::new(open));
    }
    row
}

/// The English name of each channel in `CATEGORIES`, shown in the mute list.
const CHANNEL_LABELS: [(&str, &str); 13] = [
    ("common_channel", "Common"),
    ("library_progress_channel", "Library update progress"),
    ("library_errors_channel", "Library update errors"),
    ("new_chapters_episodes_channel", "New chapters and episodes"),
    ("downloader_progress_channel", "Download progress"),
    ("downloader_error_channel", "Download errors"),
    ("backup_restore_progress_channel", "Backup and restore progress"),
    ("backup_restore_complete_channel_v2", "Backup and restore complete"),
    ("incognito_mode_channel", "Incognito mode"),
    ("torrent_server_channel", "Torrent server"),
    ("http_server_channel", "HTTP server"),
    ("app_apk_update_channel", "App updates"),
    ("ext_apk_update_channel", "Extension updates"),
];

/// The list of channels: a ticked channel is muted. The choice is the `muted_notification_channels` preference, which
/// the app notifier reads on every send.
pub fn mute_row(base: &kuro_prefs::base::BasePrefs) -> PreferenceItem {
    multi_choice("Muted notification channels", &base.muted_notification_channels, &CHANNEL_LABELS, true)
}

/// The row for the current `PATH`.
pub fn row() -> PreferenceItem {
    row_with(std::env::var_os("PATH").as_deref())
}

fn open() {
    if let Err(err) = kuro_platform::process::spawn_detached(Path::new(PROGRAM), &[OsStr::new(PAGE)]) {
        tracing::warn!(%err, "the notification settings did not open");
    }
}

#[cfg(test)]
use kuro_platform::notifications::CATEGORIES;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_channel_can_be_muted_from_the_list() {
        for category in CATEGORIES {
            assert!(CHANNEL_LABELS.iter().any(|(id, _)| *id == category.channel_id), "{}", category.channel_id);
        }
        assert_eq!(CHANNEL_LABELS.len(), CATEGORIES.len());
    }

    #[test]
    fn the_program_is_found_in_a_listed_directory() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(PROGRAM), b"").unwrap();
        let path_var = std::env::join_paths([dir.path()]).unwrap();
        assert_eq!(find_in(Some(&path_var), PROGRAM), Some(dir.path().join(PROGRAM)));
    }

    #[test]
    fn a_missing_program_or_no_path_finds_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let path_var = std::env::join_paths([dir.path()]).unwrap();
        assert_eq!(find_in(Some(&path_var), PROGRAM), None);
        assert_eq!(find_in(None, PROGRAM), None);
    }

    #[test]
    fn the_row_is_disabled_and_so_hidden_without_the_program() {
        let dir = tempfile::tempdir().unwrap();
        let path_var = std::env::join_paths([dir.path()]).unwrap();
        assert!(!row_with(Some(&path_var)).enabled());
        std::fs::write(dir.path().join(PROGRAM), b"").unwrap();
        assert!(row_with(Some(&path_var)).enabled());
    }
}
