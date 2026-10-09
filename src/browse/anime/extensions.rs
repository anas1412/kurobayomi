//! The anime extension list model (T-41a-016). The groups follow the query, the NSFW switch and the enabled languages,
//! and are recomputed when one of the preferences changes. The installer is read as data (`extension_installer`) and
//! produces no banner on Linux, where the install path has no permission step to show (T-41a-025). The update count is
//! derived from the rows (installed with an update), since the preferences do not store it (T-19a-064).

use std::collections::BTreeSet;

use kuro_prefs::base::BasePrefs;
use kuro_prefs::source::SourcePrefs;

use crate::browse::extensions::{Ext, ExtGroup, ExtState, group, split_available};

/// The extension list: the rows, the query, the preferences it reads, and the groups computed from them.
pub struct ExtensionsModel {
    exts: Vec<Ext>,
    query: String,
    show_nsfw: bool,
    enabled_langs: BTreeSet<String>,
    installer: String,
    updates: usize,
    groups: Vec<ExtGroup>,
}

impl ExtensionsModel {
    /// Reads `show_nsfw_source`, `source_languages` and the installer, and computes the groups.
    pub fn new(exts: Vec<Ext>, query: &str, prefs: &SourcePrefs, base: &BasePrefs) -> Self {
        let updates = exts.iter().filter(|e| matches!(e.state, ExtState::Installed { has_update: true, .. })).count();
        let mut model = Self {
            exts,
            query: query.to_owned(),
            show_nsfw: true,
            enabled_langs: BTreeSet::new(),
            installer: String::new(),
            updates,
            groups: Vec::new(),
        };
        model.read_prefs(prefs, base);
        model
    }

    /// Called when a preference changes: reads them again and recomputes the groups (race R8).
    pub fn on_pref_change(&mut self, prefs: &SourcePrefs, base: &BasePrefs) {
        self.read_prefs(prefs, base);
    }

    pub fn set_query(&mut self, query: &str) {
        self.query = query.to_owned();
        self.recompute();
    }

    pub fn groups(&self) -> &[ExtGroup] {
        &self.groups
    }

    /// The number of rows across all groups.
    pub fn row_count(&self) -> usize {
        self.groups.iter().map(|g| g.rows.len()).sum()
    }

    /// Installed extensions with an update: the badge on the Extensions tab, shown when above zero.
    pub fn updates(&self) -> usize {
        self.updates
    }

    /// The stored installer name, kept for parity. Nothing on Linux reads it to show a banner.
    pub fn installer(&self) -> &str {
        &self.installer
    }

    fn read_prefs(&mut self, prefs: &SourcePrefs, base: &BasePrefs) {
        self.show_nsfw = prefs.show_nsfw_source.get();
        self.enabled_langs = prefs.enabled_languages.get();
        self.installer = base.extension_installer.get();
        self.recompute();
    }

    fn recompute(&mut self) {
        let visible = split_available(&self.exts, Some(&self.enabled_langs));
        self.groups = group(&visible, &self.query, self.show_nsfw);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_prefs::store::PrefStore;

    fn ext(name: &str, pkg: &str, lang: &str, nsfw: bool, state: ExtState) -> Ext {
        Ext {
            name: name.to_owned(),
            pkg: pkg.to_owned(),
            version: "1.0".to_owned(),
            lang: Some(lang.to_owned()),
            nsfw,
            sources: Vec::new(),
            state,
            icon_file: None,
        }
    }

    fn installed() -> ExtState {
        ExtState::Installed { has_update: false, obsolete: false }
    }

    fn fixtures() -> Vec<Ext> {
        vec![
            ext("Safe Anime", "pkg.safe", "en", false, installed()),
            ext("Adult Pack", "pkg.adult", "en", true, installed()),
            ext("French Feed", "pkg.fr", "fr", false, ExtState::Available),
        ]
    }

    #[test]
    fn the_nsfw_switch_is_read_at_start_and_again_when_it_changes() {
        let store = PrefStore::in_memory();
        let prefs = SourcePrefs::new(store.clone());
        let base = BasePrefs::new(store);
        prefs.enabled_languages.set(BTreeSet::from(["en".to_owned()])).expect("store");
        let mut model = ExtensionsModel::new(fixtures(), "", &prefs, &base);
        assert!(prefs.show_nsfw_source.get(), "the default shows NSFW sources");
        assert_eq!(model.row_count(), 2, "the French available row is not in an enabled language");
        prefs.show_nsfw_source.set(false).expect("store");
        model.on_pref_change(&prefs, &base);
        assert_eq!(model.row_count(), 1, "the adult installed row leaves the list");
    }

    #[test]
    fn a_language_change_recomputes_the_available_rows() {
        let store = PrefStore::in_memory();
        let prefs = SourcePrefs::new(store.clone());
        let base = BasePrefs::new(store);
        prefs.enabled_languages.set(BTreeSet::from(["en".to_owned(), "fr".to_owned()])).expect("store");
        let mut model = ExtensionsModel::new(fixtures(), "", &prefs, &base);
        assert_eq!(model.row_count(), 3, "French is enabled, so its available row shows");
        prefs.enabled_languages.set(BTreeSet::from(["en".to_owned()])).expect("store");
        model.on_pref_change(&prefs, &base);
        assert_eq!(model.row_count(), 2, "French is disabled, so its available row goes");
    }

    #[test]
    fn the_query_filters_the_groups_by_name() {
        let store = PrefStore::in_memory();
        let prefs = SourcePrefs::new(store.clone());
        let base = BasePrefs::new(store);
        let mut model = ExtensionsModel::new(fixtures(), "", &prefs, &base);
        model.set_query("adult");
        assert_eq!(model.row_count(), 1);
        assert_eq!(model.groups().len(), 1);
    }

    #[test]
    fn the_update_count_is_the_installed_rows_with_an_update_and_defaults_to_zero() {
        let store = PrefStore::in_memory();
        let prefs = SourcePrefs::new(store.clone());
        let base = BasePrefs::new(store);
        let none = ExtensionsModel::new(fixtures(), "", &prefs, &base);
        assert_eq!(none.updates(), 0);
        let mut exts = fixtures();
        exts.push(ext("Newer", "pkg.new", "en", false, ExtState::Installed { has_update: true, obsolete: false }));
        exts.push(ext("Stale", "pkg.stale", "en", false, ExtState::Installed { has_update: false, obsolete: true }));
        assert_eq!(ExtensionsModel::new(exts, "", &prefs, &base).updates(), 1);
    }

    #[test]
    fn the_installer_is_read_as_data_from_the_base_preferences() {
        let store = PrefStore::in_memory();
        let prefs = SourcePrefs::new(store.clone());
        let base = BasePrefs::new(store);
        let mut model = ExtensionsModel::new(fixtures(), "", &prefs, &base);
        assert_eq!(model.installer(), "PACKAGEINSTALLER", "the default installer");
        base.extension_installer.set("SHIZUKU".into()).expect("store");
        model.on_pref_change(&prefs, &base);
        assert_eq!(model.installer(), "SHIZUKU");
    }
}
