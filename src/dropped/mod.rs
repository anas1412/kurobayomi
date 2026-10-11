//! Decision records for the Android-only pieces of areas 11, 26, 35 and 53 (T-26a-120 to T-26a-126, T-35a-012,
//! T-35a-025 to T-35a-030, T-35a-056, T-35a-059 to T-35a-065, T-53a-036 to T-53a-068). Each row names what was
//! dropped, what replaces it and what the user loses. `docs/drop-decisions.md` carries the same rows (a test compares
//! them), and the tests below scan the code lines of `src/` so a later change cannot bring a dropped name back.
//!
//! `register` is the drop register of the legacy import (T-11a-029).
//!
//! Comments may still name a dropped thing; only code lines are scanned. The platform-specific pieces of the
//! replacements are in `kuro-platform`, so nothing here is OS-specific.

pub mod register;

/// `(task, dropped, replacement, loss, names that must not appear in the code of src/)`.
pub struct Decision {
    pub task: &'static str,
    pub dropped: &'static str,
    pub replacement: &'static str,
    pub loss: &'static str,
    pub absent: &'static [&'static str],
}

const fn d(
    task: &'static str,
    dropped: &'static str,
    replacement: &'static str,
    loss: &'static str,
    absent: &'static [&'static str],
) -> Decision {
    Decision { task, dropped, replacement, loss, absent }
}

pub const DECISIONS: &[Decision] = &[
    d("T-26a-120", "foreground service declaration of the library update job", "the resident process or the systemd timer", "nothing beyond background running when the app is closed in mode A without the portal", &["ForegroundInfo", "setForeground", "FOREGROUND_SERVICE", "foreground_service_type"]),
    d("T-26a-121", "notification channels and the deprecated channel cleanup", "freedesktop urgency per kind and one notification switch (T-26a-036)", "per-channel system settings", &["createNotificationChannel", "create_notification_channel", "deleteNotificationChannel", "NotificationChannelId"]),
    d("T-26a-122", "Wi-Fi pre-check for Android versions below 28", "the NetworkManager constraint (T-26a-027) in every case", "none", &["SDK_INT", "Build.VERSION", "Build::VERSION"]),
    d("T-26a-123", "FileProvider URIs for shared files", "plain file:// URIs from gio (T-26a-096)", "none", &["FileProvider", "content://"]),
    d("T-26a-124", "Injekt dependency lookup", "explicit PassDeps construction at startup", "none", &["Injekt", "injectLazy", "ServiceLocator"]),
    d("T-26a-125", "WorkManager migrations", "validation of the stored interval: only 0, 12, 24, 48, 72 and 168 hours, other values snap to the nearest choice", "none", &["cancelAllWorkByTag", "cancelAllWorks", "UseWorkManager", "WorkManager"]),
    d("T-26a-126", "decoding the launcher icon bitmap for notifications", "the freedesktop app icon name", "none", &["BitmapFactory", "decodeResource"]),
    d("T-35a-012", "channel creation and the nine deprecated channel ids", "the notification switches of T-35a-004", "per-channel system settings", &["createNotificationChannel", "create_notification_channel", "deleteNotificationChannel"]),
    d("T-35a-025", "the unused ACTION_OPEN_ENTRY constant", "entry opening of T-35a-015", "none", &["ACTION_OPEN_ENTRY"]),
    d("T-35a-026", "group summary dismissal of the notification receiver", "none; desktop notices have no group summary", "none visible; a single notice cannot be left behind", &["setGroupSummary", "group_summary", "GroupSummary", "GROUP_SUMMARY"]),
    d("T-35a-027", "Samsung S Pen remote actions", "the five actions as keyboard keys (T-35a-028)", "S Pen remote gestures", &["s_pen_actions", "SPenActions", "remote-actions", "REMOTE_ACTION"]),
    d("T-35a-029", "searchable.xml, the Android global search hint and search provider", "the in-app global search entry; the GNOME search provider is an optional extra of T-53a-066, not part of v1", "system-wide search for library titles on Android", &["searchable.xml", "SearchableInfo", "SearchManager"]),
    d("T-35a-030", "FileProvider paths (provider_paths.xml)", "plain file paths; no content URIs are created", "none", &["FileProvider", "provider_paths", "content://"]),
    d("T-35a-031", "network_security_config.xml (the Android cleartext and certificate-authority policy)", "the reqwest client: rustls with the system certificate verifier, so the system and user-installed CAs are trusted, and http:// URLs are not refused", "none", &["network_security_config", "cleartextTrafficPermitted"]),
    d("T-35a-056", "APK install and in-place AppImage self-replacement", "the update notice offers Open folder and the user replaces the file", "the app cannot replace itself", &["InstallApk", "install_apk", "PackageInstaller", "self_replace", "replace_appimage"]),
    d("T-35a-059", "foreground service and the data sync service type", "none; a desktop process is not suspended", "none", &["FOREGROUND_SERVICE", "DATA_SYNC", "dataSync", "foreground_service_type"]),
    d("T-35a-060", "the OsTooOld result of the update check", "none; the check has no operating system version gate", "none", &["OsTooOld", "update_check_eol", "os_too_old"]),
    d("T-35a-061", "the preview build channel with commit-count tags", "one stable channel", "preview builds through the in-app updater", &["aniyomi-preview"]),
    d("T-35a-063", "the runtime notification permission check", "the toast fallback of T-35a-011 and T-35a-002 when the notification service is missing", "none", &[concat!("POST_", "NOTIFICATIONS"), "areNotificationsEnabled", "are_notifications_enabled"]),
    d("T-35a-064", "accent colour tint on notifications", "the desktop theme tint", "Android-style tinted notifications", &["accent_blue"]),
    d("T-35a-065", "Android sound override for the backup-complete notice", "none; the category keeps high priority and the desktop picks the sound", "the backup-complete notice may play the desktop sound", &["setSound", "set_sound", "notification_sound"]),
    d("T-53a-036", "extension install services and activities", "extension install of area 20", "installing APK extensions, and the vendor-skin timing workaround (defect E-29) is not carried", &["InstallService", "InstallActivity", "ExtensionInstallReceiver", "PackageInstaller"]),
    d("T-53a-038", "package install, delete and query permissions and the privileged installer provider; the extension_installer preference is not ported", "none; no package query or install permission exists on Linux", "installing extensions through the privileged helper app", &["REQUEST_INSTALL_PACKAGES", "REQUEST_DELETE_PACKAGES", "UPDATE_PACKAGES_WITHOUT_USER_ACTION", "QUERY_ALL_PACKAGES", "ShizukuProvider", "ExtensionInstallerPreference", "extension_installer"]),
    d("T-53a-046", "the deep-link trampoline activities", "route_request receives the search request directly", "none; the forwarding was internal", &["DeepLinkAnimeActivity", "DeepLinkMangaActivity", "trampoline"]),
    d("T-53a-052", "the ten unreferenced player animations", "none; the player has no transition, as in Aniyomi", "none", &["player_enter_", "player_exit_", "player_fade_"]),
    d("T-53a-056", "the unused preference switch layout", "gtk::Switch rows", "none", &["PrefWidgetSwitchMaterial", "pref_widget_switch_material"]),
    d("T-53a-059", "the chapter download menu", "the row menu of the download queue (T-53a-060)", "none", &["R.menu.chapter_download"]),
    d("T-53a-061", "the old bottom navigation menu", "the HomeScreen tab set of area 36", "none", &["R.menu.main_nav", "ic_animelibrary_selector_24dp"]),
    d("T-53a-062", "the md_* keep rule", "none; no md_* layout exists", "none", &[]),
    d("T-53a-064", "FileProvider and the path whitelist", "files are opened through Opener", "none visible", &["FileProvider", "provider_paths", "content://"]),
    d("T-53a-065", "S Pen remote actions", "keys Right, Left, Page Up, Page Down, N and P in the reader and player; Escape and Alt+Left go back", "S Pen remote gestures", &["s_pen_actions", "REMOTE_ACTION"]),
    d("T-53a-067", "the empty module manifests and the macrobenchmark module", "one Cargo crate per module, none needs a manifest", "none", &["AndroidManifest"]),
    d("T-53a-068", "the home-screen widget receivers and provider files", "the Updates tab (area 42) and desktop notices (area 35); the optional Plasma applets read the same data", "the Android home-screen widget", &["AppWidgetProvider", "GlanceAppWidget", "appwidget-provider", "AppWidgetReceiver"]),
];

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::*;

    const DOCS: &str = include_str!("../../docs/drop-decisions.md");

    /// Files that may hold the text `content://` in code: the cover loader refuses such a URL, and the torrent route of the
    /// player crate is outside `src/`.
    const CONTENT_URI_REFUSERS: [&str; 1] = ["src/feed/cover.rs"];

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else {
                out.push(path);
            }
        }
    }

    /// The code lines (comment lines skipped) of every Rust file of `src/` except this directory: `(file, line)`.
    fn code_lines() -> Vec<(String, String)> {
        let mut files = Vec::new();
        walk(&root().join("src"), &mut files);
        let mut out = Vec::new();
        for file in files.into_iter().filter(|f| f.extension().is_some_and(|e| e == "rs")) {
            let rel = file.strip_prefix(root()).unwrap().to_string_lossy().into_owned();
            if rel.starts_with("src/dropped/") {
                continue;
            }
            let Ok(text) = fs::read_to_string(&file) else { continue };
            out.extend(text.lines().filter(|l| !l.trim_start().starts_with("//")).map(|l| (rel.clone(), l.to_owned())));
        }
        out
    }

    fn found(names: &[&str]) -> Vec<String> {
        code_lines()
            .into_iter()
            .filter(|(file, line)| names.iter().any(|n| line.contains(n) && !(*n == "content://" && CONTENT_URI_REFUSERS.contains(&file.as_str()))))
            .map(|(file, line)| format!("{file}: {}", line.trim()))
            .collect()
    }

    #[test]
    fn every_decision_has_every_field_and_one_row_per_task() {
        for d in DECISIONS {
            assert!(d.task.starts_with("T-"), "{}", d.task);
            assert!(!d.dropped.is_empty() && !d.replacement.is_empty() && !d.loss.is_empty(), "{}", d.task);
            assert!(!d.dropped.contains('|') && !d.replacement.contains('|') && !d.loss.contains('|'), "{} breaks the table", d.task);
        }
        let ids: BTreeSet<_> = DECISIONS.iter().map(|d| d.task).collect();
        assert_eq!(ids.len(), DECISIONS.len());
    }

    #[test]
    fn every_decision_is_recorded_in_the_docs_with_the_same_words() {
        for d in DECISIONS {
            let row = format!("| {} | {} | {} | {} |", d.task, d.dropped, d.replacement, d.loss);
            assert!(DOCS.lines().any(|l| l == row), "docs/drop-decisions.md misses the row: {row}");
        }
    }

    #[test]
    fn no_dropped_name_is_in_the_code_of_src() {
        for d in DECISIONS.iter().filter(|d| !d.absent.is_empty()) {
            let hits = found(d.absent);
            assert!(hits.is_empty(), "{} came back: {hits:#?}", d.task);
        }
    }

    #[test]
    fn t35a030_no_content_uri_is_built_in_src() {
        // the cover loader only refuses one; nothing builds one
        let built: Vec<_> = found(&["content://"]);
        assert!(built.is_empty(), "{built:#?}");
        let refusers: Vec<_> = code_lines().into_iter().filter(|(f, l)| CONTENT_URI_REFUSERS.contains(&f.as_str()) && l.contains("content://")).collect();
        assert!(refusers.iter().all(|(_, l)| !l.contains("format!")), "a content URI is formatted: {refusers:#?}");
    }

    #[test]
    fn t26a125_the_stored_update_interval_snaps_to_the_six_choices() {
        let choices = kuro_prefs::ranges::LIBRARY_UPDATE_INTERVAL;
        assert_eq!(choices.values(), [0, 12, 24, 48, 72, 168]);
        for (stored, snapped) in [(0, 0), (12, 12), (1, 0), (2, 0), (7, 12), (30, 24), (36, 24), (60, 48), (100, 72), (500, 168), (-5, 0)] {
            assert_eq!(choices.clamped(stored), snapped, "{stored}");
        }
    }

    #[test]
    fn t26a126_the_notification_icon_is_a_cover_file_not_a_decoded_resource() {
        let _: fn(&Path, &Path, &str) -> Option<PathBuf> = crate::misc::notify_icon::circle_icon;
    }

    #[test]
    fn t35a029_the_in_app_global_search_entry_exists() {
        assert!(code_lines().iter().any(|(_, l)| l.contains("Global search")));
    }

    #[test]
    fn t53a046_the_search_request_is_routed_without_a_trampoline() {
        let _ = crate::shell::intents::route::route_request;
    }

    #[test]
    fn t53a062_no_md_layout_exists() {
        let mut files = Vec::new();
        walk(&root().join("src"), &mut files);
        walk(&root().join("crates"), &mut files);
        let md: Vec<_> = files.iter().filter(|f| f.file_name().is_some_and(|n| n.to_string_lossy().starts_with("md_"))).collect();
        assert!(md.is_empty(), "{md:?}");
    }

    #[test]
    fn t53a067_no_module_has_an_android_manifest() {
        let mut files = Vec::new();
        for dir in ["src", "crates", "dist", "packaging", "scripts", "docs"] {
            walk(&root().join(dir), &mut files);
        }
        let manifests: Vec<_> = files.iter().filter(|f| !f.components().any(|c| c.as_os_str() == "target") && f.file_name().is_some_and(|n| n == "AndroidManifest.xml")).collect();
        assert!(manifests.is_empty(), "{manifests:?}");
    }

    #[test]
    fn t53a065_the_reader_and_player_keep_the_s_pen_keys() {
        use crate::reader::viewers::keys::{pager_key, InputKey, KeyContext, KeyEvent};
        let ctx = KeyContext { volume_enabled: false, volume_inverted: false, menu_visible: false };
        for key in [InputKey::Right, InputKey::Left, InputKey::PageDown, InputKey::PageUp] {
            let out = pager_key(KeyEvent { key, ctrl: false, released: false }, ctx);
            assert!(out.claimed && out.action.is_some(), "{key:?} turns a page in the reader");
        }
        assert_eq!(crate::reader::nav::key_step('n'), Some(crate::reader::nav::Step::Next));
        assert_eq!(crate::reader::nav::key_step('p'), Some(crate::reader::nav::Step::Previous));
        assert_eq!((crate::misc::shortcuts::NEXT_ITEM, crate::misc::shortcuts::PREVIOUS_ITEM), ("n", "p"));
        assert_eq!(crate::misc::shortcuts::BACK_ALT_LEFT, "<Alt>Left");
        // the player seeks with the arrows; N and Shift+P are the item shortcuts of misc::shortcuts
        let none = kuro_player::keys::Modifiers::default();
        for keyval in [0xff51_u32, 0xff53] {
            assert!(kuro_player::shortcuts::action_for(keyval, none, false).is_some());
        }
        assert_eq!(crate::misc::shortcuts::PLAYER_PREVIOUS_ITEM, "<Shift>p");
    }
}
