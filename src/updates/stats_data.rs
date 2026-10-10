//! The statistics aggregation of the Statistics page (T-42a-040, T-42a-041, T-42a-044): one SQL statement per section
//! over the library of one kind, so a failing section shows "N/A" while the others keep their values (EC-42-S-06).
//! The pure shapes and the card layout are in [`stats`]; this file only reads the database.
//!
//! Two sections are narrower than the Kotlin ones, by design of this build:
//! - "In global update" counts the library entries whose update strategy is always-update; the include and exclude
//!   categories and the restriction tokens are applied by the library update, not read here.
//! - Trackers are `Failed` while no tracker service is reachable from the screens (a missing service is a section
//!   error, not a panic).

use std::sync::Arc;

use kuro_db::error::DbResult;
use kuro_db::handle::Db;
use kuro_domain::kind::EntryKind;
use rusqlite::Connection;

use crate::updates::stats::{AnimeStats, Counts, Failed, MangaStats, Overview, StatsState, Titles, TrackerStats};

/// The ids of the local source (`LocalSource.ID`).
const LOCAL_SOURCE_ID: i64 = 0;
/// `SManga.COMPLETED`.
const COMPLETED_STATUS: i64 = 2;

/// What the page shows and which sections failed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Loaded {
    pub state: StatsState,
    pub failed: Failed,
}

/// One kind's table and column names.
struct Names {
    entries: &'static str,
    items: &'static str,
    item_fk: &'static str,
    done: &'static str,
}

fn names(kind: EntryKind) -> Names {
    match kind {
        EntryKind::Manga => Names { entries: "mangas", items: "chapters", item_fk: "manga_id", done: "read" },
        EntryKind::Anime => Names { entries: "animes", items: "episodes", item_fk: "anime_id", done: "seen" },
    }
}

fn count(c: &Connection, sql: &str) -> rusqlite::Result<i64> {
    c.query_row(sql, [], |r| r.get(0))
}

fn overview(c: &Connection, kind: EntryKind) -> rusqlite::Result<Overview> {
    let n = names(kind);
    let library_count = count(c, &format!("SELECT COUNT(*) FROM {} WHERE favorite = 1", n.entries))?;
    let completed_count = count(
        c,
        &format!(
            "SELECT COUNT(*) FROM {e} e WHERE e.favorite = 1 AND e.status = {COMPLETED_STATUS} \
             AND NOT EXISTS (SELECT 1 FROM {i} i WHERE i.{fk} = e._id AND i.{d} = 0)",
            e = n.entries,
            i = n.items,
            fk = n.item_fk,
            d = n.done
        ),
    )?;
    // manga: the read time of all history (EC-42-S-01); anime: the length of a seen episode, else the position
    // reached, over library entries only (EC-42-S-03)
    let total_duration_ms = match kind {
        EntryKind::Manga => count(c, "SELECT COALESCE(SUM(time_read), 0) FROM history")?,
        EntryKind::Anime => count(
            c,
            "SELECT COALESCE(SUM(CASE WHEN i.seen = 1 THEN i.total_seconds ELSE i.last_second_seen END), 0) \
             FROM episodes i JOIN animes e ON e._id = i.anime_id WHERE e.favorite = 1",
        )?,
    };
    Ok(Overview { library_count, completed_count, total_duration_ms })
}

fn titles(c: &Connection, kind: EntryKind) -> rusqlite::Result<Titles> {
    let n = names(kind);
    let global_update_count = count(c, &format!("SELECT COUNT(*) FROM {} WHERE favorite = 1 AND update_strategy = 0", n.entries))?;
    let started_count = count(
        c,
        &format!(
            "SELECT COUNT(*) FROM {e} e WHERE e.favorite = 1 AND EXISTS (SELECT 1 FROM {i} i WHERE i.{fk} = e._id AND i.{d} = 1)",
            e = n.entries,
            i = n.items,
            fk = n.item_fk,
            d = n.done
        ),
    )?;
    let local_count = count(c, &format!("SELECT COUNT(*) FROM {} WHERE favorite = 1 AND source = {LOCAL_SOURCE_ID}", n.entries))?;
    Ok(Titles { global_update_count, started_count, local_count })
}

fn counts(c: &Connection, kind: EntryKind, downloaded: i64) -> rusqlite::Result<Counts> {
    let n = names(kind);
    let (total, read) = c.query_row(
        &format!(
            "SELECT COUNT(*), COALESCE(SUM(i.{d}), 0) FROM {i} i JOIN {e} e ON e._id = i.{fk} WHERE e.favorite = 1",
            e = n.entries,
            i = n.items,
            fk = n.item_fk,
            d = n.done
        ),
        [],
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
    )?;
    Ok(Counts { total, read, downloaded })
}

/// The value of a finished section, or its default with the flag set when the query failed.
fn section<T: Default>(result: DbResult<T>, failed: &mut bool) -> T {
    result
        .map_err(|e| {
            tracing::warn!("a statistics section failed: {e}");
            *failed = true;
        })
        .unwrap_or_default()
}

/// Aggregates one kind. `downloaded` is the number of downloaded units (the caller owns the download cache).
pub fn load(db: &Arc<Db>, kind: EntryKind, downloaded: i64) -> Loaded {
    let mut failed = Failed::default();
    let overview = section(db.read(|c| Ok(overview(c, kind)?)), &mut failed.overview);
    let titles = section(db.read(|c| Ok(titles(c, kind)?)), &mut failed.titles);
    let counts = section(db.read(|c| Ok(counts(c, kind, downloaded)?)), &mut failed.counts);
    failed.trackers = true;
    let trackers = TrackerStats::default();
    let state = match kind {
        EntryKind::Manga => StatsState::Manga(MangaStats { overview, titles, chapters: counts, trackers }),
        EntryKind::Anime => StatsState::Anime(AnimeStats { overview, titles, episodes: counts, trackers }),
    };
    Loaded { state, failed }
}

#[cfg(test)]
mod tests {
    use kuro_data::chapter::{Chapter, ChapterRepo};
    use kuro_data::manga::{Manga, MangaRepo};

    use super::*;

    fn manga_db() -> (tempfile::TempDir, Arc<Db>) {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("manga.db");
        let writer = kuro_db::schema_manga::open(&path).expect("open manga db");
        (dir, Arc::new(Db::new(&path, writer).expect("manga db handle")))
    }

    fn manga(db: &Arc<Db>, title: &str, favorite: bool, status: i64, source: i64) -> i64 {
        MangaRepo::new(db.clone())
            .insert(&Manga { favorite, status, source, url: title.into(), title: title.into(), ..Manga::create() })
            .expect("insert manga")
    }

    fn chapters(db: &Arc<Db>, manga_id: i64, read: &[bool]) {
        let rows: Vec<Chapter> = read
            .iter()
            .enumerate()
            .map(|(i, r)| Chapter { manga_id, url: format!("{manga_id}-{i}"), name: format!("c{i}"), read: *r, ..Chapter::create() })
            .collect();
        ChapterRepo::new(db.clone()).add_all(&rows).expect("add chapters");
    }

    #[test]
    fn stats_manga_counts_library_completed_started_local_and_chapters() {
        let (_dir, db) = manga_db();
        let done = manga(&db, "Done", true, COMPLETED_STATUS, 7);
        chapters(&db, done, &[true, true]);
        let ongoing = manga(&db, "Ongoing", true, 1, 7);
        chapters(&db, ongoing, &[true, false]);
        let local = manga(&db, "Local", true, 1, LOCAL_SOURCE_ID);
        chapters(&db, local, &[false]);
        let left = manga(&db, "Left", false, COMPLETED_STATUS, 7);
        chapters(&db, left, &[true]);

        let loaded = load(&db, EntryKind::Manga, 4);
        let StatsState::Manga(stats) = loaded.state else { panic!("manga state") };
        assert_eq!(stats.overview.library_count, 3);
        assert_eq!(stats.overview.completed_count, 1, "status 2 and no unread chapter");
        assert_eq!(stats.titles.started_count, 2);
        assert_eq!(stats.titles.local_count, 1);
        assert_eq!(stats.titles.global_update_count, 3);
        assert_eq!(stats.chapters, Counts { total: 5, read: 3, downloaded: 4 }, "entries that left the library do not count");
        assert!(!loaded.failed.overview && !loaded.failed.titles && !loaded.failed.counts);
        assert!(loaded.failed.trackers, "no tracker service is reachable: the section shows N/A");
    }

    #[test]
    fn stats_manga_read_duration_sums_all_history() {
        let (_dir, db) = manga_db();
        let m = manga(&db, "M", true, 1, 7);
        chapters(&db, m, &[true]);
        db.write(|tx| {
            tx.execute("INSERT INTO history(chapter_id, last_read, time_read) SELECT _id, 5, 90000 FROM chapters", [])?;
            Ok(())
        })
        .expect("history");
        let StatsState::Manga(stats) = load(&db, EntryKind::Manga, 0).state else { panic!("manga state") };
        assert_eq!(stats.overview.total_duration_ms, 90_000);
    }

    #[test]
    fn stats_an_empty_library_gives_zeros_not_a_failure() {
        let (_dir, db) = manga_db();
        let loaded = load(&db, EntryKind::Manga, 0);
        assert_eq!(loaded.state, StatsState::Manga(MangaStats::default()));
        assert!(!loaded.failed.overview);
    }

    #[test]
    fn stats_a_failing_section_is_marked_and_the_others_survive() {
        let (_dir, db) = manga_db();
        manga(&db, "M", true, 1, 7);
        db.write(|tx| {
            tx.execute("DROP TABLE history", [])?;
            Ok(())
        })
        .expect("drop history");
        let loaded = load(&db, EntryKind::Manga, 0);
        assert!(loaded.failed.overview, "the duration query fails without the history table");
        assert!(!loaded.failed.titles && !loaded.failed.counts);
        let StatsState::Manga(stats) = loaded.state else { panic!("manga state") };
        assert_eq!(stats.titles.global_update_count, 1);
    }

    #[test]
    fn stats_anime_watch_time_uses_the_length_when_seen_else_the_position() {
        use kuro_data::anime::{Anime, AnimeRepo};
        use kuro_data::episode::{Episode, EpisodeRepo};
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("anime.db");
        let writer = kuro_db::schema_anime::open(&path).expect("open anime db");
        let db = Arc::new(Db::new(&path, writer).expect("anime db handle"));
        let anime = AnimeRepo::new(db.clone())
            .insert(&Anime { favorite: true, source: 7, url: "a".into(), title: "A".into(), ..Anime::create() })
            .expect("insert anime");
        let episode = |name: &str, seen: bool, position: i64, length: i64| Episode {
            anime_id: anime,
            url: name.into(),
            name: name.into(),
            seen,
            last_second_seen: position,
            total_seconds: length,
            ..Episode::create()
        };
        EpisodeRepo::new(db.clone())
            .add_all(&[episode("seen", true, 10, 1_000), episode("started", false, 250, 2_000), episode("fresh", false, 0, 500)])
            .expect("add episodes");
        let StatsState::Anime(stats) = load(&db, EntryKind::Anime, 0).state else { panic!("anime state") };
        assert_eq!(stats.overview.total_duration_ms, 1_250);
        assert_eq!(stats.episodes, Counts { total: 3, read: 1, downloaded: 0 });
        assert_eq!(stats.titles.started_count, 1);
    }
}
