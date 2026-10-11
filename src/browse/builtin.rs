//! The sources this build ships: Weeb Central and Comix for manga, KickAssAnime for anime.
//! `start` builds them once over the app's network client. The manga ones are read through `directory::current`, which
//! puts them in front of the installed directory (the recipe sources); the anime ones are added to the anime directory,
//! the anime Sources list and the anime entry page next to the recipe sources. `start` also installs the image service,
//! so the remote covers of the Browse grids, the library and the entry pages draw through the image cache, with each
//! built-in source's image headers.
//!
//! The sources keep their own rate limits (one request at a time, a gap between requests), so nothing here throttles.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use kuro_data::manga::Manga;
use kuro_domain::entries::source_convert::to_s_manga;
use kuro_domain::source::browse::BrowseQuery;
use kuro_domain::source::model::Source;
use kuro_images::fetch::{ImageService, SourceClient, SourceHttp};
use kuro_net::NetworkClient;
use kuro_platform::paths::Paths;
use kuro_prefs::network::NetworkPrefs;
use kuro_prefs::store::PrefStore;
use kuro_source::anime::AnimeSource;
use kuro_source::filter_list::FilterList;
use kuro_source::{CatalogueSource, MangaSource};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

use super::anime::directory::AnimeInstances;
use super::directory::{ChaptersFuture, DirectorySource, SourceDirectory};
use super::entries::FavoriteStore;
use super::listing::{ListItem, ListedPage, LoadFuture, PageLoader};

/// The built-in sources: each with the row the Browse lists show, and the headers of its images by source id.
pub struct Builtin {
    manga: Vec<(Arc<dyn CatalogueSource>, DirectorySource)>,
    anime: Vec<(Arc<dyn AnimeSource>, DirectorySource)>,
    image_headers: BTreeMap<i64, HeaderMap>,
    /// The client the sources were built over; the reader fetches their pages over it, so both share one cookie jar.
    client: Option<NetworkClient>,
}

static BUILTIN: Mutex<Option<Arc<Builtin>>> = Mutex::new(None);

/// The built-in sources, once [`start`] has run.
pub fn current() -> Option<Arc<Builtin>> {
    BUILTIN.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone()
}

fn row(id: i64, name: &str, lang: &str, supports_latest: bool, package: &str, home: &str) -> DirectorySource {
    DirectorySource { source: Source { supports_latest, ..Source::new(id, name, lang) }, package: package.to_owned(), home_url: Some(home.to_owned()) }
}

/// `(name, value)` pairs as a header map; a pair that is not a valid header is left out.
fn header_map(pairs: Vec<(String, String)>) -> HeaderMap {
    pairs.into_iter().filter_map(|(name, value)| Some((HeaderName::try_from(name).ok()?, HeaderValue::try_from(value).ok()?))).collect()
}

impl Builtin {
    /// Sources given as `(source, package, home url)`; a manga source without image headers gets none.
    pub fn new(manga: Vec<(Arc<dyn CatalogueSource>, &str, &str)>, anime: Vec<(Arc<dyn AnimeSource>, &str, &str)>) -> Self {
        let manga = manga
            .into_iter()
            .map(|(source, package, home)| {
                let row = row(source.id(), source.name(), source.lang(), source.supports_latest(), package, home);
                (source, row)
            })
            .collect();
        let anime = anime
            .into_iter()
            .map(|(source, package, home)| {
                let row = row(source.id(), source.name(), source.lang(), source.supports_latest(), package, home);
                (source, row)
            })
            .collect();
        Self { manga, anime, image_headers: BTreeMap::new(), client: None }
    }

    fn manga_of(&self, id: i64) -> Option<&Arc<dyn CatalogueSource>> {
        self.manga.iter().find(|(_, row)| row.source.id == id).map(|(source, _)| source)
    }

    /// The anime sources as directory rows, for the anime directory.
    pub fn anime_rows(&self) -> Vec<DirectorySource> {
        self.anime.iter().map(|(_, row)| row.clone()).collect()
    }

    /// The runnable anime sources, for the anime entry page.
    pub fn anime_sources(&self) -> Vec<Arc<dyn AnimeSource>> {
        self.anime.iter().map(|(source, _)| Arc::clone(source)).collect()
    }

    /// The headers a source's images need (`Referer`, `Accept`; none for Comix, whose image hosts refuse them). The image
    /// service sends them with covers; the reader's page loader can send them with pages.
    pub fn image_headers(&self, source_id: i64) -> Option<HeaderMap> {
        self.image_headers.get(&source_id).cloned()
    }

    /// The network client the shipped sources use, `None` for sources given to [`Builtin::new`].
    pub fn client(&self) -> Option<NetworkClient> {
        self.client.clone()
    }
}

/// The anime rows of the built-in sources, empty before [`start`].
pub fn anime_rows() -> Vec<DirectorySource> {
    current().map(|b| b.anime_rows()).unwrap_or_default()
}

/// The anime sources as the Sources list shows them.
pub fn anime_list() -> Vec<Source> {
    anime_rows().into_iter().map(|row| row.source).collect()
}

/// The runnable built-in anime sources.
pub fn anime_sources() -> Vec<Arc<dyn AnimeSource>> {
    current().map(|b| b.anime_sources()).unwrap_or_default()
}

impl SourceDirectory for Builtin {
    fn sources(&self) -> Vec<DirectorySource> {
        self.manga.iter().map(|(_, row)| row.clone()).collect()
    }

    fn loader(&self, source_id: i64) -> Option<Arc<dyn PageLoader>> {
        let source = Arc::clone(self.manga_of(source_id)?);
        Some(Arc::new(CatalogueLoader { source, store: library_store()? }))
    }

    fn filters(&self, source_id: i64) -> Option<FilterList> {
        self.manga_of(source_id).map(|source| source.filter_list())
    }

    fn fetch_chapters(&self, source_id: i64, manga: &Manga) -> Option<ChaptersFuture> {
        let (source, manga) = (Arc::clone(self.manga_of(source_id)?), to_s_manga(manga));
        Some(Box::pin(async move { source.get_chapter_list(&manga).await.map_err(|e| e.to_string()) }))
    }

    fn manga_source(&self, source_id: i64) -> Option<Arc<dyn MangaSource>> {
        self.manga_of(source_id).map(|source| Arc::clone(source) as Arc<dyn MangaSource>)
    }
}

/// The installed anime instances with the built-in ones in front.
struct WithBuiltin(Arc<Builtin>, Option<Arc<dyn AnimeInstances>>);

impl AnimeInstances for WithBuiltin {
    fn source(&self, id: i64) -> Option<Arc<dyn AnimeSource>> {
        let builtin = self.0.anime.iter().find(|(source, _)| source.id() == id).map(|(source, _)| Arc::clone(source));
        builtin.or_else(|| self.1.as_ref()?.source(id))
    }

    fn preferences(&self, id: i64) -> Option<Vec<kuro_source::SourcePref>> {
        self.1.as_ref()?.preferences(id)
    }
}

/// `installed` with the built-in anime sources in front, once [`start`] has run.
pub fn with_anime(installed: Option<Arc<dyn AnimeInstances>>) -> Option<Arc<dyn AnimeInstances>> {
    match current() {
        Some(builtin) => Some(Arc::new(WithBuiltin(builtin, installed))),
        None => installed,
    }
}

/// The library the manga listings map their entries to, as the manga Browse screen builds its store.
fn library_store() -> Option<Arc<dyn FavoriteStore>> {
    use kuro_data::category::{CategoryKind, CategoryRepo};
    use kuro_data::manga::MangaRepo;
    use kuro_domain::kind::EntryKind;

    if !crate::data::is_installed() {
        return None;
    }
    let app = crate::data::app();
    let db = app.db(EntryKind::Manga);
    let prefs = super::prefs::BrowsePrefs::new(app.base.store().clone());
    Some(Arc::new(super::repo_store::RepoStore::new(
        MangaRepo::new(Arc::clone(&db)),
        CategoryRepo::new(db, CategoryKind::Manga),
        prefs.default_manga_category.clone(),
        Arc::new(|_| 0),
    )))
}

/// A catalogue source's popular, latest or search listing, each entry mapped to its library row (inserted as a
/// non-favourite row of the source when new). The source call runs on the runtime, the store on the blocking pool.
pub struct CatalogueLoader {
    pub source: Arc<dyn CatalogueSource>,
    pub store: Arc<dyn FavoriteStore>,
}

impl PageLoader for CatalogueLoader {
    fn load(&self, query: BrowseQuery, filters: FilterList, page: u32) -> LoadFuture {
        let (source, store) = (Arc::clone(&self.source), Arc::clone(&self.store));
        Box::pin(async move {
            let page = i32::try_from(page).map_err(|error| error.to_string())?;
            // a panicking source is an error of this listing, not a dead task
            let found = kuro_source::isolate(async {
                match query {
                    BrowseQuery::Popular => source.popular(page).await,
                    BrowseQuery::Latest => source.latest(page).await,
                    BrowseQuery::Search(text) => source.search(page, &text, &filters).await,
                }
            })
            .await
            .map_err(|error| kuro_source::messages::source_error_message(&error, true))?;
            let source_id = source.id();
            tokio::task::spawn_blocking(move || -> Result<ListedPage, String> {
                let mut items = Vec::with_capacity(found.mangas.len());
                for manga in &found.mangas {
                    let local = store.local_entry(source_id, &manga.url, &manga.title, manga.thumbnail_url.as_deref())?;
                    items.push(ListItem { id: local.id, source_id, url: manga.url.clone(), title: local.title, favorite: local.favorite, cover: manga.thumbnail_url.clone() });
                }
                Ok(ListedPage { items, has_next: found.has_next_page })
            })
            .await
            .map_err(|error| error.to_string())?
        })
    }
}

/// Image requests of the built-in sources go with their headers; any other source's with none.
struct BuiltinHttp(NetworkClient);

impl SourceHttp for BuiltinHttp {
    fn http_source(&self, source_id: i64) -> Option<SourceClient> {
        let headers = current()?.image_headers(source_id)?;
        Some(SourceClient { client: self.0.clone(), headers })
    }
}

/// Builds the built-in sources and the image service. Call once at start, inside the app's runtime.
pub fn start(store: &PrefStore, paths: &kuro_platform::linux::LinuxPaths) {
    let client = match crate::net_client::build(&NetworkPrefs::new(store.clone()), paths) {
        Ok(client) => client,
        Err(error) => {
            tracing::warn!("the built-in sources do not start: {error}");
            return;
        }
    };
    let built = Arc::new(Builtin::ship(&client));
    tracing::info!(manga = built.manga.len(), anime = built.anime.len(), "built-in sources loaded");
    *BUILTIN.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(built);
    let paths: Arc<dyn Paths + Send + Sync> = Arc::new(paths.clone());
    tokio::spawn(async move {
        match ImageService::new(client.clone(), Arc::new(BuiltinHttp(client)), paths).await {
            Ok(service) => crate::entry_core::cover_view::install_images(Arc::new(service)),
            Err(error) => tracing::warn!("the image service does not start, covers show their placeholder: {error}"),
        }
    });
}

#[cfg(test)]
pub(crate) mod tests {
    use async_trait::async_trait;
    use kuro_source::SourceError;
    use kuro_source::model::{MangasPage, Page, SChapter, SManga};

    use super::*;
    use crate::browse::entries::fake::FakeStore;

    /// A catalogue source over fixed answers: two mangas per listing, one detail text and two chapters. Every call is
    /// logged; `fail` makes every call answer HTTP 503.
    pub struct FakeCatalogue {
        pub id: i64,
        pub fail: std::sync::atomic::AtomicBool,
        pub calls: Mutex<Vec<String>>,
    }

    impl FakeCatalogue {
        pub fn new(id: i64) -> Arc<Self> {
            Arc::new(Self { id, fail: false.into(), calls: Mutex::default() })
        }

        fn log(&self, call: String) -> Result<(), SourceError> {
            self.calls.lock().unwrap().push(call);
            if self.fail.load(std::sync::atomic::Ordering::SeqCst) { Err(SourceError::Http(503)) } else { Ok(()) }
        }
    }

    #[async_trait]
    impl MangaSource for FakeCatalogue {
        fn id(&self) -> i64 {
            self.id
        }
        fn name(&self) -> &str {
            "Fake Central"
        }
        fn lang(&self) -> &str {
            "en"
        }
        async fn get_manga_details(&self, manga: &SManga) -> Result<SManga, SourceError> {
            self.log(format!("details {}", manga.url))?;
            Ok(SManga {
                description: Some("A story.".into()),
                genre: Some("Action, Drama".into()),
                status: kuro_source::model::manga::ONGOING,
                author: Some("Author A".into()),
                thumbnail_url: Some("https://img.example/cover.jpg".into()),
                ..SManga::new(manga.url.clone(), "Remote title")
            })
        }
        async fn get_chapter_list(&self, manga: &SManga) -> Result<Vec<SChapter>, SourceError> {
            self.log(format!("chapters {}", manga.url))?;
            let chapter = |n: u32| SChapter { url: format!("{}/c{n}", manga.url), name: format!("Chapter {n}"), date_upload: 1_700_000_000_000, chapter_number: n as f32, scanlator: None };
            Ok(vec![chapter(2), chapter(1)])
        }
        async fn get_page_list(&self, _chapter: &SChapter) -> Result<Vec<Page>, SourceError> {
            Ok(Vec::new())
        }
    }

    #[async_trait]
    impl CatalogueSource for FakeCatalogue {
        fn supports_latest(&self) -> bool {
            true
        }
        fn filter_list(&self) -> FilterList {
            FilterList(vec![kuro_source::Filter::check_box("Completed")])
        }
        async fn popular(&self, page: i32) -> Result<MangasPage, SourceError> {
            self.log(format!("popular {page}"))?;
            let manga = |url: &str, title: &str| SManga { thumbnail_url: Some(format!("https://img.example{url}.jpg")), ..SManga::new(url, title) };
            Ok(MangasPage { mangas: vec![manga("/m/one", "One"), manga("/m/two", "Two")], has_next_page: true })
        }
        async fn search(&self, page: i32, query: &str, filters: &FilterList) -> Result<MangasPage, SourceError> {
            self.log(format!("search {page} {query} {}", filters.len()))?;
            Ok(MangasPage { mangas: vec![SManga::new("/m/found", query)], has_next_page: false })
        }
        async fn latest(&self, page: i32) -> Result<MangasPage, SourceError> {
            self.log(format!("latest {page}"))?;
            Ok(MangasPage { mangas: Vec::new(), has_next_page: false })
        }
    }

    fn run<T>(future: impl std::future::Future<Output = T>) -> T {
        tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_time().build().expect("runtime").block_on(future)
    }

    #[test]
    fn the_rows_carry_name_language_id_and_latest_flag_and_the_directory_answers_by_id() {
        let fake = FakeCatalogue::new(41);
        let built = Builtin::new(vec![(fake, "builtin.fake", "https://fake.example")], Vec::new());
        let rows = built.sources();
        assert_eq!(rows.len(), 1);
        assert_eq!((rows[0].source.id, rows[0].source.name.as_str(), rows[0].source.lang.as_str()), (41, "Fake Central", "en"));
        assert!(rows[0].source.supports_latest, "the Latest chip follows the source");
        assert_eq!((rows[0].package.as_str(), rows[0].home_url.as_deref()), ("builtin.fake", Some("https://fake.example")));
        assert_eq!(built.filters(41).map(|f| f.len()), Some(1), "the search sheet starts from the source's filters");
        assert!(built.filters(9).is_none() && built.manga_source(9).is_none());
        assert_eq!(built.manga_source(41).map(|s| s.id()), Some(41));
        let chapters = run(built.fetch_chapters(41, &Manga { url: "/m/one".into(), ..Manga::create() }).expect("a fetch")).expect("chapters");
        assert_eq!(chapters.len(), 2);
    }

    /// The three shipped sources: two manga and one anime, with the ids the Kotlin extensions give (backups match).
    #[test]
    fn this_build_ships_weebcentral_and_comix_for_manga_and_kickassanime_for_anime() {
        let paths = kuro_testkit::TempPaths::new().unwrap();
        let client = crate::net_client::build(&NetworkPrefs::new(PrefStore::in_memory()), &paths).unwrap();
        let built = Builtin::ship(&client);
        let manga: Vec<_> = built.sources().into_iter().map(|row| (row.source.name, row.source.lang, row.source.id)).collect();
        assert_eq!(
            manga,
            [("Weeb Central".to_owned(), "en".to_owned(), 2131019126180322627), ("Comix".to_owned(), "en".to_owned(), kuro_source::generate_id("Comix", "en", 1))]
        );
        let anime: Vec<_> = built.anime_rows().into_iter().map(|row| (row.source.name, row.source.id)).collect();
        assert_eq!(anime, [("KickAssAnime".to_owned(), 2081771513481539599)]);
        let referer = built.image_headers(2131019126180322627).and_then(|h| h.get("referer").map(|v| v.to_str().unwrap().to_owned()));
        assert_eq!(referer.as_deref(), Some("https://weebcentral.com/"));
        let comix = built.image_headers(kuro_source::generate_id("Comix", "en", 1)).expect("comix headers");
        assert!(comix.get("referer").is_none() && comix.get("origin").is_none(), "the Comix image hosts refuse both");
    }

    /// A Browse grid cover carries the listed entry's source, so the image service picks that source's headers; the
    /// shared cache source the tracker covers use has none. The entry is not in the library: no cover file is written.
    #[test]
    fn a_grid_cover_of_weeb_central_goes_out_with_its_referer() {
        let paths = kuro_testkit::TempPaths::new().unwrap();
        let client = crate::net_client::build(&NetworkPrefs::new(PrefStore::in_memory()), &paths).unwrap();
        let built = Builtin::ship(&client);
        let art = crate::browse::view::remote_art("https://temp.compsci88.com/cover/normal/1.webp", 2131019126180322627);
        assert_eq!((art.entry_id, art.favorite), (0, false));
        let referer = built.image_headers(art.source_id).and_then(|h| h.get("referer").map(|v| v.to_str().unwrap().to_owned()));
        assert_eq!(referer.as_deref(), Some("https://weebcentral.com/"));
        assert!(built.image_headers(crate::entry_core::cover_view::TRACKER_COVER_SOURCE).is_none());
    }

    #[test]
    fn a_listing_maps_each_entry_to_its_library_row_and_a_failure_is_the_source_message() {
        let fake = FakeCatalogue::new(41);
        let store = Arc::new(FakeStore::default());
        let loader = CatalogueLoader { source: fake.clone(), store: store.clone() };
        let page = run(loader.load(BrowseQuery::Popular, FilterList::default(), 1)).expect("page");
        assert!(page.has_next);
        assert_eq!(page.items.iter().map(|i| (i.source_id, i.url.as_str())).collect::<Vec<_>>(), [(41, "/m/one"), (41, "/m/two")]);
        assert_eq!(page.items[0].cover.as_deref(), Some("https://img.example/m/one.jpg"));
        let found = run(loader.load(BrowseQuery::Search("needle".into()), FilterList(vec![kuro_source::Filter::check_box("A")]), 2)).expect("search");
        assert_eq!(found.items[0].title, "needle");
        assert_eq!(fake.calls.lock().unwrap().as_slice(), ["popular 1", "search 2 needle 1"]);
        fake.fail.store(true, std::sync::atomic::Ordering::SeqCst);
        let error = run(loader.load(BrowseQuery::Latest, FilterList::default(), 1)).expect_err("a failed listing");
        assert!(error.contains("503"), "{error}");
    }

    #[test]
    fn the_builtin_anime_instances_come_first_and_the_installed_ones_still_answer() {
        let fake = crate::browse::anime::fixtures::FakeSource::new(77, Vec::new());
        let built = Arc::new(Builtin::new(Vec::new(), vec![(fake, "builtin.anime", "https://anime.example")]));
        let held = crate::browse::anime::fixtures::FakeSource::new(5, Vec::new());
        struct One(Arc<dyn AnimeSource>);
        impl AnimeInstances for One {
            fn source(&self, id: i64) -> Option<Arc<dyn AnimeSource>> {
                (id == self.0.id()).then(|| Arc::clone(&self.0))
            }
        }
        let both = WithBuiltin(Arc::clone(&built), Some(Arc::new(One(held))));
        assert_eq!(both.source(77).map(|s| s.id()), Some(77));
        assert_eq!(both.source(5).map(|s| s.id()), Some(5));
        assert!(both.source(6).is_none());
        assert_eq!(built.anime_rows()[0].source.id, 77);
    }
}
