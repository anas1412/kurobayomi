//! The settings main menu (task T-47a-003) and the two-pane background rule.
//! Titles, subtitles and icons follow the Linux spec (`spec/settings.json`, `main`); About shows the build.
// Not reachable from `route.rs` yet: the main list in `pref_model/shell.rs` still reads `spec/settings.json`.
#![allow(dead_code)]

/// One row of the main menu. `target` is the screen id the row opens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuRow {
    pub title: &'static str,
    pub subtitle: String,
    pub icon: &'static str,
    pub target: &'static str,
}

/// About subtitle: `"<app> Stable <version>"` for release builds, `"Debug <commit>"` otherwise.
pub fn about_subtitle(app: &str, release: bool, version: &str, commit: &str) -> String {
    if release { format!("{app} Stable {version}") } else { format!("Debug {commit}") }
}

/// The eleven rows in their order. `about` is the About subtitle (see [`about_subtitle`]).
pub fn rows(about: &str) -> Vec<MenuRow> {
    let row = |title, subtitle: &str, icon, target| MenuRow { title, subtitle: subtitle.to_owned(), icon, target };
    vec![
        row("Appearance", "Theme, date & time format", "preferences-desktop-appearance-symbolic", "settings_appearance"),
        row("Library", "Categories, global update, chapter/episode swipe", "view-grid-symbolic", "settings_library"),
        row("Reader", "Reading mode, display, navigation", "x-office-document-symbolic", "settings_reader"),
        row("Player", "Internal player, gestures, decoders, subtitles", "video-display-symbolic", "settings_player"),
        row("Downloads", "Automatic download, download ahead", "folder-download-symbolic", "settings_downloads"),
        row("Tracking", "One-way progress sync, enhanced sync", "emblem-synchronizing-symbolic", "settings_tracking"),
        row("Browse", "Sources, extensions, global search", "compass-symbolic", "settings_browse"),
        row("Data and storage", "Manual & automatic backups, storage space", "drive-harddisk-symbolic", "settings_data_storage"),
        row("Security and privacy", "App lock", "security-high-symbolic", "settings_security"),
        row("Advanced", "Dump crash logs", "utilities-terminal-symbolic", "settings_advanced"),
        row("About", about, "help-about-symbolic", "settings_about"),
    ]
}

/// Lightness of the selected-pane background in two-pane mode: `-0.05` in dark mode, `+0.02` in light mode,
/// clamped to `[0, 1]`.
pub fn pane_lightness(lightness: f64, dark: bool) -> f64 {
    let shifted = if dark { lightness - 0.05 } else { lightness + 0.02 };
    shifted.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eleven_rows_in_the_task_order() {
        let titles: Vec<&str> = rows("x").iter().map(|r| r.title).collect();
        assert_eq!(
            titles,
            [
                "Appearance",
                "Library",
                "Reader",
                "Player",
                "Downloads",
                "Tracking",
                "Browse",
                "Data and storage",
                "Security and privacy",
                "Advanced",
                "About"
            ]
        );
    }

    #[test]
    fn no_more_row_routes_to_the_storage_tab() {
        // T-47ba-017: the Storage tab is reached from Updates; the More list opens the settings screens only
        assert!(rows("x").iter().all(|r| !r.target.starts_with("storage")));
    }

    #[test]
    fn player_row_opens_the_player_root_and_about_takes_the_given_subtitle() {
        let rows = rows("Kurobayomi Stable 0.1.0");
        assert_eq!(rows[3].target, "settings_player");
        assert_eq!(rows[10].subtitle, "Kurobayomi Stable 0.1.0");
    }

    #[test]
    fn about_subtitle_is_release_or_debug() {
        assert_eq!(about_subtitle("Kurobayomi", true, "0.1.0", "abc"), "Kurobayomi Stable 0.1.0");
        assert_eq!(about_subtitle("Kurobayomi", false, "0.1.0", "abc1234"), "Debug abc1234");
    }

    #[test]
    fn pane_lightness_shifts_by_theme_and_clamps() {
        assert!((pane_lightness(0.5, false) - 0.52).abs() < 1e-9);
        assert!((pane_lightness(0.5, true) - 0.45).abs() < 1e-9);
        assert_eq!(pane_lightness(0.99, false), 1.0);
        assert_eq!(pane_lightness(0.02, true), 0.0);
    }
}
