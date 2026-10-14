//! Recipe sources in the app (`docs/source-recipes.md`). At start the recipes of the scrapers folder become sources: each
//! is installed into the Browse source directory of its kind. Nothing is installed when the folder holds no recipe, so an app without recipes behaves as
//! before. A recipe that is not valid is logged and skipped.

use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex, OnceLock};
use std::time::Duration;

use http::header::{HeaderMap, HeaderName, HeaderValue};
use tokio::sync::watch;

use kuro_domain::source::model::Source;
use kuro_extensions::instantiate::{RepoPlugin, instantiate_repo_plugin};
use kuro_extensions::repo::{self, RepoError};
use kuro_images::fetch::{ImageService, SourceClient, SourceHttp};
use kuro_net::NetworkClient;
use kuro_platform::paths::Paths;
use kuro_prefs::network::NetworkPrefs;
use kuro_prefs::source::SourcePrefs;
use kuro_prefs::store::PrefStore;
use kuro_source::CatalogueSource;
use kuro_source::anime::AnimeSource;
use kuro_platform::kind::Kind;
use kuro_source_scrape::{Recipe, ScrapeAnime, ScrapeManga};

use crate::browse::directory::{Catalogues, DirectorySource, SourceDirectory, install};
use crate::browse::loader::Catalogue;

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

/// The runnable anime recipe sources, for the anime entry page.
pub fn anime_sources() -> Vec<Arc<dyn AnimeSource>> {
    loaded().anime.iter().map(|(source, _)| Arc::clone(source)).collect()
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

/// The saved index of a repo link, when it has one.
pub fn saved_index(link: &str) -> Option<repo::Index> {
    repo::read_index(&repo::link_dir(&repo_root()?, link))
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

/// Fetches a repo link's index and its sources into the link's folder and returns how many sources it lists. Blocks, so
/// call it off the GTK thread.
pub fn sync_link(link: &str) -> Result<usize, RepoError> {
    let root = repo_root().ok_or_else(|| RepoError::Write("the app folders are not set".to_owned()))?;
    let dir = repo::link_dir(&root, link);
    let http = repo_http();
    match block(repo::sync_link(&http, &dir, link)) {
        Some(result) => result.map(|index| index.sources.len()),
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
) -> (Catalogues, Loaded) {
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
    let manga = manga.into_iter().map(|(source, row)| (Catalogue::Manga(source), row)).collect();
    (Catalogues::new(Kind::Manga, manga), Loaded { anime })
}

/// Whether the sources are being (re)loaded, and a counter of finished loads. The Browse pages follow both: they show a
/// spinner while loading and rebuild their lists when the counter moves.
struct LoadState {
    pending: Mutex<usize>,
    ready: watch::Sender<bool>,
    changes: watch::Sender<u64>,
}

impl LoadState {
    fn new() -> Self {
        Self { pending: Mutex::new(0), ready: watch::channel(true).0, changes: watch::channel(0).0 }
    }

    /// A load starts. The state is "loading" until every started load has ended.
    fn begin(&self) {
        let mut pending = self.pending.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        *pending += 1;
        self.ready.send_replace(false);
    }

    /// A load ended and its result is installed. The change goes out before "ready", so a page that wakes on "ready"
    /// already holds the new lists.
    fn end(&self) {
        let mut pending = self.pending.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        *pending = pending.saturating_sub(1);
        self.changes.send_modify(|count| *count += 1);
        if *pending == 0 {
            self.ready.send_replace(true);
        }
    }
}

static LOAD_STATE: LazyLock<LoadState> = LazyLock::new(LoadState::new);

/// `false` while the sources load (the Sources tabs show "Loading sources…"), `true` once they are listed.
pub fn ready() -> watch::Receiver<bool> {
    LOAD_STATE.ready.subscribe()
}

/// Tells the Browse pages to read their source lists again, for a change that is not a load (a source's badge).
pub fn notify_changed() {
    LOAD_STATE.changes.send_modify(|count| *count += 1);
}

/// A counter that moves each time a load has installed new sources. Pages read their lists again when it changes.
pub fn changes() -> watch::Receiver<u64> {
    LOAD_STATE.changes.subscribe()
}

/// `(source id, "Name (LANG)")` of the loaded repository sources of a kind (recipes and plug-ins): the name each source's
/// download folder is made from, for the download indexes.
pub fn folder_names(kind: Kind) -> Vec<(i64, String)> {
    let rows = match kind {
        Kind::Manga => crate::browse::directory::current(Kind::Manga).sources(),
        Kind::Anime => anime_rows(),
    };
    names_of(rows)
}

fn names_of(rows: Vec<DirectorySource>) -> Vec<(i64, String)> {
    rows.into_iter().map(|row| (row.source.id, kuro_source::display_name(&row.source.name, &row.source.lang))).collect()
}

/// Runs `rescan` on the current runtime once the first load of the sources has ended, and again after each later load,
/// each time once no load is running. A download index follows the loads through this, so the downloads of the
/// repository sources are recognised at start and after a repository is added. Nothing runs outside a runtime.
pub fn follow_loads<F, Fut>(rescan: F)
where
    F: FnMut() -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        tracing::warn!("the download index does not follow the source loads: no runtime");
        return;
    };
    handle.spawn(follow(ready(), changes(), rescan));
}

async fn follow<F, Fut>(mut ready: watch::Receiver<bool>, mut changes: watch::Receiver<u64>, mut rescan: F)
where
    F: FnMut() -> Fut,
    Fut: Future<Output = ()>,
{
    // "ready" is true before the first load starts, so the counter tells whether a load has ended yet
    let loaded = *changes.borrow_and_update() > 0;
    if !loaded && changes.changed().await.is_err() {
        return;
    }
    loop {
        let idle = ready.wait_for(|loading_done| *loading_done).await.is_ok();
        if !idle {
            return;
        }
        // this rescan reads the sources of every load that has ended so far
        changes.mark_unchanged();
        rescan().await;
        if changes.changed().await.is_err() {
            return;
        }
    }
}

/// Ends the load it was made for, also when the load panics, so the spinner never stays.
struct EndOnDrop;

impl Drop for EndOnDrop {
    fn drop(&mut self) {
        LOAD_STATE.end();
    }
}

/// One load at a time, so two quick reloads (an add, then a refresh) install in the order they were asked.
static RELOAD_LOCK: Mutex<()> = Mutex::new(());

/// Reloads the sources on a worker thread and returns at once; never blocks the GTK thread. The Browse pages follow
/// [`ready`] and [`changes`]. A plug-in that does not load is named in a toast.
pub fn reload() {
    LOAD_STATE.begin();
    let spawned = std::thread::Builder::new().name("kuro-sources".to_owned()).spawn(|| {
        let _end = EndOnDrop;
        let _one_at_a_time = RELOAD_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let failed = load_sources();
        if !failed.is_empty() {
            crate::toast::notify(failed_text(&failed));
        }
    });
    if let Err(error) = spawned {
        tracing::warn!(%error, "the sources could not be loaded: no thread");
        LOAD_STATE.end();
    }
}

/// The toast naming the sources that did not load.
fn failed_text(ids: &[String]) -> String {
    format!("Could not load: {}", ids.join(", "))
}

/// Reads the recipes of the scrapers folder and the saved copies of every repo link, instantiates the plug-ins, and
/// installs the result for the Browse pages. A call replaces the last one, so a link that was added or removed shows at
/// once. It waits for the plug-ins to compile, so only [`reload`] calls it, on a worker thread. Returns the ids of the
/// sources that could not be loaded.
fn load_sources() -> Vec<String> {
    let mut failed = Vec::new();
    let (Some(paths), Some(store), Some(client)) = (PATHS.get(), PREFS.get(), client()) else { return failed };
    let (mut recipes, problems) = kuro_source_scrape::load_dir(&paths.scrapers_dir());
    for (file, reason) in problems {
        tracing::warn!(file, "recipe skipped: {reason}");
    }
    let root = paths.repo_sources_dir();
    let links: Vec<String> = SourcePrefs::new(store.clone()).manga_extension_repos.get().into_iter().collect();
    // (id, kind, allowed hosts, bytes) for each plug-in of a link
    let mut plugins = Vec::new();
    // the saved icon of each repo package ("scraper.<id>" or "repo.<id>"), as the rows name their packages
    let mut icons: HashMap<String, PathBuf> = HashMap::new();
    // the status each repo package's index gives it, keyed like `icons`
    let mut statuses: HashMap<String, repo::Status> = HashMap::new();
    for link in &links {
        let dir = repo::link_dir(&root, link);
        let Some(index) = repo::read_index(&dir) else {
            tracing::warn!(link = link.as_str(), "the saved index of a repo is missing or not valid; its sources are skipped");
            failed.push(link.clone());
            continue;
        };
        for entry in &index.sources {
            let Some(bytes) = repo::read_source(&dir, entry) else {
                tracing::warn!(id = entry.id.as_str(), "a repo source is missing or changed; it is skipped until the next check");
                failed.push(entry.id.clone());
                continue;
            };
            let icon = repo::read_icon(&dir, entry);
            if entry.file.ends_with(".wasm") {
                if let Some(path) = icon {
                    icons.insert(format!("repo.{}", entry.id), path);
                }
                statuses.insert(format!("repo.{}", entry.id), entry.status);
                plugins.push((entry.id.clone(), entry.kind, entry.hosts.clone(), bytes));
                continue;
            }
            match recipe_from(&bytes) {
                Ok(recipe) if recipes.iter().any(|known| known.id == recipe.id) => {
                    tracing::warn!(id = entry.id.as_str(), "repo recipe skipped: its id is already used");
                }
                Ok(recipe) => {
                    if let Some(path) = icon {
                        icons.insert(package_of(&recipe), path);
                    }
                    statuses.insert(package_of(&recipe), entry.status);
                    recipes.push(recipe);
                }
                Err(reason) => {
                    tracing::warn!(id = entry.id.as_str(), "repo recipe skipped: {reason}");
                    failed.push(entry.id.clone());
                }
            }
        }
    }

    let (mut manga_plugins, mut anime_plugins) = (Vec::new(), Vec::new());
    // the plug-ins compile side by side (the compile cache makes a later start fast)
    let ids: Vec<String> = plugins.iter().map(|(id, ..)| id.clone()).collect();
    let loads = plugins.into_iter().map(|(_id, kind, hosts, bytes)| instantiate_repo_plugin(bytes, hosts, kind, client.clone(), store.clone()));
    let results = block(futures::future::join_all(loads));
    for (index, id) in ids.iter().enumerate() {
        let package = format!("repo.{id}");
        match results.as_ref().map(|all| &all[index]) {
            Some(Ok(RepoPlugin::Manga(sources))) => {
                for source in sources {
                    let row = DirectorySource { source: Source::new(source.id(), source.name(), source.lang()), package: package.clone(), home_url: None };
                    manga_plugins.push((Arc::clone(source), row));
                }
            }
            Some(Ok(RepoPlugin::Anime(sources))) => {
                for source in sources {
                    let row = DirectorySource { source: Source::new(source.id(), source.name(), source.lang()), package: package.clone(), home_url: None };
                    anime_plugins.push((Arc::clone(source), row));
                }
            }
            Some(Err(reason)) => {
                tracing::warn!(id = id.as_str(), "repo plug-in skipped: {reason}");
                failed.push(id.clone());
            }
            None => {
                tracing::warn!(id = id.as_str(), "repo plug-in skipped: the app runtime is not running");
                failed.push(id.clone());
            }
        }
    }

    let by_package: HashMap<String, HeaderMap> = recipes.iter().map(|recipe| (package_of(recipe), header_map(recipe))).collect();
    let (manga, anime) = build(recipes, manga_plugins, anime_plugins, &client);
    let rows: Vec<DirectorySource> = manga.sources().into_iter().chain(anime.anime.iter().map(|(_, row)| row.clone())).collect();
    let headers: HashMap<i64, HeaderMap> = rows.iter().filter_map(|row| Some((row.source.id, by_package.get(&row.package)?.clone()))).collect();
    let icon_paths: HashMap<i64, PathBuf> = rows.iter().filter_map(|row| Some((row.source.id, icons.get(&row.package)?.clone()))).collect();
    *IMAGE_HEADERS.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(headers);
    *ICON_PATHS.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(icon_paths);
    let statuses: HashMap<i64, repo::Status> = rows.iter().filter_map(|row| Some((row.source.id, *statuses.get(&row.package)?))).collect();
    *STATUSES.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(statuses);
    install(Kind::Manga, manga);
    let runnable = anime.anime.iter().map(|(source, row)| (Catalogue::Anime(Arc::clone(source)), row.clone())).collect();
    install(Kind::Anime, Catalogues::new(Kind::Anime, runnable));
    *LOADED.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(Arc::new(anime));
    failed
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

/// The saved PNG icon of each repo source, by source id. Set by each load with the rows of the repo sources.
static ICON_PATHS: Mutex<Option<HashMap<i64, PathBuf>>> = Mutex::new(None);

static STATUSES: Mutex<Option<HashMap<i64, repo::Status>>> = Mutex::new(None);

/// The status a repo source's index gives it; `None` for a source outside any repo.
pub fn status(source_id: i64) -> Option<repo::Status> {
    STATUSES.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).as_ref()?.get(&source_id).copied()
}

/// The saved PNG icon of a repo source (recipe or plug-in), `None` for a source without one: its row keeps the letter tile.
pub fn icon_path(source_id: i64) -> Option<PathBuf> {
    ICON_PATHS.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).as_ref()?.get(&source_id).cloned()
}

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
    kuro_extensions::instantiate::set_compile_cache_dir(paths.wasm_cache_dir());
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

        assert_eq!(anime.anime.iter().map(|(_, row)| row.source.name.as_str()).collect::<Vec<_>>(), ["Example Anime"]);
        assert_eq!(anime.anime[0].0.id(), anime.anime[0].1.source.id, "the anime row names its runnable source");
    }

    #[test]
    fn the_state_is_loading_from_the_first_begin_to_the_last_end_and_each_end_moves_the_counter() {
        let state = LoadState::new();
        let (ready, changes) = (state.ready.subscribe(), state.changes.subscribe());
        assert!(*ready.borrow(), "nothing is loading before a load starts");
        state.begin();
        state.begin();
        assert!(!*ready.borrow());
        state.end();
        assert!(!*ready.borrow(), "one load is still running");
        assert_eq!(*changes.borrow(), 1);
        state.end();
        assert!(*ready.borrow(), "ready again after the last end");
        assert_eq!(*changes.borrow(), 2);
        state.end();
        assert!(*ready.borrow(), "an extra end does not break the state");
    }

    /// A chapter downloaded from a repository source sits in `<root>/Name (LANG)/<title>/<chapter>`: the names the
    /// indexes get match that folder.
    #[test]
    fn the_download_index_recognises_the_folder_of_a_repository_source() {
        let recipe = recipe_from(MANGA.as_bytes()).unwrap();
        let id = kuro_source::generate_id("Example Manga", "en", 1);
        let names = names_of(vec![listed(&recipe, id)]);
        assert_eq!(names, [(id, "Example Manga (EN)".to_owned())]);
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("Example Manga (EN)").join("Solo").join("Chapter 1")).unwrap();
        let tree = kuro_downloads::index::scan(Kind::Manga, root.path(), &names);
        assert!(tree.sources[&id].entries["Solo"].units.contains("Chapter 1"));
        assert!(kuro_downloads::index::scan(Kind::Manga, root.path(), &[]).sources.is_empty(), "without the repository source the folder is unknown");
    }

    /// Lets the spawned follower run until it waits again (the test runtime has one thread).
    async fn settle() {
        for _ in 0..20 {
            tokio::task::yield_now().await;
        }
    }

    #[tokio::test]
    async fn the_indexes_rescan_after_the_first_load_and_after_each_later_one_once_no_load_runs() {
        let state = LoadState::new();
        let (tx, mut rescans) = tokio::sync::mpsc::unbounded_channel();
        let task = tokio::spawn(follow(state.ready.subscribe(), state.changes.subscribe(), move || {
            let tx = tx.clone();
            async move {
                let _ = tx.send(());
            }
        }));
        settle().await;
        assert!(rescans.try_recv().is_err(), "no rescan before the first load ends, though nothing is loading yet");
        state.begin();
        state.end();
        settle().await;
        assert!(rescans.try_recv().is_ok(), "the first load ended");
        state.begin();
        state.begin();
        state.end();
        settle().await;
        assert!(rescans.try_recv().is_err(), "one load still runs");
        state.end();
        settle().await;
        assert!(rescans.try_recv().is_ok(), "the last load ended");
        assert!(rescans.try_recv().is_err(), "one rescan per wake");
        task.abort();
    }

    #[tokio::test]
    async fn a_follower_that_starts_after_the_first_load_rescans_at_once() {
        let state = LoadState::new();
        state.begin();
        state.end();
        let (tx, mut rescans) = tokio::sync::mpsc::unbounded_channel();
        let task = tokio::spawn(follow(state.ready.subscribe(), state.changes.subscribe(), move || {
            let tx = tx.clone();
            async move {
                let _ = tx.send(());
            }
        }));
        settle().await;
        assert!(rescans.try_recv().is_ok());
        task.abort();
    }

    #[test]
    fn the_toast_for_sources_that_did_not_load_names_them() {
        assert_eq!(failed_text(&["a".to_owned(), "b".to_owned()]), "Could not load: a, b");
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
