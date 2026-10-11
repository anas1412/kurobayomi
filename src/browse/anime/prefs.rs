//! The typed preferences of the anime browse scope (T-41b-043, T-41b-017). Each accessor reads one stored key with the
//! default of `p2-data-and-config.md` section 3. The keys live in `kuro-prefs` (`[sources]`, `[library]`, `[base]`); this
//! type only names the ones the anime browse reads, so the manga keys stay out of the anime grids. The last-used source,
//! the per-extension incognito, the hide-in-library switch and the migrate flags are cut and hard-coded.

use std::rc::Rc;

use kuro_prefs::base::BasePrefs;
use kuro_prefs::library::LibraryPrefs;
use kuro_prefs::source::SourcePrefs;
use kuro_prefs::store::PrefStore;

use crate::browse::prefs::BrowsePrefs;
use crate::misc::diet_defaults as diet;

/// The anime browse keys, each typed. Built once per store; the reads happen when a method is called.
pub struct AnimeBrowsePrefs {
    pub sources: SourcePrefs,
    pub library: LibraryPrefs,
    pub base: BasePrefs,
    store: PrefStore,
}

impl AnimeBrowsePrefs {
    pub fn new(store: PrefStore) -> Self {
        Self { sources: SourcePrefs::new(store.clone()), library: LibraryPrefs::new(store.clone()), base: BasePrefs::new(store.clone()), store }
    }

    /// The preferences the shared browse page reads, with the anime keys where the manga page reads its own: the anime
    /// landscape column count and the anime default category.
    pub fn browse_prefs(&self) -> Rc<BrowsePrefs> {
        let mut prefs = BrowsePrefs::new(self.store.clone());
        prefs.default_manga_category = self.library.default_anime_category.clone();
        prefs.library_columns_landscape = self.library.anime_landscape_columns.clone();
        Rc::new(prefs)
    }

    /// `browse_hide_in_anime_library_items`: cut from the settings, so nothing is hidden.
    pub fn hide_in_library(&self) -> bool {
        diet::BROWSE_HIDE_IN_LIBRARY
    }

    #[cfg(test)]
    /// The grid columns for a window `width` wide: the landscape key from 720 px, otherwise the automatic portrait value.
    /// Zero or less is adaptive (E-BR-23).
    pub fn columns(&self, width: i32) -> GridColumns {
        grid_columns(width, diet::LIBRARY_PORTRAIT_COLUMNS, self.library.anime_landscape_columns.get())
    }

    #[cfg(test)]
    /// `default_anime_category`, default -1 (ask every time). The system category 0 is never offered.
    pub fn default_category(&self) -> i64 {
        i64::from(self.library.default_anime_category.get())
    }
}

#[cfg(test)]
use crate::browse::grid::{GridColumns, grid_columns};

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_prefs::library::LibraryDisplayMode;

    fn prefs() -> AnimeBrowsePrefs {
        AnimeBrowsePrefs::new(PrefStore::in_memory())
    }

    #[test]
    fn the_defaults_match_the_listed_ones() {
        let p = prefs();
        assert_eq!(p.columns(800), GridColumns::Adaptive { cell_px: 128 }, "0 is adaptive");
        assert_eq!(p.default_category(), -1);
        assert!(!p.hide_in_library());
        assert!(p.sources.show_nsfw_source.get());
    }

    #[test]
    fn the_anime_landscape_key_picks_the_grid_and_the_manga_keys_do_not() {
        let store = PrefStore::in_memory();
        let p = AnimeBrowsePrefs::new(store.clone());
        p.library.anime_landscape_columns.set(3).expect("set");
        p.library.manga_landscape_columns.set(9).expect("set");
        assert_eq!(p.columns(800), GridColumns::Fixed(3), "a wide window reads the landscape key");
        assert_eq!(p.columns(600), GridColumns::Adaptive { cell_px: 128 }, "the portrait value is automatic");
        assert_eq!(store.get_raw("pref_animelib_columns_landscape_key"), Some(kuro_prefs::values::PrefValue::Int(3)));
    }

    #[test]
    fn the_browse_prefs_read_the_anime_keys_for_the_shared_page() {
        let p = prefs();
        p.library.anime_landscape_columns.set(4).expect("set");
        p.library.default_anime_category.set(7).expect("set");
        let browse = p.browse_prefs();
        assert_eq!(browse.library_columns_landscape.get(), 4);
        assert_eq!(browse.default_manga_category.get(), 7);
        assert_eq!(browse.display_mode(), LibraryDisplayMode::CompactGrid);
    }

    #[test]
    fn the_default_category_reads_its_stored_value() {
        let p = prefs();
        p.library.default_anime_category.set(4).expect("set");
        assert_eq!(p.default_category(), 4);
    }
}
