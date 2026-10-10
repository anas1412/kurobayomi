//! Browse preferences (T-40a-032, T-40b-005). The keys live in the one settings file with the other preferences, under
//! `[sources]`, rather than in a separate `browse.toml`: the foundation's `SourcePrefs` already owns the upstream keys,
//! and two files would each need their own atomic write and unknown-key handling. The keys this lane adds are typed here.

use kuro_prefs::PrefError;
use kuro_prefs::library::LibraryDisplayMode;
use kuro_prefs::pref::Pref;
use kuro_prefs::source::SourcePrefs;
use kuro_prefs::store::PrefStore;

/// `migrate_flags` default: every migrate option on (`Int.MAX_VALUE`).
pub const MIGRATE_FLAGS_DEFAULT: i32 = i32::MAX;
/// `default_category`: -1 asks every time.
pub const DEFAULT_CATEGORY_ASK: i32 = -1;

/// The typed browse preferences. Each field reads and writes one stored key.
pub struct BrowsePrefs {
    /// `[sources]`: the upstream source keys (display mode, languages, pins, disabled sources, migrate sorting, the
    /// last-used source, the global-search filter toggle and the library-hide switch).
    pub sources: SourcePrefs,
    /// `migrate_flags`: which options the migrate dialog offers as selected.
    pub migrate_flags: Pref<i32>,
    /// `default_category` (owned by the library screen): -1 asks, 0 is uncategorised, above 0 a category id.
    pub default_manga_category: Pref<i32>,
    /// `pref_library_columns_portrait_key`: 0 means adaptive.
    pub library_columns_portrait: Pref<i32>,
    /// `pref_library_columns_landscape_key`: 0 means adaptive.
    pub library_columns_landscape: Pref<i32>,
}

impl BrowsePrefs {
    pub fn new(store: PrefStore) -> Self {
        Self {
            sources: SourcePrefs::new(store.clone()),
            migrate_flags: store.pref("migrate_flags", MIGRATE_FLAGS_DEFAULT),
            default_manga_category: store.pref("default_category", DEFAULT_CATEGORY_ASK),
            library_columns_portrait: store.pref("pref_library_columns_portrait_key", 0_i32),
            library_columns_landscape: store.pref("pref_library_columns_landscape_key", 0_i32),
        }
    }

    /// The stored display mode of the source grids. Unknown stored text reads as the default.
    pub fn display_mode(&self) -> LibraryDisplayMode {
        self.sources.source_display_mode.get()
    }

    pub fn set_display_mode(&self, mode: LibraryDisplayMode) -> Result<(), PrefError> {
        self.sources.source_display_mode.set(mode)
    }

    /// `browse_hide_in_library_items`: a manga browse page leaves the library's entries out of its listing. Read once,
    /// when the page is made.
    pub fn hide_in_library(&self) -> bool {
        self.sources.hide_in_manga_library_items.get()
    }

    /// A column count from the settings, never negative.
    pub fn columns(stored: i32) -> i32 {
        stored.max(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_prefs::source::{MigrationDirection, MigrationSortingMode, default_enabled_languages};
    use kuro_prefs::toml_backend::TomlBackend;
    use kuro_prefs::values::PrefValue;

    fn prefs() -> BrowsePrefs {
        BrowsePrefs::new(PrefStore::in_memory())
    }

    #[test]
    fn defaults_match_the_upstream_settings() {
        let p = prefs();
        assert_eq!(p.display_mode(), LibraryDisplayMode::CompactGrid);
        assert_eq!(p.migrate_flags.get(), i32::MAX);
        assert_eq!(p.sources.migration_sorting_mode.get(), MigrationSortingMode::ALPHABETICAL);
        assert_eq!(p.sources.migration_sorting_direction.get(), MigrationDirection::ASCENDING);
        assert!(p.sources.show_nsfw_source.get());
        assert!(p.sources.enabled_languages.get().contains("all"));
        assert_eq!(p.sources.last_used_manga_source.get(), -1);
        assert_eq!(p.default_manga_category.get(), -1);
        assert_eq!(p.library_columns_portrait.get(), 0);
        assert_eq!(p.library_columns_landscape.get(), 0);
        assert!(!p.sources.global_search_filter_state.get());
        assert!(!p.sources.hide_in_manga_library_items.get());
    }

    #[test]
    fn an_unknown_display_mode_reads_as_compact_grid() {
        let store = PrefStore::in_memory();
        store.set_raw("pref_display_mode_catalogue", PrefValue::Str("NOT_A_MODE".into()));
        let p = BrowsePrefs::new(store);
        assert_eq!(p.display_mode(), LibraryDisplayMode::CompactGrid);
    }

    #[test]
    fn a_stored_display_mode_round_trips() {
        let p = prefs();
        p.set_display_mode(LibraryDisplayMode::List).expect("set");
        assert_eq!(p.display_mode(), LibraryDisplayMode::List);
    }

    #[test]
    fn negative_column_counts_read_as_adaptive() {
        assert_eq!(BrowsePrefs::columns(-3), 0);
        assert_eq!(BrowsePrefs::columns(4), 4);
    }

    #[test]
    fn c_and_posix_locales_default_to_english() {
        for locale in ["C", "POSIX", "C.UTF-8"] {
            let langs = default_enabled_languages(locale);
            assert!(langs.contains("en"), "{locale}");
            assert!(!langs.contains("c") && !langs.contains("posix"), "{locale}");
        }
    }

    #[test]
    fn unknown_keys_in_the_settings_file_survive_a_save() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config = dir.path().join("config");
        let state = dir.path().join("state");
        std::fs::create_dir_all(&config).expect("config dir");
        std::fs::create_dir_all(&state).expect("state dir");
        std::fs::write(kuro_prefs::toml_backend::config_file(&config), "[sources]\nfuture_key = \"kept\"\n").expect("seed");
        {
            let backend = TomlBackend::open(&config, &state).expect("open");
            let store = PrefStore::new(Box::new(backend)).expect("store");
            let p = BrowsePrefs::new(store.clone());
            p.set_display_mode(LibraryDisplayMode::List).expect("set");
            store.flush().expect("flush");
        }
        let text = std::fs::read_to_string(kuro_prefs::toml_backend::config_file(&config)).expect("read");
        assert!(text.contains("future_key"), "unknown key dropped:\n{text}");
    }
}
