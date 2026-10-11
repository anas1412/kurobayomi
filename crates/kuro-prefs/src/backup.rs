//! `[backup]` and `[storage]` preferences (Aniyomi `BackupPreferences`, `StoragePreferences`, T-02a-024).
//!
//! `backup_flags` is cut (settings diet; it is in `DROPPED_KEYS`). The flag strings exist so an import can tell which
//! parts an Aniyomi backup preference named.

use crate::pref::Watch;
use crate::registry::define_prefs;

pub const FLAG_CATEGORIES: &str = "1";
pub const FLAG_CHAPTERS: &str = "2";
pub const FLAG_HISTORY: &str = "4";
pub const FLAG_TRACK: &str = "8";
pub const FLAG_SETTINGS: &str = "10";
pub const FLAG_EXT_SETTINGS: &str = "20";
pub const FLAG_EXTENSIONS: &str = "40";

define_prefs! {
    /// `[backup]` section: the automatic backup interval in hours and the app-state time of the last automatic run.
    pub struct BackupPrefs in "backup" {
        backup_interval: i32 = "backup_interval" => 12;
        last_auto_backup_timestamp: i64 = "__APP_STATE_last_auto_backup_timestamp" => 0;
    }
}

impl BackupPrefs {
    /// A subscription to `backup_interval`, for the scheduler: poll [`Watch::try_changed`] from the main loop.
    pub fn subscribe(&self) -> Watch<i32> {
        self.backup_interval.watch()
    }
}

define_prefs! {
    /// `[storage]` section: the app-state base storage folder, empty until the user picks one. The default folder
    /// comes from `kuro_platform::paths::Paths`.
    pub struct StoragePrefs in "storage" {
        base_storage_directory: String = "__APP_STATE_storage_dir" => String::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::PrefStore;
    use crate::testutil::assert_section_registered;

    #[test]
    fn keys_and_defaults_are_the_plan_ones() {
        let backup = BackupPrefs::new(PrefStore::in_memory());
        assert_eq!(backup.backup_interval.get(), 12);
        assert_eq!(backup.last_auto_backup_timestamp.get(), 0);
        assert!(StoragePrefs::new(PrefStore::in_memory()).base_storage_directory.get().is_empty());
    }

    #[test]
    fn every_field_is_registered_in_its_table() {
        let backup = BackupPrefs::new(PrefStore::in_memory());
        assert_section_registered(BackupPrefs::TABLE, BackupPrefs::FIELDS, &backup.defaults());
        let storage = StoragePrefs::new(PrefStore::in_memory());
        assert_section_registered(StoragePrefs::TABLE, StoragePrefs::FIELDS, &storage.defaults());
    }

    #[test]
    fn a_change_of_the_interval_reaches_a_subscriber_once() {
        let backup = BackupPrefs::new(PrefStore::in_memory());
        let watch = backup.subscribe();
        assert_eq!(watch.try_changed(), None);
        backup.backup_interval.set(24).unwrap();
        assert_eq!(watch.try_changed(), Some(24));
        assert_eq!(watch.try_changed(), None);
    }

    #[test]
    fn the_flag_strings_are_the_aniyomi_hex_digits() {
        assert_eq!([FLAG_CATEGORIES, FLAG_CHAPTERS, FLAG_HISTORY, FLAG_TRACK, FLAG_SETTINGS, FLAG_EXT_SETTINGS, FLAG_EXTENSIONS], ["1", "2", "4", "8", "10", "20", "40"]);
    }
}
