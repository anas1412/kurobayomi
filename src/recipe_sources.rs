//! Recipe sources in the app (`docs/source-recipes.md`). At start the recipes of the scrapers folder become sources: a
//! manga recipe is listed by the manga source directory the Browse pages read, an anime recipe by the anime directory
//! and the anime instances. Nothing is installed when the folder holds no recipe, so an app without recipes behaves as
//! before. A recipe that is not valid is logged and skipped.

use std::sync::{Arc, Mutex};

use kuro_domain::source::browse::BrowseQuery;
use kuro_domain::source::model::Source;
use kuro_net::NetworkClient;
use kuro_platform::paths::Paths;
use kuro_prefs::network::NetworkPrefs;
use kuro_prefs::store::PrefStore;
use kuro_source::anime::AnimeSource;
use kuro_source::filter_list::FilterList;
use kuro_source::CatalogueSource;
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
    anime: Vec<(Arc<ScrapeAnime>, DirectorySource)>,
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
    loaded().anime.iter().map(|(source, _)| Arc::clone(source) as Arc<dyn AnimeSource>).collect()
}

struct Instances(Arc<Loaded>);

impl AnimeInstances for Instances {
    fn source(&self, id: i64) -> Option<Arc<dyn AnimeSource>> {
        self.0.anime.iter().find(|(source, _)| source.id() == id).map(|(source, _)| Arc::clone(source) as Arc<dyn AnimeSource>)
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
struct MangaLoader {
    source: Arc<dyn CatalogueSource>,
    store: Arc<dyn FavoriteStore>,
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

/// The sources of the recipes: the manga directory and the anime ones, each only when there is one.
fn build(recipes: Vec<Recipe>, client: &NetworkClient) -> (Option<MangaDirectory>, Option<Loaded>) {
    let mut manga: Vec<(Arc<dyn CatalogueSource>, DirectorySource)> = Vec::new();
    let mut anime = Vec::new();
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
    tracing::info!(manga = manga.len(), anime = anime.len(), "recipe sources loaded");
    ((!manga.is_empty()).then_some(MangaDirectory { sources: manga }), (!anime.is_empty()).then_some(Loaded { anime }))
}

/// Hands the sources to the Browse pages.
fn register(recipes: Vec<Recipe>, client: &NetworkClient) {
    let (manga, anime) = build(recipes, client);
    if let Some(directory) = manga {
        install(Arc::new(directory));
    }
    if let Some(loaded) = anime {
        let loaded = Arc::new(loaded);
        install_instances(Arc::new(Instances(Arc::clone(&loaded))));
        *LOADED.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(loaded);
    }
}

/// Reads the recipes of the scrapers folder and registers their sources. Call once at start.
pub fn start(store: &PrefStore, paths: &dyn Paths) {
    let (recipes, problems) = kuro_source_scrape::load_dir(&paths.scrapers_dir());
    for (file, reason) in problems {
        tracing::warn!(file, "recipe skipped: {reason}");
    }
    if recipes.is_empty() {
        return;
    }
    match crate::net_client::build(&NetworkPrefs::new(store.clone()), paths) {
        Ok(client) => register(recipes, &client),
        Err(error) => tracing::warn!("recipe sources do not start: {error}"),
    }
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
        let (manga, anime) = build(recipes, &client);

        let manga = manga.expect("a manga directory").sources();
        assert_eq!(manga.len(), 1);
        assert_eq!((manga[0].source.name.as_str(), manga[0].source.lang.as_str()), ("Example Manga", "en"));
        assert_eq!(manga[0].source.id, kuro_source::generate_id("Example Manga", "en", 1));
        assert_eq!(manga[0].package, "scraper.example-manga");
        assert_eq!(manga[0].home_url.as_deref(), Some("https://manga.example.org"));

        let anime = Arc::new(anime.expect("anime sources"));
        assert_eq!(anime.anime.iter().map(|(_, row)| row.source.name.as_str()).collect::<Vec<_>>(), ["Example Anime"]);
        let id = anime.anime[0].1.source.id;
        assert!(Instances(anime).source(id).is_some(), "the anime instance can be asked");
    }

    #[test]
    fn without_recipes_nothing_is_built() {
        let paths = TempPaths::new().unwrap();
        let (recipes, problems) = kuro_source_scrape::load_dir(&paths.scrapers_dir());
        assert!(recipes.is_empty() && problems.is_empty());
        let client = crate::net_client::build(&NetworkPrefs::new(PrefStore::in_memory()), &paths).unwrap();
        let (manga, anime) = build(Vec::new(), &client);
        assert!(manga.is_none() && anime.is_none());
    }
}
