//! The seam between the browse pages and the installed manga sources (T-40b-060, T-40b-070). The app has no manga source
//! registry of its own yet, so the pages ask a [`SourceDirectory`] for the installed sources and their loaders; the
//! registry's adapter calls [`install`] once it exists. With none installed every page still builds: global search has no
//! sources to ask, and the migration picker lists each library source as a stub.

use std::sync::{Arc, Mutex};

use kuro_domain::source::model::Source;
use kuro_source::filter_list::FilterList;

use super::listing::PageLoader;

/// One installed catalogue source and the package it came from (`""` when it has none, as the local source).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirectorySource {
    pub source: Source,
    pub package: String,
    /// The source's site, for Open in WebView.
    pub home_url: Option<String>,
}

/// What the browse pages need of the installed sources.
pub trait SourceDirectory: Send + Sync + 'static {
    /// The installed catalogue sources.
    fn sources(&self) -> Vec<DirectorySource>;
    /// The loader of a source's listings, with each entry mapped to its library row. `None` when it is not installed.
    fn loader(&self, source_id: i64) -> Option<Arc<dyn PageLoader>>;
    /// The filter list a source's search sheet starts from. `None` when it is not installed.
    fn filters(&self, source_id: i64) -> Option<FilterList>;
}

static CURRENT: Mutex<Option<Arc<dyn SourceDirectory>>> = Mutex::new(None);

/// Installs the directory the pages read. A second call replaces the first.
pub fn install(directory: Arc<dyn SourceDirectory>) {
    *CURRENT.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(directory);
}

/// The installed directory, if the app has one.
pub fn current() -> Option<Arc<dyn SourceDirectory>> {
    CURRENT.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone()
}

/// A source as the migration picker shows it: the installed one, or a stub with no name or language when the directory
/// does not know the id (`getOrStub`, T-40b-070). `directory` is `None` when the app has none.
pub fn source_or_stub(directory: Option<&dyn SourceDirectory>, id: i64) -> Source {
    directory
        .and_then(|directory| directory.sources().into_iter().find(|entry| entry.source.id == id))
        .map(|entry| entry.source)
        .unwrap_or_else(|| Source { is_stub: true, ..Source::new(id, "", "") })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::browse::listing::{ListedPage, LoadFuture};
    use kuro_domain::source::browse::BrowseQuery;

    /// A directory over a list of sources whose search answers with the scripted result of the source id.
    pub struct FakeDirectory {
        pub entries: Vec<DirectorySource>,
        pub answers: Arc<dyn Fn(i64, &BrowseQuery) -> Result<Vec<super::super::listing::ListItem>, String> + Send + Sync>,
    }

    struct FakeLoader {
        id: i64,
        answers: Arc<dyn Fn(i64, &BrowseQuery) -> Result<Vec<super::super::listing::ListItem>, String> + Send + Sync>,
    }

    impl PageLoader for FakeLoader {
        fn load(&self, query: BrowseQuery, _filters: FilterList, _page: u32) -> LoadFuture {
            let answer = (self.answers)(self.id, &query);
            Box::pin(async move { answer.map(|items| ListedPage { items, has_next: false }) })
        }
    }

    impl SourceDirectory for FakeDirectory {
        fn sources(&self) -> Vec<DirectorySource> {
            self.entries.clone()
        }
        fn loader(&self, source_id: i64) -> Option<Arc<dyn PageLoader>> {
            self.entries
                .iter()
                .any(|entry| entry.source.id == source_id)
                .then(|| Arc::new(FakeLoader { id: source_id, answers: Arc::clone(&self.answers) }) as Arc<dyn PageLoader>)
        }
        fn filters(&self, source_id: i64) -> Option<FilterList> {
            self.entries.iter().any(|entry| entry.source.id == source_id).then(FilterList::default)
        }
    }

    pub fn entry(id: i64, name: &str, lang: &str, package: &str) -> DirectorySource {
        DirectorySource { source: Source::new(id, name, lang), package: package.to_owned(), home_url: None }
    }

    #[test]
    fn an_unknown_source_is_a_stub_and_a_known_one_is_itself() {
        let directory = FakeDirectory { entries: vec![entry(1, "Alpha", "en", "ext.a")], answers: Arc::new(|_, _| Ok(Vec::new())) };
        let known = source_or_stub(Some(&directory), 1);
        assert!(!known.is_stub, "an installed source is not a stub");
        assert_eq!(known.name, "Alpha");
        let unknown = source_or_stub(Some(&directory), 9);
        assert!(unknown.is_stub, "an id the directory does not know is a stub (T-40b-070)");
        assert_eq!(unknown.id, 9);
        assert!(source_or_stub(None, 1).is_stub, "with no directory every source is a stub");
    }

    #[test]
    fn the_installed_directory_is_read_back_and_replaced() {
        let one = Arc::new(FakeDirectory { entries: vec![entry(1, "A", "en", "")], answers: Arc::new(|_, _| Ok(Vec::new())) });
        install(one);
        assert_eq!(current().map(|d| d.sources().len()), Some(1));
        let two = Arc::new(FakeDirectory { entries: vec![entry(1, "A", "en", ""), entry(2, "B", "en", "")], answers: Arc::new(|_, _| Ok(Vec::new())) });
        install(two);
        assert_eq!(current().map(|d| d.sources().len()), Some(2));
        assert!(current().is_some_and(|d| d.loader(2).is_some() && d.loader(3).is_none()));
        *CURRENT.lock().expect("lock") = None;
    }
}
