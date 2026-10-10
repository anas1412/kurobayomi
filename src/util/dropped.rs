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
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::*;

    const THIS_FILE: &str = "src/util/dropped.rs";

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                rust_files(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }

    /// The code lines (comment lines are skipped) of every Rust file under `dirs`, as `(file, line number, text)`.
    fn code_lines(dirs: &[&str]) -> Vec<(String, usize, String)> {
        let mut files = Vec::new();
        for dir in dirs {
            rust_files(&root().join(dir), &mut files);
        }
        let mut out = Vec::new();
        for file in files {
            let rel = file.strip_prefix(root()).unwrap().to_string_lossy().into_owned();
            if rel == THIS_FILE {
                continue;
            }
            let Ok(text) = fs::read_to_string(&file) else { continue };
            for (i, line) in text.lines().enumerate() {
                if !line.trim_start().starts_with("//") {
                    out.push((rel.clone(), i + 1, line.to_owned()));
                }
            }
        }
        out
    }

    /// Identifier-like words of a line, lower case.
    fn words(line: &str) -> Vec<String> {
        line.split(|c: char| !(c.is_alphanumeric() || c == '_')).filter(|w| !w.is_empty()).map(str::to_lowercase).collect()
    }

    fn hits(dirs: &[&str], forbidden: &[&str]) -> Vec<String> {
        code_lines(dirs)
            .into_iter()
            .filter(|(_, _, line)| words(line).iter().any(|w| forbidden.contains(&w.as_str())))
            .map(|(file, n, line)| format!("{file}:{n}: {}", line.trim()))
            .collect()
    }

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

    #[test]
    fn no_android_only_widget_or_compose_or_preference_type_is_used() {
        // T-36a-063, T-52a-058, T-52a-061
        let found = hits(&["src"], &["viewpager", "edittextpreference", "composeview", "setcomposecontent"]);
        assert!(found.is_empty(), "{found:#?}");
        // for this area only: the tracker's own `NumberPicker` model and the player's seek bar model are other areas' names
        let found = hits(&["src/shell", "src/util"], &["seekbar", "numberpicker"]);
        assert!(found.is_empty(), "{found:#?}");
    }

    #[test]
    fn no_dex_class_loader_package_installer_or_intent_code_exists() {
        // T-52a-035, T-52a-043, T-52a-049, T-52a-050, T-52a-055
        let found = hits(
            &["src"],
            &[
                "dex",
                "classloader",
                "pathclassloader",
                "ispackageinstalled",
                "shizuku",
                "miui",
                "getparcelableextra",
                "getserializableextra",
                "istvbox",
                "tvutils",
            ],
        );
        assert!(found.is_empty(), "{found:#?}");
        let found: Vec<_> = code_lines(&["src"]).into_iter().filter(|(_, _, l)| l.contains("Resources.getSystem")).collect();
        assert!(found.is_empty(), "{found:#?}");
    }

    #[test]
    fn no_foreground_service_wake_lock_permission_or_secure_flag_code_exists() {
        // T-52a-037, T-52a-057, T-53a-028, T-53a-029, T-53a-030, T-36a-035, T-52a-062
        let found = hits(
            &["src"],
            &[
                "setforegroundsafely",
                "startforeground",
                "wake_lock",
                "wakelock",
                "request_ignore_battery_optimizations",
                "post_notifications",
                "flag_secure",
                "set_secure_screen",
                "setsecurescreen",
            ],
        );
        assert!(found.is_empty(), "{found:#?}");
    }

    #[test]
    fn no_per_app_locale_service_or_permission_exists() {
        // T-53a-031
        let found = hits(&["src"], &["read_app_specific_locales", "applocalesmetadataholderservice"]);
        assert!(found.is_empty(), "{found:#?}");
    }

    #[test]
    fn no_dynamic_colour_code_exists() {
        // T-52a-063: the `pref_app_theme` key is kept by the theme page for its Linux schemes (area 49)
        let found = hits(&["src"], &["isdynamiccoloravailable", "dynamiccolors"]);
        assert!(found.is_empty(), "{found:#?}");
    }

    #[test]
    fn the_app_never_asks_for_always_on_top() {
        // T-53a-033
        let found = hits(&["src"], &["set_keep_above", "keep_above", "always_on_top"]);
        assert!(found.is_empty(), "{found:#?}");
    }

    #[test]
    fn no_rx_library_is_in_the_dependency_lock() {
        // T-52a-006
        let lock = fs::read_to_string(root().join("Cargo.lock")).unwrap();
        let rx: Vec<_> = lock
            .lines()
            .filter_map(|l| l.strip_prefix("name = \""))
            .filter(|n| n.starts_with("rx") || n.starts_with("rxrust") || n.contains("rxjava"))
            .collect();
        assert!(rx.is_empty(), "{rx:?}");
    }

    #[test]
    fn notice_actions_that_android_never_sent_are_not_built() {
        // TC-53-24 (T-53a-077). The `AppAction` enum lives in `src/misc/router.rs`: its definition, its dispatch arm and its codec
        // test are the only places `OpenChapter` and `OpenEpisode` appear. No builder constructs them, and `OpenEntry`
        // does not exist at all.
        let allowed = "src/misc/router.rs";
        let mut found = Vec::new();
        for (file, n, line) in code_lines(&["src", "crates"]) {
            let in_crate_src = file.starts_with("src/") || file.contains("/src/");
            if !in_crate_src {
                continue;
            }
            let w = words(&line);
            let entry = w.iter().any(|w| w == "openentry");
            let others = w.iter().any(|w| w == "openchapter" || w == "openepisode");
            if entry || (others && file != allowed) {
                found.push(format!("{file}:{n}: {}", line.trim()));
            }
        }
        assert!(found.is_empty(), "dropped notification actions are constructed:\n{found:#?}");
    }
}
