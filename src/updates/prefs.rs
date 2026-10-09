//! Preference bindings of area 42 (T-42a-010). The keys and defaults live in the `kuro-prefs` and `kuro-downloads`
//! structs; this test pins them to the area table, read from an empty store.

#[cfg(test)]
mod tests {
    use kuro_downloads::download_prefs::DownloadPrefs;
    use kuro_prefs::library::LibraryPrefs;
    use kuro_prefs::player::PlayerPrefs;
    use kuro_prefs::store::PrefStore;
    use kuro_prefs::ui::{NavStyle, UiPrefs};

    #[test]
    fn prefs_defaults_match_the_area_table() {
        let store = PrefStore::in_memory();
        let library = LibraryPrefs::new(store.clone());
        assert_eq!(library.last_updated_timestamp.get(), 0);
        assert_eq!(library.new_manga_updates_count.get(), 0);
        assert_eq!(library.new_anime_updates_count.get(), 0);
        assert!(!library.hide_hidden_categories_settings.get());
        assert_eq!(library.default_manga_category.get(), -1);
        assert_eq!(library.default_anime_category.get(), -1);
        assert_eq!(library.new_manga_updates_count.key(), "library_unread_updates_count");
        assert_eq!(library.new_anime_updates_count.key(), "library_unseen_updates_count");
        assert_eq!(library.default_manga_category.key(), "default_category");

        let ui = UiPrefs::new(store.clone());
        assert!(ui.relative_time.get());
        assert_eq!(ui.date_format.get(), "");
        assert_eq!(ui.nav_style.get(), NavStyle::MOVE_HISTORY_TO_MORE);

        assert!(!PlayerPrefs::new(store.clone()).always_use_external_player.get());
        assert!(!DownloadPrefs::new(store).use_external_downloader().get());
    }

    #[test]
    fn prefs_counts_and_timestamp_are_watchable() {
        let store = PrefStore::in_memory();
        let library = LibraryPrefs::new(store.clone());
        let watch = library.new_manga_updates_count.watch();
        assert_eq!(watch.latest(), 0);
        library.new_manga_updates_count.set(3).expect("the memory store accepts writes");
        assert_eq!(watch.latest(), 3);
    }
}
