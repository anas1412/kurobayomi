//! Updates feed for both kinds: favourite entries' items fetched after they were added, uploaded after `after`,
//! newest fetch first, at most [`crate::UPDATES_LIMIT`] rows. The SQL is in `kuro-db` (the views filter favourites
//! and `date_fetch > date_added`), so the excluded-scanlator filter does not apply here (documented upstream).

use kuro_platform::kind::Kind;
use std::sync::Arc;
use std::time::Duration;

use kuro_db::handle::Db;
use rusqlite::Connection;
use kuro_db::statements_anime as anime;
use kuro_db::statements_manga::views as manga_views;
use kuro_db::subscribe::Subscription;

use super::model::UpdateRow;
use crate::error::DomainError;

/// Tables the feed reads; a write to any of them re-runs the feed.
const MANGA_TABLES: &[&str] = &["items", "entries"];
const ANIME_TABLES: &[&str] = &["items", "entries"];
const FEED_DEBOUNCE: Duration = Duration::from_millis(30);

#[derive(Clone)]
pub struct UpdatesRepo {
    db: Arc<Db>,
}

fn manga_row(r: kuro_db::statements_manga::UpdateViewRow) -> UpdateRow {
    UpdateRow {
        kind: Kind::Manga,
        entry_id: r.manga_id,
        entry_title: r.manga_title,
        item_id: r.chapter_id,
        item_name: r.chapter_name,
        scanlator: r.scanlator,
        done: r.read,
        bookmark: r.bookmark,
        fillermark: false,
        progress: r.last_page_read,
        total_seconds: 0,
        source_id: r.source,
        date_fetch: r.date_fetch,
        date_upload: r.date_upload,
        thumbnail_url: r.thumbnail_url,
        cover_last_modified: r.cover_last_modified,
    }
}

fn anime_row(r: anime::AnimeUpdateRow) -> UpdateRow {
    UpdateRow {
        kind: Kind::Anime,
        entry_id: r.anime_id,
        entry_title: r.anime_title,
        item_id: r.episode_id,
        item_name: r.episode_name,
        scanlator: r.scanlator,
        done: r.seen,
        bookmark: r.bookmark,
        fillermark: r.fillermark,
        progress: r.last_second_seen,
        total_seconds: r.total_seconds,
        source_id: r.source,
        date_fetch: r.date_fetch,
        date_upload: r.date_upload,
        thumbnail_url: r.thumbnail_url,
        cover_last_modified: r.cover_last_modified,
    }
}

/// The rows of the feed whose read (anime: seen) flag equals `done`, uploaded after `after`, at most the feed limit.
fn rows_with_read(c: &Connection, kind: Kind, done: bool, after: i64) -> rusqlite::Result<Vec<UpdateRow>> {
    let limit = crate::UPDATES_LIMIT;
    Ok(match kind {
        Kind::Manga => manga_views::get_updates_by_read_status(c, done, after, limit)?
            .into_iter()
            .map(manga_row)
            .collect(),
        Kind::Anime => anime::updates_by_seen(c, done, after, limit)?.into_iter().map(anime_row).collect(),
    })
}

fn feed_tables(kind: Kind) -> &'static [&'static str] {
    match kind {
        Kind::Manga => MANGA_TABLES,
        Kind::Anime => ANIME_TABLES,
    }
}

/// Newest fetch first; ties by item id, newest first (the feed's `date_fetch DESC, id DESC`).
fn order(mut rows: Vec<UpdateRow>) -> Vec<UpdateRow> {
    rows.sort_by(|a, b| b.date_fetch.cmp(&a.date_fetch).then(b.item_id.cmp(&a.item_id)));
    rows.truncate(crate::UPDATES_LIMIT as usize);
    rows
}

impl UpdatesRepo {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db }
    }

    /// Every feed row uploaded after `after` (epoch ms), read or not.
    pub fn recent(&self, kind: Kind, after: i64) -> Result<Vec<UpdateRow>, DomainError> {
        let limit = crate::UPDATES_LIMIT;
        Ok(order(match kind {
            Kind::Manga => self
                .db
                .read(|c| Ok(manga_views::get_recent_updates(c, after, limit)?))?
                .into_iter()
                .map(manga_row)
                .collect(),
            Kind::Anime => {
                self.db.read(|c| Ok(anime::recent_updates(c, after, limit)?))?.into_iter().map(anime_row).collect()
            }
        }))
    }

    /// The feed rows whose read (or seen) flag equals `done`, in the feed order. Anime uses its `seen` flag.
    /// Kept for parity with the Kotlin repository, which has no caller in the first release.
    pub fn with_read(&self, kind: Kind, done: bool, after: i64) -> Result<Vec<UpdateRow>, DomainError> {
        Ok(order(self.db.read(|c| Ok(rows_with_read(c, kind, done, after)?))?))
    }

    /// Live feed of [`UpdatesRepo::recent`]. It publishes after each change to the feed's tables.
    pub fn watch_recent(&self, kind: Kind, after: i64) -> Subscription<Vec<UpdateRow>> {
        let tables = match kind {
            Kind::Manga => MANGA_TABLES,
            Kind::Anime => ANIME_TABLES,
        };
        self.db.subscribe(tables, FEED_DEBOUNCE, move |c| {
            let rows: Vec<UpdateRow> = match kind {
                Kind::Manga => manga_views::get_recent_updates(c, after, crate::UPDATES_LIMIT)?
                    .into_iter()
                    .map(manga_row)
                    .collect(),
                Kind::Anime => {
                    anime::recent_updates(c, after, crate::UPDATES_LIMIT)?.into_iter().map(anime_row).collect()
                }
            };
            Ok(order(rows))
        })
    }

    /// Live feed of [`UpdatesRepo::with_read`], published again after each change to the feed's tables.
    pub fn watch_with_read(&self, kind: Kind, done: bool, after: i64) -> Subscription<Vec<UpdateRow>> {
        self.db.subscribe(feed_tables(kind), FEED_DEBOUNCE, move |c| Ok(order(rows_with_read(c, kind, done, after)?)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdb::test_db;
    use kuro_data::manga::{Manga, MangaRepo};

    fn manga(db: &Arc<Db>, title: &str, favorite: bool, date_added: i64) -> i64 {
        MangaRepo::new(db.clone())
            .insert(&Manga {
                favorite,
                source: 7,
                url: format!("u/{title}"),
                title: title.into(),
                date_added,
                ..Manga::create()
            })
            .unwrap()
    }

    fn chapter(db: &Arc<Db>, manga_id: i64, name: &str, date_fetch: i64, date_upload: i64, read: bool) -> i64 {
        db.write(|tx| {
            tx.execute(
                "INSERT INTO items(entry_id, url, name, scanlator, done, bookmark, progress, number, source_order, date_fetch, date_upload) \
                 VALUES (?1, ?2, ?3, NULL, ?4, 0, 0, 1, 0, ?5, ?6)",
                (manga_id, format!("{name}-{manga_id}-{date_fetch}"), name, read, date_fetch, date_upload),
            )?;
            Ok(tx.last_insert_rowid())
        })
        .unwrap()
    }

    fn names(rows: &[UpdateRow]) -> Vec<String> {
        rows.iter().map(|r| r.item_name.clone()).collect()
    }

    #[test]
    fn the_fixture_of_the_feed_window_yields_d_then_b() {
        let (_d, db) = test_db();
        let m = manga(&db, "M", true, 1000);
        let n = manga(&db, "N", false, 0);
        chapter(&db, m, "a", 900, 5000, false);
        chapter(&db, m, "b", 2000, 5000, false);
        chapter(&db, m, "c", 3000, 100, false);
        chapter(&db, m, "d", 4000, 6000, true);
        chapter(&db, n, "e", 9000, 9000, false);
        let repo = UpdatesRepo::new(db.clone());
        assert_eq!(names(&repo.recent(Kind::Manga, 4000).unwrap()), vec!["d", "b"]);
        assert_eq!(names(&repo.with_read(Kind::Manga, false, 4000).unwrap()), vec!["b"]);
        assert_eq!(names(&repo.with_read(Kind::Manga, true, 4000).unwrap()), vec!["d"]);
    }

    /// The anime twin reads `seen`, not `read`, and keeps the same window.
    #[test]
    fn the_anime_twin_filters_by_seen_inside_the_window() {
        let (_d, db) = crate::testdb::test_db();
        let anime = kuro_data::anime::AnimeRepo::new(db.clone())
            .insert(&kuro_data::anime::Anime {
                favorite: true,
                source: 7,
                url: "a".into(),
                title: "A".into(),
                date_added: 1000,
                ..kuro_data::anime::Anime::create()
            })
            .unwrap();
        db.write(|tx| {
            for (name, seen, date_fetch) in [("old", false, 900), ("new", false, 2000), ("done", true, 4000)] {
                tx.execute(
                    "INSERT INTO items(entry_id, url, name, done, bookmark, progress, total_seconds, number, source_order, date_fetch, date_upload, fillermark) \
                     VALUES (?1, ?2, ?3, ?4, 0, 0, 0, 1, 0, ?5, 5000, 0)",
                    (anime, format!("u-{name}"), name, seen, date_fetch),
                )?;
            }
            Ok(())
        })
        .unwrap();
        let repo = UpdatesRepo::new(db.clone());
        assert_eq!(names(&repo.with_read(Kind::Anime, false, 1000).unwrap()), vec!["new"]);
        assert_eq!(names(&repo.with_read(Kind::Anime, true, 1000).unwrap()), vec!["done"]);
    }

    #[tokio::test]
    async fn watch_with_read_publishes_the_filtered_feed() {
        let (_d, db) = test_db();
        let m = manga(&db, "M", true, 1000);
        chapter(&db, m, "b", 2000, 5000, false);
        let mut sub = UpdatesRepo::new(db.clone()).watch_with_read(Kind::Manga, false, 1000);
        let first = tokio::time::timeout(Duration::from_millis(500), sub.changed()).await.unwrap().unwrap().unwrap();
        assert_eq!(names(&first), vec!["b"]);
        chapter(&db, m, "c", 3000, 5000, false);
        let second = tokio::time::timeout(Duration::from_millis(500), sub.changed()).await.unwrap().unwrap().unwrap();
        assert_eq!(names(&second), vec!["c", "b"]);
    }

    #[test]
    fn re_adding_an_entry_hides_items_fetched_before_the_new_date_added() {
        let (_d, db) = test_db();
        let m = manga(&db, "M", true, 1000);
        chapter(&db, m, "a", 900, 5000, false);
        chapter(&db, m, "b", 2000, 5000, false);
        chapter(&db, m, "d", 4000, 6000, true);
        let repo = UpdatesRepo::new(db.clone());
        db.write(|tx| {
            tx.execute("UPDATE entries SET date_added = 3500 WHERE _id = ?1", [m])?;
            Ok(())
        })
        .unwrap();
        assert_eq!(names(&repo.recent(Kind::Manga, 4000).unwrap()), vec!["d"]);
    }

    #[test]
    fn the_feed_is_capped_at_500_rows_newest_fetch_first() {
        let (_d, db) = test_db();
        let m = manga(&db, "Big", true, 0);
        for i in 0..600 {
            chapter(&db, m, &format!("c{i}"), 1000 + i, 5000, false);
        }
        let rows = UpdatesRepo::new(db.clone()).recent(Kind::Manga, 0).unwrap();
        assert_eq!(rows.len(), 500);
        assert_eq!(rows[0].date_fetch, 1599);
        assert!(rows.windows(2).all(|w| w[0].date_fetch >= w[1].date_fetch));
    }
}
