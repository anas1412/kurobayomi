//! Preference bindings of area 42 (T-42a-010). The keys and defaults live in the `kuro-prefs` and `kuro-downloads`
//! structs; this test pins them to the area table, read from an empty store. The Upcoming week start and the Home key
//! are cut from the settings and hard-coded (see `misc::diet_defaults`).

#[cfg(test)]
mod tests {
    use kuro_prefs::library::LibraryPrefs;
    use kuro_prefs::player::PlayerPrefs;
    use kuro_prefs::store::PrefStore;

    #[test]
    fn prefs_defaults_match_the_area_table() {
        let store = PrefStore::in_memory();
        let library = LibraryPrefs::new(store.clone());
        assert_eq!(library.last_updated_timestamp.get(), 0);
        assert_eq!(library.new_manga_updates_count.get(), 0);
        assert_eq!(library.new_anime_updates_count.get(), 0);
        assert_eq!(library.default_manga_category.get(), -1);
        assert_eq!(library.default_anime_category.get(), -1);
        assert_eq!(library.new_manga_updates_count.key(), "library_unread_updates_count");
        assert_eq!(library.new_anime_updates_count.key(), "library_unseen_updates_count");
        assert_eq!(library.default_manga_category.key(), "default_category");
        assert!(!PlayerPrefs::new(store).always_use_external_player.get());
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
