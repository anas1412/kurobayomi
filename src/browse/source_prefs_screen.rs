//! The source preferences screen (`source_preferences`; T-41a-023, mockup gap G24): opened by the Settings button of the
//! extension details page, by the Settings item of a browse page's menu and by the one of an entry page, each of which
//! parks a `SourcePrefsRequest`. The title is the source's `toString()`, `Name (LANG)`; the rows are the ones the source
//! declares through the directory of its kind; a source that declares none is not configurable and the page says so.
//! Every row writes into the source's own preference store, which `SourcePrefsPage` does.

use std::rc::Rc;
use std::sync::Arc;

use kuro_domain::source::model::Source;
use kuro_domain::source::settings::SourcePrefs;
use kuro_prefs::store::PrefStore;
use kuro_source::SourcePref;
use tokio::sync::watch;

use super::directory::{SourceDirectory, source_or_stub};
use super::primitives::stub_source_label;
use super::route::BrowseKind;
use super::source_prefs_page::SourcePrefsPage;
use crate::entry_core::handoff::SourcePrefsRequest;

/// The page title, `toString()` of the source: `Name (LANG)`. A source that is not installed shows its stub label, its
/// id, so the page never crashes on it (upstream reads `source!!`).
pub fn title_of(source: &Source) -> String {
    if source.is_stub {
        stub_source_label(&source.name, &source.lang, source.id)
    } else {
        kuro_source::display_name(&source.name, &source.lang)
    }
}

/// The source and its rows for `source_id`: the installed source or a stub, and what it declares. A source the directory
/// does not know, or one that declares nothing, has no rows.
pub fn rows_of(directory: Option<&dyn SourceDirectory>, source_id: i64) -> (Source, Vec<SourcePref>) {
    let source = source_or_stub(directory, source_id);
    let rows = directory.and_then(|directory| directory.preferences(source_id)).unwrap_or_default();
    (source, rows)
}

/// The page over `rows`, writing into `store` under `source_<id>`.
pub fn prefs_page(store: PrefStore, source_id: i64, rows: Vec<SourcePref>) -> SourcePrefsPage {
    let (_never_sent, ready) = watch::channel(true);
    let page = SourcePrefsPage::new(SourcePrefs::new(store, source_id), Rc::new(|_| {}), ready);
    page.set_rows(rows);
    page
}

/// The directory the request's kind reads.
fn directory_of(request: &SourcePrefsRequest) -> Arc<dyn SourceDirectory> {
    super::directory::current(if request.anime { BrowseKind::Anime } else { BrowseKind::Manga })
}

/// The page for the parked request, as `screens::build` asks for it. `None` leaves the screen to its sample page: no app
/// data, or nothing was parked.
pub fn page() -> Option<gtk::Box> {
    if !crate::data::is_installed() {
        return None;
    }
    let request = crate::entry_core::handoff::take_source_prefs()?;
    let directory = directory_of(&request);
    let (source, rows) = rows_of(Some(&*directory), request.source_id);
    let content = prefs_page(crate::data::app().base.store().clone(), request.source_id, rows);
    let page = crate::ui::page(&title_of(&source), content.widget(), Vec::new(), true);
    crate::ui::tag(&page, "source_preferences");
    Some(page)
}

#[cfg(test)]
pub(crate) mod tests {
    use gtk::prelude::*;

    use super::*;
    use crate::browse::directory::{DirectorySource, SourceDirectory};
    use crate::browse::listing::PageLoader;

    /// A directory that knows one source and declares rows for it.
    struct Declaring {
        rows: Vec<SourcePref>,
    }

    impl SourceDirectory for Declaring {
        fn sources(&self) -> Vec<DirectorySource> {
            vec![DirectorySource { source: Source::new(7, "Alpha", "en"), package: "ext.a".into(), home_url: None }]
        }
        fn loader(&self, _source_id: i64) -> Option<Arc<dyn PageLoader>> {
            None
        }
        fn filters(&self, _source_id: i64) -> Option<kuro_source::filter_list::FilterList> {
            None
        }
        fn preferences(&self, source_id: i64) -> Option<Vec<SourcePref>> {
            (source_id == 7 && !self.rows.is_empty()).then(|| self.rows.clone())
        }
    }

    fn switch(key: &str) -> SourcePref {
        SourcePref::Switch { key: key.into(), title: "T".into(), summary: None, default: false }
    }

    #[test]
    fn the_title_is_the_sources_to_string_and_a_stub_shows_its_label() {
        assert_eq!(title_of(&Source::new(7, "Alpha", "en")), "Alpha (EN)");
        let stub = Source { is_stub: true, ..Source::new(9, "", "") };
        assert_eq!(title_of(&stub), "9", "a missing source never crashes the page");
    }

    #[test]
    fn the_rows_are_what_the_directory_declares_and_nothing_for_an_unknown_or_plain_source() {
        let directory = Declaring { rows: vec![switch("hd")] };
        let (source, rows) = rows_of(Some(&directory), 7);
        assert_eq!((source.name.as_str(), rows.len()), ("Alpha", 1));
        let (stub, rows) = rows_of(Some(&directory), 8);
        assert!(stub.is_stub && rows.is_empty(), "an unknown source is a stub with no rows");
        assert!(rows_of(Some(&Declaring { rows: Vec::new() }), 7).1.is_empty(), "no declared rows: not configurable");
        assert!(rows_of(None, 7).1.is_empty());
    }

    fn the_page_writes_into_the_sources_own_store_and_a_plain_source_shows_the_awkward_screen() {
        let store = PrefStore::in_memory();
        let page = prefs_page(store.clone(), 7, vec![switch("hd")]);
        fn switches(widget: &gtk::Widget, out: &mut Vec<gtk::Switch>) {
            if let Some(found) = widget.downcast_ref::<gtk::Switch>() {
                out.push(found.clone());
            }
            let mut child = widget.first_child();
            while let Some(next) = child {
                switches(&next, out);
                child = next.next_sibling();
            }
        }
        let mut found = Vec::new();
        switches(page.widget().upcast_ref(), &mut found);
        assert_eq!(found.len(), 1);
        found[0].emit_by_name::<bool>("state-set", &[&true]);
        assert!(store.get_raw("source_7.hd").is_some(), "the value is stored under source_<id>");

        let plain = prefs_page(PrefStore::in_memory(), 7, Vec::new());
        let mut text = Vec::new();
        fn labels(widget: &gtk::Widget, out: &mut Vec<String>) {
            if let Some(label) = widget.downcast_ref::<gtk::Label>() {
                out.push(label.label().to_string());
            }
            let mut child = widget.first_child();
            while let Some(next) = child {
                labels(&next, out);
                child = next.next_sibling();
            }
        }
        labels(plain.widget().upcast_ref(), &mut text);
        assert!(text.contains(&kuro_i18n::tr(kuro_i18n::MsgKey::empty_screen)), "non-configurable: {text:?}");
    }

    pub(crate) fn smoke() {
        the_page_writes_into_the_sources_own_store_and_a_plain_source_shows_the_awkward_screen();
    }
}
