//! `LibraryDisplayMode` (`LibraryDisplayMode.kt`). The enum and its stored codec live in `kuro-prefs`, where the
//! typed `pref_display_mode_library` key decodes them. One global key serves both kinds.

use kuro_prefs::library::LibraryPrefs;

pub use kuro_prefs::library::LibraryDisplayMode;

/// The display mode stored under `pref_display_mode_library`. Unknown stored text reads as the default.
pub fn read_display_mode(prefs: &LibraryPrefs) -> LibraryDisplayMode {
    prefs.display_mode.get()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_prefs::store::PrefStore;
    use kuro_prefs::values::PrefValue;

    #[test]
    fn order_and_default_match_the_menu() {
        assert_eq!(
            LibraryDisplayMode::ALL,
            [
                LibraryDisplayMode::CompactGrid,
                LibraryDisplayMode::ComfortableGrid,
                LibraryDisplayMode::List,
                LibraryDisplayMode::CoverOnlyGrid,
            ]
        );
        assert_eq!(LibraryDisplayMode::DEFAULT, LibraryDisplayMode::CompactGrid);
    }

    #[test]
    fn every_mode_round_trips_and_bad_input_reads_as_compact() {
        for mode in LibraryDisplayMode::ALL {
            assert_eq!(LibraryDisplayMode::from_serialized(mode.serialize()), Some(mode));
        }
        for bad in ["", "list", "compact_grid", "GRID", "  LIST"] {
            assert_eq!(LibraryDisplayMode::from_serialized(bad), None, "{bad:?}");
            let prefs = LibraryPrefs::new(PrefStore::in_memory());
            prefs.store().set_raw("pref_display_mode_library", PrefValue::Str(bad.into()));
            assert_eq!(read_display_mode(&prefs), LibraryDisplayMode::CompactGrid, "{bad:?}");
        }
    }

    #[test]
    fn the_stored_key_round_trips_through_the_prefs_store() {
        let prefs = LibraryPrefs::new(PrefStore::in_memory());
        assert_eq!(read_display_mode(&prefs), LibraryDisplayMode::CompactGrid, "registry default");
        for mode in LibraryDisplayMode::ALL {
            crate::category::service::set_display_mode(&prefs, mode);
            assert_eq!(read_display_mode(&prefs), mode);
        }
        let _ = prefs.display_mode.set(LibraryDisplayMode::CompactGrid);
        prefs.store().set_raw("pref_display_mode_library", PrefValue::Str("list".into()));
        assert_eq!(read_display_mode(&prefs), LibraryDisplayMode::CompactGrid, "lower case reads as compact");
        assert_eq!(prefs.store().get_raw("pref_display_mode_library"), Some(PrefValue::Str("list".into())));
    }
}
