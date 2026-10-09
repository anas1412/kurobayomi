//! The anime extension list model (T-41a-016). The groups follow the query, the NSFW switch and the enabled languages,
//! and are recomputed when one of the preferences changes. The installer kind is not read, because the Linux install
//! path has no permission banner to drive (T-41a-025). Ports the inputs of `AnimeExtensionsScreenModel`.

use std::collections::BTreeSet;

use kuro_prefs::source::SourcePrefs;

use crate::browse::extensions::{Ext, ExtGroup, group, split_available};

/// The extension list: the rows, the query, the two preferences it reads, and the groups computed from them.
pub struct ExtensionsModel {
    exts: Vec<Ext>,
    query: String,
    show_nsfw: bool,
    enabled_langs: BTreeSet<String>,
    groups: Vec<ExtGroup>,
}

impl ExtensionsModel {
    /// Reads `show_nsfw_source` and `source_languages` from the preferences and computes the groups.
    pub fn new(exts: Vec<Ext>, query: &str, prefs: &SourcePrefs) -> Self {
        let mut model = Self {
            exts,
            query: query.to_owned(),
            show_nsfw: true,
            enabled_langs: BTreeSet::new(),
            groups: Vec::new(),
        };
        model.read_prefs(prefs);
        model
    }

    /// Called when either preference changes: reads both again and recomputes the groups (race R8).
    pub fn on_pref_change(&mut self, prefs: &SourcePrefs) {
        self.read_prefs(prefs);
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

    fn read_prefs(&mut self, prefs: &SourcePrefs) {
        self.show_nsfw = prefs.show_nsfw_source.get();
        self.enabled_langs = prefs.enabled_languages.get();
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
    use crate::browse::extensions::ExtState;
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
        let prefs = SourcePrefs::new(PrefStore::in_memory());
        prefs.enabled_languages.set(BTreeSet::from(["en".to_owned()])).expect("store");
        let mut model = ExtensionsModel::new(fixtures(), "", &prefs);
        assert!(prefs.show_nsfw_source.get(), "the default shows NSFW sources");
        assert_eq!(model.row_count(), 2, "the French available row is not in an enabled language");
        prefs.show_nsfw_source.set(false).expect("store");
        model.on_pref_change(&prefs);
        assert_eq!(model.row_count(), 1, "the adult installed row leaves the list");
    }

    #[test]
    fn a_language_change_recomputes_the_available_rows() {
        let prefs = SourcePrefs::new(PrefStore::in_memory());
        prefs.enabled_languages.set(BTreeSet::from(["en".to_owned(), "fr".to_owned()])).expect("store");
        let mut model = ExtensionsModel::new(fixtures(), "", &prefs);
        assert_eq!(model.row_count(), 3, "French is enabled, so its available row shows");
        prefs.enabled_languages.set(BTreeSet::from(["en".to_owned()])).expect("store");
        model.on_pref_change(&prefs);
        assert_eq!(model.row_count(), 2, "French is disabled, so its available row goes");
    }

    #[test]
    fn the_query_filters_the_groups_by_name() {
        let prefs = SourcePrefs::new(PrefStore::in_memory());
        let mut model = ExtensionsModel::new(fixtures(), "", &prefs);
        model.set_query("adult");
        assert_eq!(model.row_count(), 1);
        assert_eq!(model.groups().len(), 1);
    }
}
