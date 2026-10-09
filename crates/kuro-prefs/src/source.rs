//! `[sources]` preferences (Aniyomi `SourcePreferences`, T-10a-017). The keys and defaults are the ones registered in
//! `registry`; the test below compares every field with its row. Commented-out SY keys (`eh_enable_source_blacklist`,
//! `sources_tab_*`) are not fields here.

use std::collections::BTreeSet;

use crate::library::LibraryDisplayMode;
use crate::pref_enum;
use crate::registry::define_prefs;

pref_enum! {
    /// `SetMigrateSorting.Mode`: how the migration source list is ordered.
    pub enum MigrationSortingMode { ALPHABETICAL, TOTAL }
}

pref_enum! {
    /// `SetMigrateSorting.Direction`.
    pub enum MigrationDirection { ASCENDING, DESCENDING }
}

pref_enum! {
    /// `SourcePreferences.DataSaver`: the image proxy a source's images go through.
    pub enum DataSaver { NONE, BANDWIDTH_HERO, WSRV_NL, RESMUSH_IT }
}

/// The preference namespace of one source: `source_<decimal id>` (T-16a-034, EC-PR-01).
pub fn source_namespace(source_id: i64) -> String {
    format!("source_{source_id}")
}

/// The stored key of one setting of a source: `<namespace>.<key>`. A read of an unset key returns the caller's default
/// and writes nothing; only a set writes a row.
pub fn source_setting_key(source_id: i64, key: &str) -> String {
    format!("{}.{key}", source_namespace(source_id))
}

define_prefs! {
    /// `[sources]` section: the keys of Aniyomi `SourcePreferences`, minus the derived `animeext_updates_count` (T-19a-064).
    pub struct SourcePrefs in "sources" {
        source_display_mode: LibraryDisplayMode = "pref_display_mode_catalogue" => LibraryDisplayMode::DEFAULT;
        enabled_languages: BTreeSet<String> = "source_languages" => default_enabled_languages(&system_locale());
        show_nsfw_source: bool = "show_nsfw_source" => true;
        /// Kurobayomi only (T-19a-066): shows the store entries that cannot run on Linux, with badges and no install.
        show_not_runnable: bool = "show_not_runnable" => false;
        migration_sorting_mode: MigrationSortingMode = "pref_migration_sorting" => MigrationSortingMode::ALPHABETICAL;
        migration_sorting_direction: MigrationDirection = "pref_migration_direction" => MigrationDirection::ASCENDING;
        anime_extension_repos: BTreeSet<String> = "anime_extension_repos" => BTreeSet::new();
        manga_extension_repos: BTreeSet<String> = "extension_repos" => BTreeSet::new();
        trusted_extensions: BTreeSet<String> = "__APP_STATE_trusted_extensions" => BTreeSet::new();
        global_search_filter_state: bool = "__APP_STATE_has_filters_toggle_state" => false;
        disabled_anime_sources: BTreeSet<String> = "hidden_anime_catalogues" => BTreeSet::new();
        disabled_manga_sources: BTreeSet<String> = "hidden_catalogues" => BTreeSet::new();
        incognito_anime_extensions: BTreeSet<String> = "incognito_anime_extensions" => BTreeSet::new();
        incognito_manga_extensions: BTreeSet<String> = "incognito_manga_extensions" => BTreeSet::new();
        pinned_anime_sources: BTreeSet<String> = "pinned_anime_catalogues" => BTreeSet::new();
        pinned_manga_sources: BTreeSet<String> = "pinned_catalogues" => BTreeSet::new();
        last_used_anime_source: i64 = "__APP_STATE_last_anime_catalogue_source" => -1;
        last_used_manga_source: i64 = "__APP_STATE_last_catalogue_source" => -1;
        manga_extension_updates_count: i32 = "ext_updates_count" => 0;
        hide_in_anime_library_items: bool = "browse_hide_in_anime_library_items" => false;
        hide_in_manga_library_items: bool = "browse_hide_in_library_items" => false;
        data_saver: DataSaver = "data_saver" => DataSaver::NONE;
        data_saver_ignore_jpeg: bool = "ignore_jpeg" => false;
        data_saver_ignore_gif: bool = "ignore_gif" => true;
        data_saver_image_quality: i32 = "data_saver_image_quality" => 80;
        data_saver_image_format_jpeg: bool = "data_saver_image_format_jpeg" => false;
        data_saver_server: String = "data_saver_server" => String::new();
        data_saver_color_bw: bool = "data_saver_color_bw" => false;
        data_saver_excluded_sources: BTreeSet<String> = "data_saver_excluded" => BTreeSet::new();
        data_saver_downloader: bool = "data_saver_downloader" => true;
    }
}

/// The locale the process runs in: `LC_MESSAGES`, then `LANG` (T-18a-023). Empty when neither is set.
pub fn system_locale() -> String {
    locale_from(|name| std::env::var(name).ok())
}

/// [`system_locale`] over any variable lookup.
fn locale_from(var: impl Fn(&str) -> Option<String>) -> String {
    ["LC_MESSAGES", "LANG"].iter().find_map(|name| var(name).filter(|v| !v.is_empty())).unwrap_or_default()
}

/// `LocaleHelper.getDefaultEnabledLanguages()`: `all`, `en` and the language subtag of `locale` in lower case. A locale
/// with no usable language gives `all` and `en` only.
pub fn default_enabled_languages(locale: &str) -> BTreeSet<String> {
    let mut languages = BTreeSet::from(["all".to_owned(), "en".to_owned()]);
    let subtag = locale.split(['.', '@', '_', '-']).next().unwrap_or_default().to_ascii_lowercase();
    let usable = (2..=3).contains(&subtag.len()) && subtag.chars().all(|c| c.is_ascii_lowercase());
    if usable && subtag != "c" && subtag != "posix" {
        languages.insert(subtag);
    }
    languages
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::REGISTRY;
    use crate::store::PrefStore;
    use crate::values::PrefValue;

    #[test]
    fn every_field_is_a_registered_sources_key_with_the_same_name() {
        assert_eq!(SourcePrefs::TABLE, "sources");
        assert_eq!(SourcePrefs::FIELDS.len(), 30, "the thirty keys of section 4.1, minus the derived animeext_updates_count, plus show_not_runnable");
        for (field, key) in SourcePrefs::FIELDS {
            let row = REGISTRY.iter().find(|m| m.key == *key).unwrap_or_else(|| panic!("{field}: {key} not registered"));
            assert_eq!(row.table, "sources", "{key}");
        }
    }

    #[test]
    fn the_locale_reads_lc_messages_before_lang_and_skips_empty_values() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |name: &str| pairs.iter().find(|(k, _)| *k == name).map(|(_, v)| (*v).to_owned())
        };
        assert_eq!(locale_from(env(&[("LANG", "de_DE.UTF-8"), ("LC_MESSAGES", "fr_FR.UTF-8")])), "fr_FR.UTF-8");
        assert_eq!(locale_from(env(&[("LANG", "de_DE.UTF-8"), ("LC_MESSAGES", "")])), "de_DE.UTF-8");
        assert_eq!(locale_from(env(&[("LC_ALL", "ja_JP.UTF-8")])), "", "LC_ALL is not read");
    }

    #[test]
    fn the_app_state_keys_carry_the_prefix() {
        let key = |field: &str| SourcePrefs::FIELDS.iter().find(|(f, _)| *f == field).expect("field").1;
        assert_eq!(key("trusted_extensions"), "__APP_STATE_trusted_extensions");
        assert_eq!(key("global_search_filter_state"), "__APP_STATE_has_filters_toggle_state");
        assert_eq!(key("last_used_anime_source"), "__APP_STATE_last_anime_catalogue_source");
        assert_eq!(key("last_used_manga_source"), "__APP_STATE_last_catalogue_source");
        assert!(!SourcePrefs::FIELDS.iter().any(|(_, k)| k.starts_with("eh_") || k.starts_with("sources_tab")));
    }

    #[test]
    fn defaults_match_the_registry_and_the_plan() {
        let store = PrefStore::in_memory();
        let prefs = SourcePrefs::new(store);
        assert_eq!(prefs.source_display_mode.get(), LibraryDisplayMode::CompactGrid);
        assert_eq!(prefs.migration_sorting_mode.get(), MigrationSortingMode::ALPHABETICAL);
        assert_eq!(prefs.migration_sorting_direction.get(), MigrationDirection::ASCENDING);
        assert_eq!(prefs.data_saver.get(), DataSaver::NONE);
        assert_eq!(prefs.last_used_manga_source.get(), -1);
        assert_eq!(prefs.data_saver_image_quality.get(), 80);
        assert!(prefs.show_nsfw_source.get());
        assert!(!prefs.show_not_runnable.get());
        assert!(!prefs.data_saver_ignore_jpeg.get());
        assert!(prefs.data_saver_ignore_gif.get());
        assert!(prefs.data_saver_downloader.get());
        assert!(prefs.disabled_manga_sources.get().is_empty());
        assert!(prefs.enabled_languages.get().contains("en"));
        assert!(prefs.enabled_languages.get().contains("all"));
    }

    #[test]
    fn enum_preferences_store_the_name_and_fall_back_on_garbage() {
        let store = PrefStore::in_memory();
        let prefs = SourcePrefs::new(store.clone());
        store.set_raw("pref_migration_sorting", PrefValue::Str("TOTAL".into()));
        assert_eq!(prefs.migration_sorting_mode.get(), MigrationSortingMode::TOTAL);
        store.set_raw("data_saver", PrefValue::Str("NOT_A_SAVER".into()));
        assert_eq!(prefs.data_saver.get(), DataSaver::NONE, "unknown names give the default");
        store.set_raw("pref_migration_direction", PrefValue::Str("sideways".into()));
        assert_eq!(prefs.migration_sorting_direction.get(), MigrationDirection::ASCENDING);
    }

    #[test]
    fn the_data_saver_enum_has_exactly_four_names_and_stores_the_name() {
        let names: Vec<&str> = DataSaver::ALL.iter().map(|saver| match saver {
            DataSaver::NONE => "NONE",
            DataSaver::BANDWIDTH_HERO => "BANDWIDTH_HERO",
            DataSaver::WSRV_NL => "WSRV_NL",
            DataSaver::RESMUSH_IT => "RESMUSH_IT",
        }).collect();
        assert_eq!(names, ["NONE", "BANDWIDTH_HERO", "WSRV_NL", "RESMUSH_IT"], "T-17a-022");
        let store = PrefStore::in_memory();
        let prefs = SourcePrefs::new(store.clone());
        prefs.data_saver.set(DataSaver::WSRV_NL).unwrap();
        assert_eq!(store.get_raw("data_saver"), Some(crate::values::PrefValue::Str("WSRV_NL".into())), "stored by name");
        assert_eq!(SourcePrefs::new(store).data_saver.get(), DataSaver::WSRV_NL);
    }

    #[test]
    fn a_set_of_source_ids_round_trips_through_its_decimal_strings() {
        let store = PrefStore::in_memory();
        let prefs = SourcePrefs::new(store.clone());
        let ids: BTreeSet<String> = ["4294967296", "7"].iter().map(|s| (*s).to_owned()).collect();
        prefs.disabled_manga_sources.set(ids.clone()).unwrap();
        assert_eq!(prefs.disabled_manga_sources.get(), ids, "T-17a-015: {{4294967296, 7}} reads back equal");
        let reopened = SourcePrefs::new(store);
        assert_eq!(reopened.disabled_manga_sources.get(), ids, "and survives a new handle on the same store");
    }

    #[test]
    fn every_default_of_the_thirty_keys_holds_on_an_empty_store() {
        let prefs = SourcePrefs::new(PrefStore::in_memory());
        assert!(prefs.trusted_extensions.get().is_empty());
        assert!(!prefs.global_search_filter_state.get());
        assert_eq!(prefs.last_used_anime_source.get(), -1);
        assert_eq!(prefs.manga_extension_updates_count.get(), 0);
        assert!(!prefs.hide_in_anime_library_items.get());
        assert!(!prefs.hide_in_manga_library_items.get());
        assert!(!prefs.data_saver_color_bw.get());
        assert!(!prefs.data_saver_image_format_jpeg.get());
        assert_eq!(prefs.data_saver_server.get(), "");
        assert!(prefs.data_saver_excluded_sources.get().is_empty());
        assert!(prefs.pinned_manga_sources.get().is_empty() && prefs.pinned_anime_sources.get().is_empty());
        assert!(prefs.incognito_manga_extensions.get().is_empty() && prefs.incognito_anime_extensions.get().is_empty());
        assert!(prefs.disabled_anime_sources.get().is_empty());
        assert!(prefs.manga_extension_repos.get().is_empty() && prefs.anime_extension_repos.get().is_empty());
    }

    #[test]
    fn the_default_languages_apply_only_while_the_key_is_unset() {
        let store = PrefStore::in_memory();
        let prefs = SourcePrefs::new(store.clone());
        let stored: BTreeSet<String> = ["fr".to_owned()].into_iter().collect();
        prefs.enabled_languages.set(stored.clone()).unwrap();
        let reopened = SourcePrefs::new(store);
        assert_eq!(reopened.enabled_languages.get(), stored, "a stored value wins over the system default");
    }

    #[test]
    fn default_languages_add_the_system_subtag_and_an_unusable_locale_gives_en() {
        let set = |items: &[&str]| items.iter().map(|s| (*s).to_owned()).collect::<BTreeSet<_>>();
        assert_eq!(default_enabled_languages("pt_BR.UTF-8"), set(&["all", "en", "pt"]));
        assert_eq!(default_enabled_languages("ja_JP.UTF-8"), set(&["all", "en", "ja"]));
        assert_eq!(default_enabled_languages("pt_BR@euro"), set(&["all", "en", "pt"]));
        assert_eq!(default_enabled_languages("zh-Hant_TW"), set(&["all", "en", "zh"]));
        assert_eq!(default_enabled_languages("JA"), set(&["all", "en", "ja"]));
        assert_eq!(default_enabled_languages("C"), set(&["all", "en"]));
        assert_eq!(default_enabled_languages("POSIX"), set(&["all", "en"]));
        assert_eq!(default_enabled_languages(""), set(&["all", "en"]));
        assert_eq!(default_enabled_languages("12345"), set(&["all", "en"]));
    }
}

#[cfg(test)]
mod namespace_tests {
    use super::*;
    use crate::store::PrefStore;
    use crate::values::PrefValue;

    #[test]
    fn the_namespace_is_the_source_id_in_decimal() {
        assert_eq!(source_namespace(4_851_548_778_462_321_757), "source_4851548778462321757");
        assert_eq!(source_setting_key(7, "nsfw"), "source_7.nsfw");
    }

    #[test]
    fn a_read_of_an_unset_source_setting_gives_the_default_and_writes_no_row() {
        let store = PrefStore::in_memory();
        let pref = store.pref(source_setting_key(9, "quality"), false);
        assert!(!pref.get());
        assert!(store.all().is_empty(), "a read never creates a row");
        assert_eq!(store.get_raw(&source_setting_key(9, "quality")), None);
    }

    #[test]
    fn a_written_source_setting_persists_under_its_namespace() {
        let store = PrefStore::in_memory();
        let pref = store.pref(source_setting_key(9, "quality"), false);
        pref.set(true).unwrap();
        assert!(pref.get());
        assert_eq!(store.get_raw("source_9.quality"), Some(PrefValue::Bool(true)));
        assert_eq!(store.get_raw("source_10.quality"), None, "another source's namespace is untouched");
    }
}

