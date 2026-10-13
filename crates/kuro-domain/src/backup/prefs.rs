//! Backup preferences (`BackupPreferences.kt`, T-09a-052): the automatic-backup interval, the time of the last automatic
//! backup, and the flags. Keys and defaults are the ones registered in `kuro_prefs::registry`.

use kuro_prefs::pref::Pref;
use kuro_prefs::store::PrefStore;

use super::flags::BackupFlags;

/// Hours between automatic backups when the preference is absent. The enable switch sets this too.
pub const DEFAULT_INTERVAL_HOURS: i32 = 12;

/// Typed handles for the backup preferences.
#[derive(Clone)]
pub struct BackupPrefs {
    /// `backup_interval`. `0` means automatic backup is off.
    pub interval: Pref<i32>,
    /// `__APP_STATE_last_auto_backup_timestamp`, in epoch milliseconds. `0` means never.
    pub last_auto_backup: Pref<i64>,
    /// `backup_flags`. Kept for format parity; see the module note in `flags`.
    pub flags: Pref<BackupFlags>,
}

impl BackupPrefs {
    pub fn new(store: &PrefStore) -> Self {
        Self {
            interval: store.pref("backup_interval", DEFAULT_INTERVAL_HOURS),
            last_auto_backup: store.pref("__APP_STATE_last_auto_backup_timestamp", 0i64),
            flags: store.pref("backup_flags", BackupFlags::default()),
        }
    }
}

/// The interval in hours, or `None` when automatic backup is off. `0` and negative values are off (E-BK-01, E-BK-02),
/// so a zero never reaches a timer as "every zero hours".
pub fn interval_hours(raw: i32) -> Option<u32> {
    u32::try_from(raw).ok().filter(|hours| *hours > 0)
}

/// `EnableAutoBackupMigration`: switching automatic backup on turns a zero interval into 12 hours.
pub fn enable_auto_backup(prefs: &BackupPrefs) {
    if prefs.interval.get() == 0 {
        let _ = prefs.interval.set(DEFAULT_INTERVAL_HOURS);
    }
}

/// Milliseconds since the last automatic backup. A last time in the future (a clock change, E-BK-04) gives `0`.
pub fn elapsed_ms(now_ms: i64, last_ms: i64) -> i64 {
    now_ms.saturating_sub(last_ms).max(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interval_defaults_to_twelve_and_zero_or_negative_is_off() {
        let store = PrefStore::in_memory();
        let prefs = BackupPrefs::new(&store);
        assert_eq!(prefs.interval.get(), 12, "TC-BK-01 default");
        assert_eq!(interval_hours(prefs.interval.get()), Some(12));
        assert_eq!(interval_hours(0), None, "TC-BK-01: zero is disabled");
        assert_eq!(interval_hours(-3), None, "E-BK-02: negative is disabled");
        assert_eq!(interval_hours(24), Some(24));
    }

    #[test]
    fn enabling_turns_a_zero_interval_into_twelve_and_keeps_others() {
        let store = PrefStore::in_memory();
        let prefs = BackupPrefs::new(&store);
        let _ = prefs.interval.set(0);
        enable_auto_backup(&prefs);
        assert_eq!(prefs.interval.get(), 12);
        let _ = prefs.interval.set(6);
        enable_auto_backup(&prefs);
        assert_eq!(prefs.interval.get(), 6);
    }

    #[test]
    fn last_backup_time_defaults_to_zero_and_stores_epoch_millis() {
        let store = PrefStore::in_memory();
        let prefs = BackupPrefs::new(&store);
        assert_eq!(prefs.last_auto_backup.get(), 0);
        let _ = prefs.last_auto_backup.set(1_760_000_000_123);
        assert_eq!(prefs.last_auto_backup.get(), 1_760_000_000_123);
        assert_eq!(elapsed_ms(1_000, 400), 600);
        assert_eq!(elapsed_ms(400, 1_000), 0, "E-BK-04: a future last time gives zero");
    }

    #[test]
    fn flags_default_to_the_four_upstream_tags() {
        let store = PrefStore::in_memory();
        let prefs = BackupPrefs::new(&store);
        assert_eq!(prefs.flags.get(), BackupFlags::default());
        assert_eq!(prefs.flags.key(), "backup_flags");
    }
}
