//! Decisions for the Android-only tracker plumbing (areas 27 to 33). Each entry is a decision recorded in code, and
//! the tests check that the code keeps it.
//!
//! * Activity lifecycle (T-27b-033, T-29a-045). `BaseOAuthLoginActivity`, the full-screen `LoadingScreen`, `finish()`
//!   and the clear-top relaunch are not ported. The main window is shown on callback (T-27b-019), and the login row
//!   shows a spinner during the exchange (T-29a-047).
//! * Login activity and link (T-29a-044). `TrackLoginActivity` and its intent filter are dropped. The MyAnimeList
//!   login uses its loopback redirect (T-29a-039). The shared link router in `oauth` still reads the registered
//!   `aniyomi://` redirects of AniList, Simkl and Bangumi (T-28a-040). It ignores `aniyomi://myanimelist-auth`.
//! * Credentials in backups (T-29a-046). The MyAnimeList token is in the platform secret store (`mal::store`), not in
//!   the preferences, so a backup does not carry it. A restored backup shows MyAnimeList logged out. The restore
//!   fixture test belongs to the backup area (22), which this crate does not own.
//! * Android-to-Rust map (T-30a-034, T-28a-041, T-32a-062). `android.graphics.Color` is a `u32` constant, `Uri` is
//!   `url::Url`, kotlinx.serialization is serde, OkHttp is reqwest, injekt is constructor injection through
//!   `TrackerDeps`, moko `StringResource` is a key, `withIOContext` is tokio, `SimpleDateFormat` is chrono, and
//!   `URLEncoder` is `url::form_urlencoded`. Replacement table: `p1-rust-design.md` section 12. There is no
//!   `openInBrowser(forceDefaultBrowser)`: links open in the default browser only. `MURating.copyTo` was dead code
//!   and is not ported.
//! * Score display (T-30a-035). A score is shown with Rust's `f64` display, so `7.5` reads `7.5` in every locale.
//!   Stored values do not change.
//! * Interceptors (T-33a-023). The OkHttp interceptors are not ported. Each request builder takes its auth context
//!   (`p1-rust-design.md` section 11).
//! * Service locator (T-33a-103). Injekt, `injectLazy` and lazy wiring are not ported. Dependencies come from
//!   constructor injection through `TrackerDeps`.
//! * Android resources (T-33a-104). `R.drawable.*`, `Color.rgb(...)` and `MR.strings.*` become asset keys, a `u32`
//!   colour and message keys.
//! * Dispatchers (T-33a-105). `withIOContext` is dropped because the tracker code is async end to end, and
//!   `withNonCancellableContext` becomes a detached `tokio::spawn` (area 27 section 4). The Kavita, Komga and
//!   Suwayomi calls `await` and do not use `spawn_blocking`.

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    /// The code lines of every `.rs` file under `src`, excluding this decision file and doc comments.
    fn code_lines() -> Vec<(PathBuf, String)> {
        fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    walk(&path, out);
                } else if path.extension().is_some_and(|e| e == "rs") && !path.ends_with("dropped.rs") {
                    out.push(path);
                }
            }
        }
        let mut files = Vec::new();
        walk(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
        files
            .into_iter()
            .flat_map(|path| {
                let text = std::fs::read_to_string(&path).unwrap();
                text.lines()
                    .filter(|l| !l.trim_start().starts_with("//"))
                    .map(|l| (path.clone(), l.to_owned()))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    fn assert_absent(word: &str) {
        for (path, line) in code_lines() {
            assert!(!line.contains(word), "{word} is Android-only: {}: {line}", path.display());
        }
    }

    #[test]
    fn t33a023_no_interceptor_type_is_left() {
        assert_absent("Interceptor");
    }

    #[test]
    fn t33a103_no_service_locator_is_left() {
        assert_absent("injectLazy");
        assert_absent("Injekt");
    }

    #[test]
    fn t33a104_no_android_resource_lookup_is_left() {
        assert_absent("R.drawable");
        assert_absent("MR.strings");
        assert_absent("Color.rgb(");
    }

    #[test]
    fn t33a105_no_coroutine_dispatcher_mapping_is_left() {
        assert_absent("withIOContext");
        assert_absent("withNonCancellableContext");
        assert_absent("runBlocking");
        for dir in ["kavita", "komga", "suwayomi"] {
            let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/trackers").join(dir);
            for entry in std::fs::read_dir(&root).unwrap() {
                let text = std::fs::read_to_string(entry.unwrap().path()).unwrap_or_default();
                assert!(!text.contains("spawn_blocking"), "{dir} blocks a thread");
            }
        }
    }

    #[test]
    fn t27b033_t29a045_no_activity_or_loading_screen_is_left() {
        assert_absent("Activity");
        assert_absent("LoadingScreen");
    }

    #[test]
    fn t30a034_t28a041_no_android_api_is_left_in_the_tracker_code() {
        for (path, line) in code_lines() {
            let lower = line.to_ascii_lowercase();
            assert!(!lower.contains("android"), "android in {}: {line}", path.display());
            assert!(!lower.contains("okhttp"), "okhttp in {}: {line}", path.display());
        }
    }

    #[test]
    fn t29a044_the_myanimelist_code_has_no_deep_link_scheme() {
        let mal = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/mal");
        for (path, line) in code_lines() {
            if path.starts_with(&mal) {
                assert!(!line.contains("aniyomi://"), "MyAnimeList uses the loopback redirect: {line}");
            }
        }
    }

    #[test]
    fn t32a062_there_is_no_per_browser_override() {
        assert_absent("forceDefaultBrowser");
        assert_absent("openInBrowser");
    }

    #[test]
    fn t30a035_a_score_reads_the_same_in_every_locale() {
        assert_eq!(crate::kitsu::score::display_score(7.5), "7.5");
        assert_eq!(crate::kitsu::score::display_score(7.0), "7");
    }
}
