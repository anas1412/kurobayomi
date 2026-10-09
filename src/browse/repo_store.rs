//! `FavoriteStore` over the manga repositories: the production side of `entries`. Every write names the columns it sets
//! (`MangaUpdate` keeps the rest), so chapter flags written by the default-flags step survive (B-EC-12).

use std::collections::BTreeSet;
use std::sync::Arc;

use kuro_data::category::CategoryRepo;
use kuro_data::manga::{Manga, MangaRepo, MangaUpdate};
use kuro_domain::entries::network_to_local::network_to_local_manga;
use kuro_prefs::pref::Pref;
use tokio::sync::watch;

use super::entries::{CategoryChoice, FavoriteStore, LocalEntry};
use super::favorite::FavoriteWrite;

/// Deletes the cached cover files of an entry and returns how many went (the cover cache is the image layer's).
pub type CoverClearer = Arc<dyn Fn(i64) -> usize + Send + Sync>;

pub struct RepoStore {
    manga: MangaRepo,
    categories: CategoryRepo,
    default_category: Pref<i32>,
    clear_cover: CoverClearer,
}

impl RepoStore {
    pub fn new(manga: MangaRepo, categories: CategoryRepo, default_category: Pref<i32>, clear_cover: CoverClearer) -> Self {
        Self { manga, categories, default_category, clear_cover }
    }
}

fn text(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn urls(list: &[Manga]) -> BTreeSet<String> {
    list.iter().map(|m| m.url.clone()).collect()
}

impl FavoriteStore for RepoStore {
    fn local_entry(&self, source_id: i64, url: &str, title: &str, cover: Option<&str>) -> Result<LocalEntry, String> {
        let listed = Manga {
            source: source_id,
            url: url.to_owned(),
            title: title.to_owned(),
            thumbnail_url: cover.map(str::to_owned),
            ..Manga::create()
        };
        let local = network_to_local_manga(&self.manga, &listed).map_err(text)?;
        Ok(LocalEntry { id: local.id, favorite: local.favorite, title: local.title })
    }

    fn duplicates(&self, id: i64, title: &str) -> Result<Vec<i64>, String> {
        Ok(self.manga.duplicates(id, title).map_err(text)?.into_iter().map(|m| m.id).collect())
    }

    fn categories(&self) -> Result<Vec<CategoryChoice>, String> {
        let all = self.categories.all().map_err(text)?;
        Ok(all.into_iter().filter(|c| c.id != 0).map(|c| CategoryChoice { id: c.id, name: c.name }).collect())
    }

    fn default_category(&self) -> i64 {
        i64::from(self.default_category.get())
    }

    fn write_favorite(&self, id: i64, write: FavoriteWrite) -> Result<(), String> {
        let update = MangaUpdate { favorite: Some(write.favorite), date_added: Some(write.date_added), ..MangaUpdate::new(id) };
        self.manga.update(&update).map_err(text)
    }

    fn set_categories(&self, id: i64, ids: &[i64]) -> Result<(), String> {
        self.manga.set_categories(id, ids).map_err(text)
    }

    fn clear_cover(&self, id: i64) -> usize {
        (self.clear_cover)(id)
    }

    fn stamp_cover(&self, id: i64, now_ms: i64) -> Result<(), String> {
        let update = MangaUpdate { cover_last_modified: Some(now_ms), ..MangaUpdate::new(id) };
        self.manga.update(&update).map_err(text)
    }

    /// Needs a tokio runtime: the follower task that republishes the subscription runs on it.
    fn favorite_urls(&self, source_id: i64) -> watch::Receiver<BTreeSet<String>> {
        let mut subscription = self.manga.subscribe_favorites_by_source(source_id);
        let initial = subscription.current().and_then(|p| p.ok()).map(|list| urls(&list)).unwrap_or_default();
        let (tx, rx) = watch::channel(initial);
        tokio::spawn(async move {
            while let Some(published) = subscription.changed().await {
                let Ok(list) = published else { continue };
                if tx.send(urls(&list)).is_err() {
                    break;
                }
            }
        });
        rx
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use kuro_db::handle::Db;
    use kuro_prefs::store::PrefStore;

    use super::*;
    use crate::browse::entries::{add_entry, remove_entry};

    fn store(deleted: Arc<AtomicUsize>) -> (tempfile::TempDir, RepoStore, MangaRepo) {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("tachiyomi.db");
        let writer = kuro_db::schema_manga::open(&path).expect("schema");
        let db = Arc::new(Db::new(&path, writer).expect("db"));
        let manga = MangaRepo::new(Arc::clone(&db));
        let categories = CategoryRepo::new(db, kuro_data::category::CategoryKind::Manga);
        let prefs = PrefStore::in_memory().pref("default_category", 0_i32);
        let clearer: CoverClearer = Arc::new(move |_| deleted.load(Ordering::SeqCst));
        (dir, RepoStore::new(manga.clone(), categories, prefs, clearer), manga)
    }

    #[test]
    fn an_unknown_entry_is_inserted_as_a_non_favourite_and_found_again() {
        let (_dir, store, manga) = store(Arc::default());
        let first = store.local_entry(5, "/series/1", "One", Some("http://c/1.png")).expect("entry");
        assert!(!first.favorite);
        let again = store.local_entry(5, "/series/1", "Renamed", None).expect("entry");
        assert_eq!(again.id, first.id, "the same (url, source) is one row");
        assert_eq!(manga.by_id(first.id).expect("row").thumbnail_url.as_deref(), Some("http://c/1.png"));
    }

    #[test]
    fn add_and_remove_write_only_their_columns_and_stamp_the_cover_when_files_went() {
        let deleted = Arc::new(AtomicUsize::new(0));
        let (_dir, store, manga) = store(Arc::clone(&deleted));
        let entry = store.local_entry(5, "/series/1", "One", None).expect("entry");
        manga.set_chapter_flags_masked(entry.id, 0b1010, 0b1111).expect("flags");

        add_entry(&store, entry.id, 4_000);
        let added = manga.by_id(entry.id).expect("row");
        assert!(added.favorite);
        assert_eq!(added.date_added, 4_000);
        assert_eq!(added.chapter_flags & 0b1111, 0b1010, "the flags written earlier survive");

        deleted.store(1, Ordering::SeqCst);
        remove_entry(&store, entry.id, 9_000).expect("removed");
        let removed = manga.by_id(entry.id).expect("row");
        assert!(!removed.favorite);
        assert_eq!(removed.date_added, 0);
        assert_eq!(removed.cover_last_modified, 9_000, "a deleted cover file stamps the row");
        assert_eq!(removed.chapter_flags & 0b1111, 0b1010);
    }

    #[test]
    fn the_favourite_urls_of_a_source_follow_the_library() {
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_time().build().expect("runtime");
        let _entered = rt.enter();
        let (_dir, store, _manga) = store(Arc::default());
        let entry = store.local_entry(5, "/series/1", "One", None).expect("entry");
        let mut urls = store.favorite_urls(5);
        assert!(urls.borrow().is_empty());
        add_entry(&store, entry.id, 1);
        let seen = rt.block_on(async { tokio::time::timeout(Duration::from_secs(3), urls.wait_for(|set| set.contains("/series/1"))).await });
        assert!(matches!(seen, Ok(Ok(_))), "the added entry reaches the follower");
    }
}
