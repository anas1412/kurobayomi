//! The typed preferences of the anime browse scope (T-41b-043, T-41b-017). Each accessor reads one stored key with the
//! default of `p2-data-and-config.md` section 3. The keys live in `kuro-prefs` (`[sources]`, `[library]`, `[base]`); this
//! type only names the ones the anime browse reads, so the manga keys stay out of the anime grids.

use std::collections::BTreeSet;
use std::rc::Rc;

use kuro_prefs::PrefError;
use kuro_prefs::base::BasePrefs;
use kuro_prefs::library::{LibraryDisplayMode, LibraryPrefs};
use kuro_prefs::source::SourcePrefs;
use kuro_prefs::store::PrefStore;
use kuro_prefs::values::PrefValue;

use crate::browse::grid::{GridColumns, grid_columns};
use crate::browse::nav::last_used_write;
use crate::browse::prefs::{BrowsePrefs, MIGRATE_FLAGS_DEFAULT};

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
    /// column counts and the anime default category. The display-mode key is the one both kinds share.
    pub fn browse_prefs(&self) -> Rc<BrowsePrefs> {
        let mut prefs = BrowsePrefs::new(self.store.clone());
        prefs.default_manga_category = self.library.default_anime_category.clone();
        prefs.library_columns_portrait = self.library.anime_portrait_columns.clone();
        prefs.library_columns_landscape = self.library.anime_landscape_columns.clone();
        Rc::new(prefs)
    }

    /// `pref_display_mode_catalogue`, default CompactGrid. Unknown stored text reads as the default (E-BR-22).
    pub fn display_mode(&self) -> LibraryDisplayMode {
        self.sources.source_display_mode.get()
    }

    /// Writes the global display mode. The grid re-renders from the same listing; nothing is queried again.
    pub fn set_display_mode(&self, mode: LibraryDisplayMode) -> Result<(), PrefError> {
        self.sources.source_display_mode.set(mode)
    }

    /// The grid columns for a window `width` wide: the landscape key from 720 px, otherwise the portrait key (T-41a-050).
    /// Zero or less is adaptive (E-BR-23).
    pub fn columns(&self, width: i32) -> GridColumns {
        grid_columns(width, self.library.anime_portrait_columns.get(), self.library.anime_landscape_columns.get())
    }

    /// `default_anime_category`, default -1 (ask every time). The system category 0 is never offered.
    pub fn default_category(&self) -> i64 {
        i64::from(self.library.default_anime_category.get())
    }

    /// `__APP_STATE_incognito_mode`, default off. While on, opening a browse page writes no last-used source.
    pub fn incognito(&self) -> bool {
        self.base.incognito_mode.get()
    }

    /// `GetAnimeIncognitoState.await`: the global switch, or the source's extension package being in
    /// `incognito_anime_extensions`. A source with no installed package (a stub) is only covered by the global switch.
    pub fn incognito_for(&self, package: Option<&str>) -> bool {
        self.incognito() || package.is_some_and(|package| self.incognito_extensions().contains(package))
    }

    fn incognito_extensions(&self) -> BTreeSet<String> {
        self.sources.incognito_anime_extensions.get()
    }

    /// Opening a browse page, for an installed or a stub source: stores it as `last_used_anime_source` unless incognito
    /// applies (T-41b-010, T-41b-025). Returns the id written. Favourites and pins are never gated by incognito.
    pub fn record_last_used(&self, source_id: i64, package: Option<&str>) -> Result<Option<i64>, PrefError> {
        let id = last_used_write(self.incognito_for(package), source_id);
        if let Some(id) = id {
            self.sources.last_used_anime_source.set(id)?;
        }
        Ok(id)
    }

    /// `browse_hide_in_anime_library_items`, default off. Read once per browse page.
    pub fn hide_in_library(&self) -> bool {
        self.sources.hide_in_anime_library_items.get()
    }

    /// `anime_migrate_flags`, default every option. The manga key `migrate_flags` is copied into it on the first read when
    /// the anime key is not stored yet; the copy is marked, so the old key is never read again (T-41a-033).
    pub fn migrate_flags(&self) -> i32 {
        if self.store.get_raw(ANIME_MIGRATE_IMPORTED).is_none() {
            self.store.set_raw(ANIME_MIGRATE_IMPORTED, PrefValue::Bool(true));
            if let (None, Some(PrefValue::Int(old))) = (self.store.get_raw(ANIME_MIGRATE_FLAGS), self.store.get_raw("migrate_flags")) {
                self.store.set_raw(ANIME_MIGRATE_FLAGS, PrefValue::Int(old));
            }
        }
        self.store.pref(ANIME_MIGRATE_FLAGS, MIGRATE_FLAGS_DEFAULT).get()
    }

    /// Stores the migrate bitmap. A migration writes it before any work starts (behavior 11.3 step 1).
    pub fn save_migrate_flags(&self, bitmap: i32) -> Result<(), PrefError> {
        self.store.pref(ANIME_MIGRATE_FLAGS, MIGRATE_FLAGS_DEFAULT).set(bitmap)
    }
}

const ANIME_MIGRATE_FLAGS: &str = "anime_migrate_flags";
const ANIME_MIGRATE_IMPORTED: &str = "__APP_STATE_anime_migrate_flags_imported";

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_prefs::values::PrefValue;

    fn prefs() -> AnimeBrowsePrefs {
        AnimeBrowsePrefs::new(PrefStore::in_memory())
    }

    #[test]
    fn the_defaults_match_the_listed_ones() {
        let p = prefs();
        assert_eq!(p.display_mode(), LibraryDisplayMode::CompactGrid);
        assert_eq!(p.columns(800), GridColumns::Adaptive { cell_px: 128 }, "0 is adaptive");
        assert_eq!(p.default_category(), -1);
        assert!(!p.incognito());
        assert!(!p.hide_in_library());
        assert_eq!(p.sources.last_used_anime_source.get(), -1);
        assert!(p.sources.show_nsfw_source.get());
    }

    #[test]
    fn an_unknown_display_mode_reads_as_compact_grid_and_a_set_one_reads_back() {
        let store = PrefStore::in_memory();
        store.set_raw("pref_display_mode_catalogue", PrefValue::Str("NOT_A_MODE".into()));
        assert_eq!(AnimeBrowsePrefs::new(store).display_mode(), LibraryDisplayMode::CompactGrid);
        let p = prefs();
        p.set_display_mode(LibraryDisplayMode::List).expect("set");
        assert_eq!(p.display_mode(), LibraryDisplayMode::List);
    }

    #[test]
    fn the_anime_column_keys_pick_the_grid_and_the_manga_keys_do_not() {
        let store = PrefStore::in_memory();
        let p = AnimeBrowsePrefs::new(store.clone());
        p.library.anime_landscape_columns.set(3).expect("set");
        p.library.manga_portrait_columns.set(9).expect("set");
        assert_eq!(p.columns(800), GridColumns::Fixed(3), "a wide window reads the landscape key");
        assert_eq!(p.columns(600), GridColumns::Adaptive { cell_px: 128 }, "the portrait key is still 0");
        assert_eq!(store.get_raw("pref_animelib_columns_landscape_key"), Some(PrefValue::Int(3)));
    }

    #[test]
    fn a_negative_column_value_is_adaptive() {
        let p = prefs();
        p.library.anime_portrait_columns.set(-2).expect("set");
        assert!(matches!(p.columns(400), GridColumns::Adaptive { .. }));
    }

    #[test]
    fn opening_a_source_writes_it_as_last_used_for_an_installed_source_and_for_a_stub() {
        let p = prefs();
        assert_eq!(p.record_last_used(11, Some("eu.example.alpha")).expect("write"), Some(11));
        assert_eq!(p.sources.last_used_anime_source.get(), 11);
        assert_eq!(p.record_last_used(999, None).expect("write"), Some(999), "a stub has no package and is still stored");
        assert_eq!(p.sources.last_used_anime_source.get(), 999);
    }

    #[test]
    fn global_incognito_blocks_the_write_for_every_source() {
        let p = prefs();
        p.base.incognito_mode.set(true).expect("set");
        assert_eq!(p.record_last_used(11, Some("eu.example.alpha")).expect("write"), None);
        assert_eq!(p.record_last_used(999, None).expect("write"), None);
        assert_eq!(p.sources.last_used_anime_source.get(), -1);
    }

    #[test]
    fn an_incognito_extension_blocks_only_its_own_sources() {
        let p = prefs();
        p.sources.incognito_anime_extensions.set(["eu.example.alpha".to_owned()].into()).expect("set");
        assert_eq!(p.record_last_used(11, Some("eu.example.alpha")).expect("write"), None);
        assert_eq!(p.sources.last_used_anime_source.get(), -1);
        assert_eq!(p.record_last_used(12, Some("eu.example.beta")).expect("write"), Some(12));
        assert_eq!(p.record_last_used(0, None).expect("write"), Some(0), "the local source has no package");
    }

    #[test]
    fn the_browse_prefs_read_the_anime_keys_and_the_shared_display_mode() {
        let p = prefs();
        p.library.anime_portrait_columns.set(4).expect("set");
        p.library.manga_portrait_columns.set(9).expect("set");
        p.library.default_anime_category.set(7).expect("set");
        p.set_display_mode(LibraryDisplayMode::List).expect("set");
        let browse = p.browse_prefs();
        assert_eq!(browse.library_columns_portrait.get(), 4);
        assert_eq!(browse.default_manga_category.get(), 7);
        assert_eq!(browse.display_mode(), LibraryDisplayMode::List);
    }

    #[test]
    fn the_anime_migrate_flags_import_the_old_manga_key_once_and_default_to_every_option() {
        let store = PrefStore::in_memory();
        assert_eq!(AnimeBrowsePrefs::new(store.clone()).migrate_flags(), i32::MAX, "no old key: every option");
        let store = PrefStore::in_memory();
        store.set_raw("migrate_flags", PrefValue::Int(3));
        let p = AnimeBrowsePrefs::new(store.clone());
        assert_eq!(p.migrate_flags(), 3, "the old value is imported on the first read");
        p.save_migrate_flags(0x10).expect("save");
        assert_eq!(p.migrate_flags(), 0x10, "the saved bitmap is read back");
        store.set_raw("migrate_flags", PrefValue::Int(7));
        assert_eq!(p.migrate_flags(), 0x10, "the old key is never read again");
        assert_eq!(store.get_raw("migrate_flags"), Some(PrefValue::Int(7)), "the manga key is untouched");
    }

    #[test]
    fn the_default_category_and_the_incognito_switch_read_their_stored_values() {
        let p = prefs();
        p.library.default_anime_category.set(4).expect("set");
        p.base.incognito_mode.set(true).expect("set");
        assert_eq!(p.default_category(), 4);
        assert!(p.incognito());
    }
}
