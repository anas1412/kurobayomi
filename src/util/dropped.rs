//! Android features the GUI package does not port, and the tests that keep each drop true (T-36a-019, T-36a-035,
//! T-36a-063, T-52a-006, T-52a-030, T-52a-035, T-52a-037, T-52a-040, T-52a-043, T-52a-046, T-52a-049, T-52a-050,
//! T-52a-055, T-52a-057, T-52a-058, T-52a-061, T-52a-062, T-52a-063, T-53a-028, T-53a-029, T-53a-030, T-53a-032,
//! T-53a-033, T-53a-077).
//!
//! The table lists the decision and the user loss, so a later lane does not bring one back by accident. The tests scan
//! the code lines of `src/` for the Android names; comments may still name them.

/// The note the security page shows beside the kept `secure_screen_v2` setting (T-52a-062, T-36a-035).
pub const SECURE_SCREEN_NOTE: &str = "Screenshot blocking is not available on Linux";

/// `(task, dropped, loss)`.
pub const DROPPED: &[(&str, &str, &str)] = &[
    ("T-36a-019", "system bar and edge-to-edge styling: GTK draws the title bar", "none on desktop"),
    ("T-36a-035", "secure screen flag: no Linux API hides a window from capture; `secure_screen_v2` stays readable for backups", "screenshots of the app are not blocked"),
    ("T-36a-063", "NumberPicker (use gtk::SpinButton), reveal animation, IME flag, ViewPager, spinner and seek bar listeners", "none"),
    ("T-52a-006", "RxJava to stream adapter: tokio watch and mpsc replace it", "none"),
    ("T-52a-030", "closeable array use: Drop closes every handle", "none"),
    ("T-52a-035", "extension class loader: extensions are designed in area 20", "none for users"),
    ("T-52a-037", "runtime permission check: portals ask when needed", "none"),
    ("T-52a-040", "default browser package lookup: the portal opens links", "none"),
    ("T-52a-043", "package and installer checks, MIUI, Shizuku, install permission", "none"),
    ("T-52a-046", "display cutout and navigation-bar scrim checks", "none on desktop"),
    ("T-52a-049", "intent extras getters", "none"),
    ("T-52a-050", "system resource booleans", "none"),
    ("T-52a-055", "TV box detection: the storage step uses the native folder picker", "none"),
    ("T-52a-057", "foreground promotion helper: JobRegistry::is_running shows busy state", "none"),
    ("T-52a-058", "EditText preference listener accessor: text settings are GTK entry rows", "none"),
    ("T-52a-061", "Compose host helpers: GTK widgets use the user's GTK theme", "none"),
    ("T-52a-062", "screenshot protection flag: the setting is kept and the page shows SECURE_SCREEN_NOTE", "screenshots are not blocked"),
    ("T-52a-063", "dynamic colour: no wallpaper-derived colours. `pref_app_theme` stays as the theme page's key, holding the Linux scheme names (area 49: System, Tokyo Night), never the Android Material You names", "wallpaper-derived colours"),
    ("T-53a-028", "wake lock and battery-optimisation prompts: none exist on Linux", "none"),
    ("T-53a-029", "foreground-service permissions: BackgroundHold and notices replace them", "none"),
    ("T-53a-030", "notification permission prompt: the toast fallback covers a missing notification server", "none"),
    ("T-53a-032", "Android application attributes (backup, heap, legacy storage, icons): the icon and label are in the desktop entry", "none"),
    ("T-53a-033", "picture-in-picture: the app never asks for always-on-top", "no floating video window"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_dropped_task_has_a_decision_and_a_recorded_loss() {
        for (task, what, loss) in DROPPED {
            assert!(task.starts_with("T-"), "{task}");
            assert!(!what.is_empty() && !loss.is_empty(), "{task}");
        }
        let mut ids: Vec<_> = DROPPED.iter().map(|d| d.0).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), DROPPED.len(), "one row per task");
    }

    #[test]
    fn the_screenshot_note_is_the_agreed_text() {
        assert_eq!(SECURE_SCREEN_NOTE, "Screenshot blocking is not available on Linux");
    }
}
