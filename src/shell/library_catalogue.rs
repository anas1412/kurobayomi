//! The library behind the desktop search provider (T-53a-066): every manga and anime title of the library that contains
//! all the search terms. [`serve`] registers the provider once the window exists.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;

use kuro_data::anime::AnimeRepo;
use kuro_data::manga::MangaRepo;
use kuro_db::handle::Db;

use kuro_platform::kind::Kind;
use super::search_provider::{self, Catalogue, Hit, Provider};

/// The library databases as a [`Catalogue`].
pub struct LibraryCatalogue {
    manga: Arc<Db>,
    anime: Arc<Db>,
}

impl LibraryCatalogue {
    pub fn new(manga: Arc<Db>, anime: Arc<Db>) -> Self {
        Self { manga, anime }
    }

    /// Every library title once (a row exists per category).
    fn hits(&self) -> Vec<Hit> {
        let mut hits = Vec::new();
        match MangaRepo::new(self.manga.clone()).library() {
            Ok(rows) => hits.extend(rows.into_iter().map(|r| Hit { kind: Kind::Manga, entry_id: r.manga.id, title: r.manga.title })),
            Err(e) => tracing::warn!("the search provider could not read the manga library: {e}"),
        }
        match AnimeRepo::new(self.anime.clone()).library() {
            Ok(rows) => hits.extend(rows.into_iter().map(|r| Hit { kind: Kind::Anime, entry_id: r.anime.id, title: r.anime.title })),
            Err(e) => tracing::warn!("the search provider could not read the anime library: {e}"),
        }
        hits
    }
}

/// The titles that contain every term (case-insensitive), once each. A title that starts with the first term comes first,
/// then the rest by title.
pub fn matching(hits: Vec<Hit>, terms: &[String]) -> Vec<Hit> {
    let terms: Vec<String> = terms.iter().map(|t| t.trim().to_lowercase()).filter(|t| !t.is_empty()).collect();
    let mut seen = HashSet::new();
    let mut found: Vec<(bool, String, Hit)> = hits
        .into_iter()
        .filter(|h| seen.insert(h.id()))
        .filter_map(|h| {
            let title = h.title.to_lowercase();
            terms.iter().all(|t| title.contains(t.as_str())).then(|| (!terms.first().is_some_and(|t| title.starts_with(t.as_str())), title, h))
        })
        .collect();
    found.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
    found.into_iter().map(|(_, _, h)| h).collect()
}

impl Catalogue for LibraryCatalogue {
    fn search(&self, terms: &[String]) -> Vec<Hit> {
        matching(self.hits(), terms)
    }

    fn get(&self, id: &str) -> Option<Hit> {
        let (kind, entry_id) = id.split_once(':')?;
        let entry_id: i64 = entry_id.parse().ok()?;
        match kind {
            "manga" => MangaRepo::new(self.manga.clone()).by_id(entry_id).ok().map(|m| Hit { kind: Kind::Manga, entry_id, title: m.title }),
            "anime" => AnimeRepo::new(self.anime.clone()).by_id(entry_id).ok().map(|a| Hit { kind: Kind::Anime, entry_id, title: a.title }),
            _ => None,
        }
    }
}

thread_local! {
    /// The registered provider; dropping it would take it off the bus.
    static PROVIDER: RefCell<Option<Provider>> = const { RefCell::new(None) };
}

/// Registers the provider over the open library, once. Call it after the application is registered.
pub fn serve(app: &gtk::Application, manga: Arc<Db>, anime: Arc<Db>) {
    PROVIDER.with(|slot| {
        if slot.borrow().is_none() {
            *slot.borrow_mut() = search_provider::serve(app, Rc::new(LibraryCatalogue::new(manga, anime)));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(kind: Kind, entry_id: i64, title: &str) -> Hit {
        Hit { kind, entry_id, title: title.into() }
    }

    fn terms(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn every_term_must_match_and_a_prefix_match_comes_first() {
        let library = vec![hit(Kind::Anime, 1, "The One Piece"), hit(Kind::Manga, 2, "One Punch Man"), hit(Kind::Anime, 3, "Bleach")];
        let titles: Vec<String> = matching(library.clone(), &terms(&["one"])).into_iter().map(|h| h.title).collect();
        assert_eq!(titles, ["One Punch Man", "The One Piece"]);
        assert_eq!(matching(library, &terms(&["one", "PIECE"])).len(), 1);
    }

    #[test]
    fn a_title_in_two_categories_is_listed_once() {
        let library = vec![hit(Kind::Manga, 2, "Berserk"), hit(Kind::Manga, 2, "Berserk")];
        assert_eq!(matching(library, &terms(&["ber"])).len(), 1);
    }

    #[test]
    fn an_empty_library_has_no_results_and_no_title_for_an_id() {
        let dir = tempfile::TempDir::new().unwrap();
        let (manga_path, anime_path) = (dir.path().join("m.db"), dir.path().join("a.db"));
        let manga = Arc::new(Db::new(&manga_path, kuro_db::schema_manga::open(&manga_path).unwrap()).unwrap());
        let anime = Arc::new(Db::new(&anime_path, kuro_db::schema_anime::open(&anime_path).unwrap()).unwrap());
        let catalogue = LibraryCatalogue::new(manga, anime);
        assert!(catalogue.search(&terms(&["x"])).is_empty());
        assert_eq!(catalogue.get("anime:5"), None);
        assert_eq!(catalogue.get("bogus"), None);
    }
}
