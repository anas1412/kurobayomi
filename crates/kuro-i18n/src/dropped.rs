//! Decisions for the localisation catalog and the Android localisation API (T-51a-019, T-51a-020, T-51a-022, T-51a-023).
//!
//! * Names with no literal reference (T-51a-019). Keep all 133 names (86 from the core base and 47 from the app
//!   base), so no translation is lost for a later feature. Dynamic lookups are checked before any removal: the
//!   debanding names (`pref_debanding_none`, `pref_debanding_cpu`, `pref_debanding_gpu`, `pref_debanding_yuv420p`) are
//!   read through the preference registry in `kuro-prefs`. No unused name is removed. The names of features that the
//!   desktop app does not have are another matter: `import::DROPPED_NAMES` lists them and the importer leaves them
//!   out of every catalog (no orientation, battery optimisation, secure screen, extension installer, swipe action,
//!   volume and brightness gesture, tablet mode or dynamic colour text).
//! * Data-quality fixes in the English catalog (T-51a-020). The generated catalog is not edited by hand. These are the
//!   rules the catalog keeps, and the tests below check them:
//!   - `confirm_exit` has the text "Enable Horizontal Seek". It is not used, and its text is not reused for an exit
//!     prompt.
//!   - The trailing space of `pref_show_next_episode_airing_time` stays as `{" "}`.
//!   - Duplicate pairs (`pref_audio_channels` and `pref_player_audio_channels`, `pref_audio_pitch_correction_title`
//!     and `pref_player_audio_pitch_correction`) both stay. The UI uses one of each pair.
//!   - Identical plural forms stay as they are.
//! * Build plumbing (T-51a-022). No Gradle, moko, AGP or locales-config code is ported. The build script `build.rs`
//!   and the `xtask` replace them, and the plan in `p1-data-and-config.md` section 2 is the reference.
//! * Notification channels (T-51a-036). Linux notifications have no channels, so the `channel_*` names are not
//!   used as channel ids. They stay in the catalog (no name is removed, above), and no code reads them yet. The
//!   notification settings view can use them as category labels later.
//! * S-Pen strings (T-51a-037). There are no stylus actions on the desktop. `spen_previous_page` and
//!   `spen_next_page` stay in the catalog unused, as the names rule above says.
//! * Android localisation API (T-51a-023). `Context.stringResource`, `StringDesc`, the moko backslash workaround and
//!   `AppCompatDelegate` locale calls are not ported. The replacements are [`crate::tr`], [`crate::trf`],
//!   [`crate::tr_plural`], the `app_language` preference (see `picker`), and `glib::language_names()` passed to
//!   [`crate::init`]. No code reproduces the backslash replacement.

#[cfg(test)]
mod tests {
    use std::path::Path;

    /// The English catalog as the app ships it.
    const ENGLISH: &str = include_str!("../locales/en/kurobayomi.ftl");

    /// Every `.rs` file of this crate's `src` directory, as text, except this decision file.
    fn sources() -> Vec<String> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "rs") && !p.ends_with("dropped.rs"))
            .map(|p| std::fs::read_to_string(p).unwrap())
            .collect()
    }

    fn line_of(key: &str) -> &str {
        ENGLISH
            .lines()
            .find(|l| l.starts_with(&format!("{key} =")))
            .unwrap_or_else(|| panic!("{key} is missing from the English catalog"))
    }

    #[test]
    fn the_confirm_exit_text_is_not_reused_by_the_app() {
        assert_eq!(line_of("confirm_exit"), "confirm_exit = Enable Horizontal Seek");
        for text in sources() {
            assert!(!text.contains("confirm_exit"), "confirm_exit must stay unused");
        }
    }

    #[test]
    fn the_airing_time_trailing_space_is_an_explicit_placeable() {
        assert!(line_of("pref_show_next_episode_airing_time").ends_with("{\" \"}"));
    }

    #[test]
    fn t51a036_the_channel_names_stay_in_the_catalog_and_unused() {
        for key in [
            "channel_common",
            "channel_progress",
            "channel_complete",
            "channel_errors",
            "channel_new_chapters",
            "channel_app_updates",
            "channel_ext_updates",
            "channel_new_chapters_episodes",
        ] {
            line_of(key);
            for text in sources() {
                assert!(!text.contains(key), "{key} must stay unused");
            }
        }
    }

    #[test]
    fn t51a037_the_spen_names_stay_in_the_catalog_and_unused() {
        for key in ["spen_previous_page", "spen_next_page"] {
            line_of(key);
            for text in sources() {
                assert!(!text.contains(key), "{key} must stay unused");
            }
        }
    }

    #[test]
    fn duplicate_pairs_are_both_kept() {
        for key in [
            "pref_audio_channels",
            "pref_player_audio_channels",
            "pref_audio_pitch_correction_title",
            "pref_player_audio_pitch_correction",
        ] {
            line_of(key);
        }
    }
}
