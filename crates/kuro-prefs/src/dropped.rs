//! Android preference plumbing this crate does not port, and the keys kept for backup parity only.
//!
//! * Source preference binders (T-02a-031). `SharedPreferencesDataStore`, the AndroidX bridge, has no Rust type. Extension
//!   settings screens read and write through [`crate::source::SourcePrefs`] and the shared binders. The replacement
//!   mapping: `getBoolean`/`putBoolean` to `Kind::Bool`, `getInt` to `Kind::Int`, `getLong` to `Kind::Long`,
//!   `getFloat` to `Kind::Float`, `getString` to `Kind::Str`, and `getStringSet` to `Kind::Set`. The user loses nothing.
//! * Backup extension preferences (T-02a-028). The `BackupExtensionPreferences` message and the duplicate
//!   `full.models` copies are not in the Rust definitions. Aniyomi never used them, so nothing is lost. An import
//!   that carries unknown fields keeps them as extras and does not fail.
//! * Android-only keys (T-02a-032, T-01a-044). The keys in `registry` that only the Android UI used stay registered
//!   so backups round-trip with their kind and value. The UI does not show them. `extension_installer` is the one
//!   exception: it is not registered (see the `DROPPED_KEYS` test in `registry`), so an import keeps it as an
//!   extra and nothing reads it. `pref_hardware_bitmap_threshold` is dropped the same way (T-13a-030).
//! * External downloader choice (T-09a-022). `use_external_downloader` and `external_downloader_selection` are read for
//!   backup parity only. The UI does not offer the option. The loss is third-party Android download apps.
//!
//! The round-trip test below keeps the list of registered Android-only keys true.

#[cfg(test)]
mod tests {
    use crate::export::export_app;
    use crate::keys::Namespace;
    use crate::registry::{Kind, REGISTRY};
    use crate::store::PrefStore;
    use crate::values::PrefValue;

    /// The keys that Android used and the desktop hides (T-02a-032). Each one is either registered or, for the two
    /// listed in `DROPPED`, deliberately absent.
    const ANDROID_ONLY: [&str; 19] = [
        "extension_installer",
        "pref_display_profile_key",
        "pref_hardware_bitmap_threshold",
        "pref_always_decode_long_strip_with_ssiv",
        "tablet_ui_mode",
        "pref_default_player_orientation_type_key",
        "pref_show_system_status_bar",
        "pref_enable_pip",
        "pref_pip_episode_toasts",
        "pref_pip_on_exit",
        "pip_replace_with_previous",
        "cutout_short",
        "reader_volume_keys",
        "reader_volume_keys_inverted",
        "pref_default_orientation_type_key",
        "secure_screen_v2",
        "backup_flags",
        "use_external_downloader",
        "external_downloader_selection",
    ];

    /// Listed above but not registered: the Android installer choice and the hardware bitmap threshold.
    const DROPPED: [&str; 2] = ["extension_installer", "pref_hardware_bitmap_threshold"];

    /// A value of the registry kind, different from the default, so a lost value cannot pass as a default.
    fn sample(kind: Kind) -> PrefValue {
        match kind {
            Kind::Bool => PrefValue::Bool(true),
            Kind::Int => PrefValue::Int(7),
            Kind::Long => PrefValue::Long(7),
            Kind::Float => PrefValue::Float(0.5),
            Kind::Str => PrefValue::Str("kept".into()),
            Kind::Set => PrefValue::Set(["kept".to_owned()].into()),
        }
    }

    #[test]
    fn every_android_only_key_round_trips_with_its_kind_and_value_or_is_dropped() {
        let store = PrefStore::in_memory();
        let mut round_tripped = 0;
        for key in ANDROID_ONLY {
            match REGISTRY.iter().find(|meta| meta.key == key) {
                Some(meta) => {
                    assert_eq!(meta.ns, Namespace::Normal, "{key} is a normal key");
                    let value = sample(meta.kind);
                    store.set_raw(key, value.clone());
                    let exported = export_app(&store, false);
                    assert!(exported.contains(&(key.to_owned(), value)), "{key} lost its value");
                    round_tripped += 1;
                }
                None => assert!(DROPPED.contains(&key), "{key} is neither registered nor dropped"),
            }
        }
        assert_eq!(round_tripped, ANDROID_ONLY.len() - DROPPED.len());
    }

    #[test]
    fn the_dropped_keys_are_not_registered() {
        for key in DROPPED {
            assert!(REGISTRY.iter().all(|meta| meta.key != key), "{key} must not be registered");
        }
    }
}
