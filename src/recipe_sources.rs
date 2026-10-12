//! Recipe sources in the app (`docs/source-recipes.md`). At start the recipes of the scrapers folder become sources: a
//! manga recipe is listed by the manga source directory the Browse pages read, an anime recipe by the anime directory
//! and the anime instances. Nothing is installed when the folder holds no recipe, so an app without recipes behaves as
//! before. A recipe that is not valid is logged and skipped.

use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use http::header::{HeaderMap, HeaderName, HeaderValue};

use kuro_domain::source::browse::BrowseQuery;
use kuro_domain::source::model::Source;
use kuro_extensions::instantiate::{RepoPlugin, instantiate_repo_plugin};
use kuro_extensions::repo::{self, RepoError};
use kuro_images::fetch::{ImageService, SourceClient, SourceHttp};
use kuro_net::NetworkClient;
use kuro_platform::paths::Paths;
use kuro_prefs::network::NetworkPrefs;
use kuro_prefs::source::SourcePrefs;
use kuro_prefs::store::PrefStore;
use kuro_source::anime::AnimeSource;
use kuro_source::filter_list::FilterList;
use kuro_source::{CatalogueSource, MangaSource};
use kuro_source_scrape::{Kind, Recipe, ScrapeAnime, ScrapeManga};

use crate::browse::anime::directory::{AnimeInstances, install_instances};
use crate::browse::directory::{DirectorySource, SourceDirectory, install};
use crate::browse::entries::FavoriteStore;
use crate::browse::listing::{ListItem, ListedPage, LoadFuture, PageLoader};

/// The package name a recipe source is listed under (the Browse pages group sources by package).
fn package_of(recipe: &Recipe) -> String {
    format!("scraper.{}", recipe.id)
}

fn listed(recipe: &Recipe, id: i64) -> DirectorySource {
    DirectorySource { source: Source::new(id, &recipe.name, &recipe.lang), package: package_of(recipe), home_url: Some(recipe.base().to_owned()) }
}

/// The anime recipe sources, as the anime pages and the anime entry page read them.
#[derive(Default)]
struct Loaded {
    anime: Vec<(Arc<dyn AnimeSource>, DirectorySource)>,
}

static LOADED: Mutex<Option<Arc<Loaded>>> = Mutex::new(None);

fn loaded() -> Arc<Loaded> {
    LOADED.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone().unwrap_or_default()
}

/// The anime recipe sources as directory rows, for the anime directory.
pub fn anime_rows() -> Vec<DirectorySource> {
    loaded().anime.iter().map(|(_, row)| row.clone()).collect()
}

/// The anime recipe sources as the Sources list shows them.
pub fn anime_list() -> Vec<Source> {
    anime_rows().into_iter().map(|row| row.source).collect()
}

/// The runnable anime recipe sources, for the anime entry page.
pub fn anime_sources() -> Vec<Arc<dyn AnimeSource>> {
    loaded().anime.iter().map(|(source, _)| Arc::clone(source)).collect()
}

struct Instances(Arc<Loaded>);

impl AnimeInstances for Instances {
    fn source(&self, id: i64) -> Option<Arc<dyn AnimeSource>> {
        self.0.anime.iter().find(|(source, _)| source.id() == id).map(|(source, _)| Arc::clone(source))
    }
}

/// The manga recipe sources as the directory of the manga Browse pages.
struct MangaDirectory {
    sources: Vec<(Arc<dyn CatalogueSource>, DirectorySource)>,
}

impl SourceDirectory for MangaDirectory {
    fn sources(&self) -> Vec<DirectorySource> {
        self.sources.iter().map(|(_, row)| row.clone()).collect()
    }

    fn loader(&self, source_id: i64) -> Option<Arc<dyn PageLoader>> {
        let (source, _) = self.sources.iter().find(|(_, row)| row.source.id == source_id)?;
        Some(Arc::new(MangaLoader { source: Arc::clone(source), store: library_store()? }))
    }

    fn filters(&self, source_id: i64) -> Option<FilterList> {
        self.sources.iter().find(|(_, row)| row.source.id == source_id).map(|(source, _)| source.filter_list())
    }

    fn manga_source(&self, source_id: i64) -> Option<Arc<dyn MangaSource>> {
        self.sources.iter().find(|(_, row)| row.source.id == source_id).map(|(source, _)| Arc::clone(source) as Arc<dyn MangaSource>)
    }
}

/// The library the listings map their entries to, built as the manga Browse screen builds its store.
fn library_store() -> Option<Arc<dyn FavoriteStore>> {
    use kuro_data::category::{CategoryKind, CategoryRepo};
    use kuro_data::manga::MangaRepo;
    use kuro_domain::kind::EntryKind;

    if !crate::data::is_installed() {
        return None;
    }
    let app = crate::data::app();
    let db = app.db(EntryKind::Manga);
    let prefs = crate::browse::prefs::BrowsePrefs::new(app.base.store().clone());
    Some(Arc::new(crate::browse::repo_store::RepoStore::new(
        MangaRepo::new(Arc::clone(&db)),
        CategoryRepo::new(db, CategoryKind::Manga),
        prefs.default_manga_category.clone(),
        Arc::new(|_| 0),
    )))
}

/// A manga source's popular, latest or search listing, each entry mapped to its library row (the manga twin of
/// `browse::anime::loader::AnimeLoader`).
pub(crate) struct MangaLoader {
    pub(crate) source: Arc<dyn CatalogueSource>,
    pub(crate) store: Arc<dyn FavoriteStore>,
}

impl PageLoader for MangaLoader {
    fn load(&self, query: BrowseQuery, filters: FilterList, page: u32) -> LoadFuture {
        let (source, store) = (Arc::clone(&self.source), Arc::clone(&self.store));
        Box::pin(async move {
            let page = i32::try_from(page).map_err(|error| error.to_string())?;
            let found = match query {
                BrowseQuery::Popular => source.popular(page).await,
                BrowseQuery::Latest => source.latest(page).await,
                BrowseQuery::Search(text) => source.search(page, &text, &filters).await,
            }
            .map_err(|error| error.to_string())?;
            let source_id = source.id();
            tokio::task::spawn_blocking(move || -> Result<ListedPage, String> {
                let mut items = Vec::with_capacity(found.mangas.len());
                for manga in &found.mangas {
                    let local = store.local_entry(source_id, &manga.url, &manga.title, manga.thumbnail_url.as_deref())?;
                    items.push(ListItem {
                        id: local.id,
                        source_id,
                        url: manga.url.clone(),
                        title: local.title,
                        favorite: local.favorite,
                        cover: manga.thumbnail_url.clone(),
                    });
                }
                Ok(ListedPage { items, has_next: found.has_next_page })
            })
            .await
            .map_err(|error| error.to_string())?
        })
    }
}

/// The app values a reload reads, set once at start: the folders, the preferences and the runtime.
static PATHS: OnceLock<Arc<dyn Paths + Send + Sync>> = OnceLock::new();
static PREFS: OnceLock<PrefStore> = OnceLock::new();
static HANDLE: OnceLock<tokio::runtime::Handle> = OnceLock::new();

/// Runs a future to its end on a plain thread, so the call works on the GTK thread and on a worker alike. `None` when the
/// app runtime is not up or the future panics.
fn block<T: Send>(future: impl Future<Output = T> + Send) -> Option<T> {
    let handle = HANDLE.get()?.clone();
    std::thread::scope(|scope| scope.spawn(move || handle.block_on(future)).join().ok())
}

/// The folder the repo links keep their copies in.
fn repo_root() -> Option<PathBuf> {
    PATHS.get().map(|paths| paths.repo_sources_dir())
}

/// The client the repo links are fetched with. It is built once and shared.
pub fn repo_http() -> reqwest::Client {
    static HTTP: OnceLock<reqwest::Client> = OnceLock::new();
    HTTP.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(concat!("Kurobayomi/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    })
    .clone()
}

/// Fetches a repo link's index and its sources into the link's folder. Blocks, so call it off the GTK thread.
pub fn sync_link(link: &str) -> Result<(), RepoError> {
    let root = repo_root().ok_or_else(|| RepoError::Write("the app folders are not set".to_owned()))?;
    let dir = repo::link_dir(&root, link);
    let http = repo_http();
    match block(repo::sync_link(&http, &dir, link)) {
        Some(result) => result.map(|_| ()),
        None => Err(RepoError::Network("the app runtime is not running".to_owned())),
    }
}

/// Deletes a removed link's saved copies. A folder that is already gone is not an error.
pub fn forget_link(link: &str) {
    let Some(root) = repo_root() else { return };
    match std::fs::remove_dir_all(repo::link_dir(&root, link)) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => tracing::warn!(%error, "could not delete the saved copies of a repo"),
    }
}

/// A recipe read from the bytes of a saved `.toml` copy.
fn recipe_from(bytes: &[u8]) -> Result<Recipe, String> {
    let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
    Recipe::parse(text).map_err(|e| e.to_string())
}

/// Makes the lists the Browse pages read: the recipes and the plug-ins, each in the list of its kind.
fn build(
    recipes: Vec<Recipe>,
    mut manga: Vec<(Arc<dyn CatalogueSource>, DirectorySource)>,
    mut anime: Vec<(Arc<dyn AnimeSource>, DirectorySource)>,
    client: &NetworkClient,
) -> (MangaDirectory, Loaded) {
    for recipe in recipes {
        let recipe = Arc::new(recipe);
        match recipe.kind {
            Kind::Manga => {
                if let Some(source) = ScrapeManga::new(client.clone(), Arc::clone(&recipe)) {
                    let row = listed(&recipe, kuro_source::MangaSource::id(&source));
                    manga.push((Arc::new(source), row));
                }
            }
            Kind::Anime => {
                if let Some(source) = ScrapeAnime::new(client.clone(), Arc::clone(&recipe)) {
                    let row = listed(&recipe, source.id());
                    anime.push((Arc::new(source), row));
                }
            }
        }
    }
    tracing::info!(manga = manga.len(), anime = anime.len(), "sources loaded");
    (MangaDirectory { sources: manga }, Loaded { anime })
}

/// Reads the recipes of the scrapers folder and the saved copies of every repo link, instantiates the plug-ins, and
/// installs the result for the Browse pages. A call replaces the last one, so a link that was added or removed shows at
/// once. Plug-ins load on the runtime, and the call waits for them, so it runs off the GTK thread after start.
pub fn reload() {
    let (Some(paths), Some(store), Some(client)) = (PATHS.get(), PREFS.get(), client()) else { return };
    let (mut recipes, problems) = kuro_source_scrape::load_dir(&paths.scrapers_dir());
    for (file, reason) in problems {
        tracing::warn!(file, "recipe skipped: {reason}");
    }
    let root = paths.repo_sources_dir();
    let links: Vec<String> = SourcePrefs::new(store.clone()).manga_extension_repos.get().into_iter().collect();
    // (id, is an anime plug-in, allowed hosts, bytes) for each plug-in of a link
    let mut plugins = Vec::new();
    for link in &links {
        let dir = repo::link_dir(&root, link);
        let Some(index) = repo::read_index(&dir) else {
            tracing::warn!(link = link.as_str(), "the saved index of a repo is missing or not valid; its sources are skipped");
            continue;
        };
        for entry in &index.sources {
            let Some(bytes) = repo::read_source(&dir, entry) else {
                tracing::warn!(id = entry.id.as_str(), "a repo source is missing or changed; it is skipped until the next check");
                continue;
            };
            if entry.file.ends_with(".wasm") {
                plugins.push((entry.id.clone(), entry.kind == repo::Kind::Anime, entry.hosts.clone(), bytes));
                continue;
            }
            match recipe_from(&bytes) {
                Ok(recipe) if recipes.iter().any(|known| known.id == recipe.id) => {
                    tracing::warn!(id = entry.id.as_str(), "repo recipe skipped: its id is already used");
                }
                Ok(recipe) => recipes.push(recipe),
                Err(reason) => tracing::warn!(id = entry.id.as_str(), "repo recipe skipped: {reason}"),
            }
        }
    }

    let (mut manga_plugins, mut anime_plugins) = (Vec::new(), Vec::new());
    for (id, anime, hosts, bytes) in plugins {
        let package = format!("repo.{id}");
        match block(instantiate_repo_plugin(bytes, hosts, anime, client.clone(), store.clone())) {
            Some(Ok(RepoPlugin::Manga(sources))) => {
                for source in sources {
                    let row = DirectorySource { source: Source::new(source.id(), source.name(), source.lang()), package: package.clone(), home_url: None };
                    manga_plugins.push((source, row));
                }
            }
            Some(Ok(RepoPlugin::Anime(sources))) => {
                for source in sources {
                    let row = DirectorySource { source: Source::new(source.id(), source.name(), source.lang()), package: package.clone(), home_url: None };
                    anime_plugins.push((source, row));
                }
            }
            Some(Err(reason)) => tracing::warn!(id = id.as_str(), "repo plug-in skipped: {reason}"),
            None => tracing::warn!(id = id.as_str(), "repo plug-in skipped: the app runtime is not running"),
        }
    }

    let by_package: HashMap<String, HeaderMap> = recipes.iter().map(|recipe| (package_of(recipe), header_map(recipe))).collect();
    let (manga, anime) = build(recipes, manga_plugins, anime_plugins, &client);
    let rows = manga.sources().into_iter().chain(anime.anime.iter().map(|(_, row)| row.clone()));
    let headers: HashMap<i64, HeaderMap> = rows.filter_map(|row| Some((row.source.id, by_package.get(&row.package)?.clone()))).collect();
    *IMAGE_HEADERS.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(headers);
    install(Arc::new(manga));
    let loaded = Arc::new(anime);
    install_instances(Arc::new(Instances(Arc::clone(&loaded))));
    *LOADED.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(loaded);
}

/// The daily check: each link whose saved index is older than a day is fetched again on a thread of its own, so the window
/// does not wait for the network. The Browse pages are reloaded when it ends.
fn schedule_daily_check(links: &[String]) {
    let Some(root) = repo_root() else { return };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|elapsed| i64::try_from(elapsed.as_secs()).ok())
        .unwrap_or_default();
    let due: Vec<String> = links.iter().filter(|link| repo::refresh_due(repo::checked_at(&repo::link_dir(&root, link)), now)).cloned().collect();
    if due.is_empty() {
        return;
    }
    std::thread::spawn(move || {
        for link in &due {
            if let Err(error) = sync_link(link) {
                tracing::warn!(link = link.as_str(), %error, "the daily repo check failed; the saved copy is kept");
            }
        }
        reload();
    });
}

/// The network client the sources and the reader's pages share, built once at start.
static CLIENT: Mutex<Option<NetworkClient>> = Mutex::new(None);

/// The shared client, `None` before [`start`] has built it.
pub fn client() -> Option<NetworkClient> {
    CLIENT.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone()
}

/// The headers each recipe source sends with its images (its `headers` without the user agent), by source id.
static IMAGE_HEADERS: Mutex<Option<HashMap<i64, HeaderMap>>> = Mutex::new(None);

fn header_map(recipe: &Recipe) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (name, value) in recipe.headers.iter().filter(|(name, _)| !name.eq_ignore_ascii_case("user-agent")) {
        if let (Ok(name), Ok(value)) = (HeaderName::from_bytes(name.as_bytes()), HeaderValue::from_str(value)) {
            map.insert(name, value);
        }
    }
    map
}

/// The image headers of a recipe source; empty for a source that is not a recipe.
pub fn image_headers(source_id: i64) -> HeaderMap {
    IMAGE_HEADERS.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).as_ref().and_then(|all| all.get(&source_id).cloned()).unwrap_or_default()
}

/// Covers and images of the recipe sources go over the shared client with the recipe's headers (a `Referer` an image
/// host asks for).
struct RecipeImageHeaders;

impl SourceHttp for RecipeImageHeaders {
    fn http_source(&self, source_id: i64) -> Option<SourceClient> {
        let headers = image_headers(source_id);
        if headers.is_empty() {
            return None;
        }
        Some(SourceClient { client: client()?, headers })
    }
}

/// Builds the shared client, installs the image service the covers draw through, and loads the sources: the recipes of
/// the scrapers folder and the repo links. Call once at start, inside the app's runtime.
pub fn start(store: &PrefStore, paths: &Arc<dyn Paths + Send + Sync>) {
    let client = match crate::net_client::build(&NetworkPrefs::new(store.clone()), paths.as_ref()) {
        Ok(client) => client,
        Err(error) => {
            tracing::warn!("the sources do not start: {error}");
            return;
        }
    };
    *CLIENT.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(client.clone());
    let _ = PATHS.set(Arc::clone(paths));
    let _ = PREFS.set(store.clone());
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        let _ = HANDLE.set(handle);
    }
    let images = Arc::clone(paths);
    let image_client = client.clone();
    tokio::spawn(async move {
        match ImageService::new(image_client, Arc::new(RecipeImageHeaders), images).await {
            Ok(service) => crate::entry_core::cover_view::install_images(Arc::new(service)),
            Err(error) => tracing::warn!("the image service does not start, covers show their placeholder: {error}"),
        }
    });
    reload();
    let links: Vec<String> = SourcePrefs::new(store.clone()).manga_extension_repos.get().into_iter().collect();
    schedule_daily_check(&links);
}

#[cfg(test)]
mod tests {
    use kuro_testkit::TempPaths;

    use super::*;

    const MANGA: &str = include_str!("../crates/kuro-source-scrape/tests/fixtures/manga/recipe.toml");
    const ANIME: &str = include_str!("../crates/kuro-source-scrape/tests/fixtures/anime/recipe.toml");

    /// The folder's recipes become sources of their kind. `build` is used rather than `start`, which installs into the
    /// process-wide registries the other tests of this binary read.
    #[test]
    fn a_manga_and_an_anime_recipe_become_listed_sources_of_their_kind() {
        let paths = TempPaths::new().unwrap();
        let dir = paths.scrapers_dir();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("example-manga.toml"), MANGA).unwrap();
        std::fs::write(dir.join("example-anime.toml"), ANIME).unwrap();
        std::fs::write(dir.join("broken.toml"), "id = \"broken\"\nwhat = 1\n").unwrap();
        let (recipes, problems) = kuro_source_scrape::load_dir(&dir);
        assert_eq!((recipes.len(), problems.len()), (2, 1));
        let client = crate::net_client::build(&NetworkPrefs::new(PrefStore::in_memory()), &paths).unwrap();
        let (manga, anime) = build(recipes, Vec::new(), Vec::new(), &client);

        let manga = manga.sources();
        assert_eq!(manga.len(), 1);
        assert_eq!((manga[0].source.name.as_str(), manga[0].source.lang.as_str()), ("Example Manga", "en"));
        assert_eq!(manga[0].source.id, kuro_source::generate_id("Example Manga", "en", 1));
        assert_eq!(manga[0].package, "scraper.example-manga");
        assert_eq!(manga[0].home_url.as_deref(), Some("https://manga.example.org"));

        let anime = Arc::new(anime);
        assert_eq!(anime.anime.iter().map(|(_, row)| row.source.name.as_str()).collect::<Vec<_>>(), ["Example Anime"]);
        let id = anime.anime[0].1.source.id;
        assert!(Instances(anime).source(id).is_some(), "the anime instance can be asked");
    }

    #[test]
    fn without_recipes_or_plug_ins_nothing_is_listed() {
        let paths = TempPaths::new().unwrap();
        let (recipes, problems) = kuro_source_scrape::load_dir(&paths.scrapers_dir());
        assert!(recipes.is_empty() && problems.is_empty());
        let client = crate::net_client::build(&NetworkPrefs::new(PrefStore::in_memory()), &paths).unwrap();
        let (manga, anime) = build(Vec::new(), Vec::new(), Vec::new(), &client);
        assert!(manga.sources().is_empty() && anime.anime.is_empty());
    }
}
