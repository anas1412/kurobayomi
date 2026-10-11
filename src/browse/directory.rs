//! The seam between the browse pages and the installed manga sources (T-40b-060, T-40b-070). The app has no manga source
//! registry of its own yet, so the pages ask a [`SourceDirectory`] for the installed sources and their loaders; the
//! registry's adapter calls [`install`] once it exists. With none installed every page still builds: global search has no
//! sources to ask, and the migration picker lists each library source as a stub.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use kuro_data::manga::Manga;
use kuro_domain::source::model::Source;
use kuro_source::{MangaSource, SourcePref};
use kuro_source::filter_list::FilterList;
use kuro_source::model::SChapter;

use super::listing::{LinkFuture, PageLoader};

/// One installed catalogue source and the package it came from (`""` when it has none, as the local source).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirectorySource {
    pub source: Source,
    pub package: String,
    /// The source's site, for Open in WebView.
    pub home_url: Option<String>,
}

/// The chapter list of a manga as its source reports it (`getChapterList`), or the reason it could not be read.
pub type ChaptersFuture = Pin<Box<dyn Future<Output = Result<Vec<SChapter>, String>> + Send>>;

/// What the browse pages need of the installed sources.
pub trait SourceDirectory: Send + Sync + 'static {
    /// The installed catalogue sources.
    fn sources(&self) -> Vec<DirectorySource>;
    /// The loader of a source's listings, with each entry mapped to its library row. `None` when it is not installed.
    fn loader(&self, source_id: i64) -> Option<Arc<dyn PageLoader>>;
    /// The filter list a source's search sheet starts from. `None` when it is not installed.
    fn filters(&self, source_id: i64) -> Option<FilterList>;
    /// The settings rows a configurable source declares (`ConfigurableSource::preferences`). `None` when the source is not
    /// installed or has no settings screen: the details page then shows no settings button for it.
    fn preferences(&self, _source_id: i64) -> Option<Vec<SourcePref>> {
        None
    }
    /// The chapter list of `manga` from its source, for the migration engine. `None` when this directory cannot list
    /// chapters: a migration then stops before it writes anything.
    fn fetch_chapters(&self, _source_id: i64, _manga: &Manga) -> Option<ChaptersFuture> {
        None
    }
    /// Resolves a pasted link to the library row of the entry it names, asking each source's `uri_type` first (see
    /// `deep_link::resolve`). `None` when this directory has no source that can resolve links: global search then
    /// searches the catalogues with the text (T-20a-031).
    fn resolve_link(&self, _uri: &str) -> Option<LinkFuture> {
        None
    }
    /// The runnable source of an id, for the entry page's details and chapter fetches and the reader's pages. `None` when
    /// this directory does not hand out its sources.
    fn manga_source(&self, _source_id: i64) -> Option<Arc<dyn MangaSource>> {
        None
    }
}

/// Two directories read as one: the sources of both, and each question answered by the first that knows the id.
struct Both(Arc<dyn SourceDirectory>, Arc<dyn SourceDirectory>);

impl SourceDirectory for Both {
    fn sources(&self) -> Vec<DirectorySource> {
        let mut all = self.0.sources();
        all.extend(self.1.sources());
        all
    }
    fn loader(&self, source_id: i64) -> Option<Arc<dyn PageLoader>> {
        self.0.loader(source_id).or_else(|| self.1.loader(source_id))
    }
    fn filters(&self, source_id: i64) -> Option<FilterList> {
        self.0.filters(source_id).or_else(|| self.1.filters(source_id))
    }
    fn preferences(&self, source_id: i64) -> Option<Vec<SourcePref>> {
        self.0.preferences(source_id).or_else(|| self.1.preferences(source_id))
    }
    fn fetch_chapters(&self, source_id: i64, manga: &Manga) -> Option<ChaptersFuture> {
        self.0.fetch_chapters(source_id, manga).or_else(|| self.1.fetch_chapters(source_id, manga))
    }
    fn resolve_link(&self, uri: &str) -> Option<LinkFuture> {
        self.0.resolve_link(uri).or_else(|| self.1.resolve_link(uri))
    }
    fn manga_source(&self, source_id: i64) -> Option<Arc<dyn MangaSource>> {
        self.0.manga_source(source_id).or_else(|| self.1.manga_source(source_id))
    }
}

static CURRENT: Mutex<Option<Arc<dyn SourceDirectory>>> = Mutex::new(None);

/// Installs the directory the pages read. A second call replaces the first.
pub fn install(directory: Arc<dyn SourceDirectory>) {
    *CURRENT.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(directory);
}

/// The directory the pages read: the built-in sources (`builtin`) in front of the installed directory, either alone
/// when the other is missing, and `None` when the app has neither.
pub fn current() -> Option<Arc<dyn SourceDirectory>> {
    let installed = CURRENT.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone();
    let builtin = super::builtin::current().map(|b| b as Arc<dyn SourceDirectory>);
    match (builtin, installed) {
        (Some(builtin), Some(installed)) => Some(Arc::new(Both(builtin, installed))),
        (builtin, installed) => builtin.or(installed),
    }
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

    /// The built-in sources sit in front of the installed directory: both lists, each id answered by the one that has it.
    #[test]
    fn two_directories_read_as_one() {
        let builtin = Arc::new(FakeDirectory { entries: vec![entry(1, "Built", "en", "builtin.a")], answers: Arc::new(|_, _| Ok(Vec::new())) });
        let recipes = Arc::new(FakeDirectory { entries: vec![entry(2, "Recipe", "en", "scraper.b")], answers: Arc::new(|_, _| Ok(Vec::new())) });
        let both = Both(builtin, recipes);
        assert_eq!(both.sources().iter().map(|e| e.source.id).collect::<Vec<_>>(), [1, 2]);
        assert!(both.loader(1).is_some() && both.loader(2).is_some() && both.loader(3).is_none());
        assert!(both.filters(2).is_some() && both.manga_source(1).is_none());
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
