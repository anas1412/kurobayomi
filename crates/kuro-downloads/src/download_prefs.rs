//! Typed download preferences (T-25a-001): one accessor per kept Kotlin `DownloadPreferences` key, each returning a
//! [`Pref`] on the area 02 store. Nothing is cached here: every `get` reads the store, every `set` writes through it,
//! and `watch` reports changes made anywhere.
//!
//! The settings diet kept four download keys (docs/design/settings-diet.md, Downloads page). The other download keys
//! are cut: the engine uses their hard-coded defaults (see the constants in the modules that read them) and the keys
//! are in `kuro_prefs::registry::DROPPED_KEYS`.
//!
//! The key strings and defaults are the ones of [`crate::settings::PREFERENCE_KEYS`]; the tests tie them together.

use kuro_prefs::pref::Pref;
use kuro_prefs::store::PrefStore;

/// The exact stored key of each kept preference (data section 3 of area 25, Kotlin `DownloadPreferences.kt`).
pub mod keys {
    pub const DOWNLOAD_NEW: &str = "download_new";
    pub const DOWNLOAD_NEW_EPISODE: &str = "download_new_episode";
    pub const DOWNLOAD_SLOTS: &str = "download_slots";
    pub const REMOVE_AFTER_READ_SLOTS: &str = "remove_after_read_slots";
}

/// Handle to the download preferences of one store. Cheap to clone.
#[derive(Clone)]
pub struct DownloadPrefs {
    store: PrefStore,
}

impl DownloadPrefs {
    pub fn new(store: PrefStore) -> Self {
        Self { store }
    }

    pub fn remove_after_read_slots(&self) -> Pref<i32> {
        self.store.pref(keys::REMOVE_AFTER_READ_SLOTS, -1)
    }

    pub fn download_new_chapters(&self) -> Pref<bool> {
        self.store.pref(keys::DOWNLOAD_NEW, false)
    }

    pub fn download_new_episodes(&self) -> Pref<bool> {
        self.store.pref(keys::DOWNLOAD_NEW_EPISODE, false)
    }

    /// Stored and written by the settings screen, read by nothing (E-12, [`crate::settings::DOWNLOAD_SLOTS_HIDDEN`]).
    pub fn number_of_downloads(&self) -> Pref<i32> {
        self.store.pref(keys::DOWNLOAD_SLOTS, 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::default_of;

    fn prefs() -> DownloadPrefs {
        DownloadPrefs::new(PrefStore::in_memory())
    }

    #[test]
    fn every_kept_accessor_uses_the_exact_key_and_default_of_the_spec_table() {
        let p = prefs();
        assert_eq!(default_of(keys::DOWNLOAD_NEW), Some(if p.download_new_chapters().default_value() { "true" } else { "false" }));
        assert_eq!(default_of(keys::DOWNLOAD_NEW_EPISODE), Some(if p.download_new_episodes().default_value() { "true" } else { "false" }));
        assert_eq!(default_of(keys::REMOVE_AFTER_READ_SLOTS), Some(p.remove_after_read_slots().default_value().to_string().as_str()));
        assert_eq!(default_of(keys::DOWNLOAD_SLOTS), Some(p.number_of_downloads().default_value().to_string().as_str()));
    }

    #[test]
    fn the_kept_keys_are_the_four_of_the_downloads_page() {
        assert_eq!(crate::settings::PREFERENCE_KEYS.len(), 4);
        for (key, _) in crate::settings::PREFERENCE_KEYS {
            assert!([keys::DOWNLOAD_NEW, keys::DOWNLOAD_NEW_EPISODE, keys::DOWNLOAD_SLOTS, keys::REMOVE_AFTER_READ_SLOTS].contains(&key));
        }
    }

    #[test]
    fn a_set_is_visible_to_a_watcher_and_a_read_sees_it() {
        let p = prefs();
        let slots = p.remove_after_read_slots();
        let watch = slots.watch();
        slots.set(3).unwrap();
        assert_eq!(p.remove_after_read_slots().get(), 3, "reads go to the store, nothing is cached");
        assert_eq!(watch.latest(), 3, "a watcher sees the write");
    }
}
