//! Preference bindings of area 42 (T-42a-010). The keys and defaults live in the `kuro-prefs` and `kuro-downloads`
//! structs; this test pins them to the area table, read from an empty store. The one key of this area that no shared
//! struct owns, the Upcoming week start (T-42b-045), is bound here.

use kuro_prefs::pref::Pref;
use kuro_prefs::store::PrefStore;

/// The key of the Upcoming week start: `auto` (the locale decides), `monday` or `sunday`.
pub const WEEK_START_KEY: &str = "upcoming.week_start";
/// The value that lets the locale decide.
pub const WEEK_START_AUTO: &str = "auto";
/// The key of the opt-in Home key (T-42b-046): when on, Home returns the Upcoming calendar to the current month.
pub const HOME_RETURNS_TODAY_KEY: &str = "upcoming.home_returns_today";

/// `upcoming.week_start`, default `auto`.
pub fn week_start(store: &PrefStore) -> Pref<String> {
    store.pref(WEEK_START_KEY, WEEK_START_AUTO.to_owned())
}

/// `upcoming.home_returns_today`, default off: Aniyomi has no Today button, so the key is not taken unless set.
pub fn home_returns_today(store: &PrefStore) -> Pref<bool> {
    store.pref(HOME_RETURNS_TODAY_KEY, false)
}

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
    fn prefs_week_start_defaults_to_auto_and_feeds_the_calendar() {
        use chrono::Weekday;
        use super::week_start;
        use crate::updates::upcoming::calendar::week_start_for;
        let store = PrefStore::in_memory();
        let pref = week_start(&store);
        assert_eq!(pref.key(), "upcoming.week_start");
        assert_eq!(pref.get(), "auto");
        assert_eq!(week_start_for(&pref.get(), "en_US"), Weekday::Sun, "auto follows the locale");
        pref.set("monday".to_owned()).expect("the memory store accepts writes");
        assert_eq!(week_start_for(&pref.get(), "en_US"), Weekday::Mon, "the setting overrides the locale");
    }

    #[test]
    fn prefs_home_key_is_off_by_default_and_follows_the_setting() {
        use super::home_returns_today;
        let store = PrefStore::in_memory();
        let pref = home_returns_today(&store);
        assert_eq!(pref.key(), "upcoming.home_returns_today");
        assert!(!pref.get(), "off by default, so parity holds");
        pref.set(true).expect("the memory store accepts writes");
        assert!(home_returns_today(&store).get());
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
