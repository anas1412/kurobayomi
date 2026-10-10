//! The statistics aggregation of the Statistics page (T-42a-040, T-42a-041, T-42a-044): one aggregate statement per kind
//! for the library sections, so a failing section shows "N/A" while the others keep their values (EC-42-S-06). The
//! pure shapes and the card layout are in [`stats`]; this file only reads the database.
//!
//! "In global update" follows `getGlobalUpdateItemCount` of the Aniyomi stats model: the include and exclude categories
//! and the three entry restriction tokens of [`GlobalUpdateRules`] decide which library entries count. Like the source it
//! does not read the update strategy or the release-period token (EC-42-S-07). Trackers are `Failed` while no tracker service is
//! reachable from the screens (a missing service is a section error, not a panic).

use std::collections::BTreeSet;
use std::sync::Arc;

use kuro_db::error::DbResult;
use kuro_db::handle::Db;
use kuro_domain::kind::EntryKind;
use rusqlite::Connection;

use crate::updates::stats::{self, AnimeStats, Counts, Failed, MangaStats, Overview, StatsState, Titles, TrackerStats};

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
    /// The library view and its done-count column, which carry the category of each row and the totals after the
    /// excluded scanlators (the numbers the library update itself reads).
    view: &'static str,
    view_done: &'static str,
}

fn names(kind: EntryKind) -> Names {
    match kind {
        EntryKind::Manga => Names {
            entries: "mangas",
            items: "chapters",
            item_fk: "manga_id",
            done: "read",
            view: "libraryView",
            view_done: "readCount",
        },
        EntryKind::Anime => Names {
            entries: "animes",
            items: "episodes",
            item_fk: "anime_id",
            done: "seen",
            view: "animelibView",
            view_done: "seenCount",
        },
    }
}

/// What the global update takes into account (T-42a-040): the include and exclude categories and the entry restriction
/// tokens of the library update preferences. The default has no rule, so every favourite counts. The release-period token
/// is not read, as in the source (EC-42-S-07).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GlobalUpdateRules {
    pub include: BTreeSet<i64>,
    pub exclude: BTreeSet<i64>,
    pub skip_completed: bool,
    pub skip_unread: bool,
    pub skip_not_started: bool,
}

impl GlobalUpdateRules {
    /// The rules of the stored preferences: category ids and restriction tokens as the library update reads them.
    pub fn from_prefs(include: &BTreeSet<String>, exclude: &BTreeSet<String>, restrictions: &BTreeSet<String>) -> Self {
        use kuro_prefs::library::{ENTRY_HAS_UNVIEWED, ENTRY_NON_COMPLETED, ENTRY_NON_VIEWED};
        let ids = |set: &BTreeSet<String>| set.iter().filter_map(|id| id.parse().ok()).collect();
        Self {
            include: ids(include),
            exclude: ids(exclude),
            skip_completed: restrictions.contains(ENTRY_NON_COMPLETED),
            skip_unread: restrictions.contains(ENTRY_HAS_UNVIEWED),
            skip_not_started: restrictions.contains(ENTRY_NON_VIEWED),
        }
    }

    /// The rules of one kind from the app's preferences.
    pub fn from_library_prefs(prefs: &kuro_prefs::library::LibraryPrefs, kind: EntryKind) -> Self {
        let (include, exclude) = match kind {
            EntryKind::Manga => (prefs.manga_update_categories.get(), prefs.manga_update_categories_exclude.get()),
            EntryKind::Anime => (prefs.anime_update_categories.get(), prefs.anime_update_categories_exclude.get()),
        };
        Self::from_prefs(&include, &exclude, &prefs.auto_update_item_restrictions.get())
    }

    /// `category IN (...)` over the ids, or `empty` when the set is empty. The ids are numbers, so they are inlined.
    fn category_test(set: &BTreeSet<i64>, empty: &str) -> String {
        if set.is_empty() {
            return empty.to_owned();
        }
        let list: Vec<String> = set.iter().map(i64::to_string).collect();
        format!("v.category IN ({})", list.join(", "))
    }
}

/// Everything the three library sections read, from one statement.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Aggregate {
    library_count: i64,
    completed_count: i64,
    started_count: i64,
    local_count: i64,
    global_update_count: i64,
    total: i64,
    done: i64,
    /// Anime only: the length of the seen episodes plus the position of the others. 0 for manga.
    watched_ms: i64,
}

/// The single aggregate statement of one kind (rust design 4.4): the favourites with their item counts, joined to the
/// global-update verdict of each entry, as `getGlobalUpdateItemCount` has it: in an included category (or any when none
/// is included), in no excluded category, and not skipped by a restriction token. An entry in no category counts as
/// category 0, as the library view says.
fn aggregate_sql(kind: EntryKind, rules: &GlobalUpdateRules) -> String {
    let n = names(kind);
    let include = GlobalUpdateRules::category_test(&rules.include, "1");
    let exclude = GlobalUpdateRules::category_test(&rules.exclude, "0");
    let watched = match kind {
        EntryKind::Manga => "0".to_owned(),
        EntryKind::Anime => "COALESCE(SUM(CASE WHEN i.seen = 1 THEN i.total_seconds ELSE i.last_second_seen END), 0)".to_owned(),
    };
    format!(
        "WITH g AS (\
            SELECT v._id AS id, \
                   CASE WHEN MAX({include}) = 1 AND MAX({exclude}) = 0 \
                         AND (?1 = 0 OR v.status != {COMPLETED_STATUS}) \
                         AND (?2 = 0 OR v.totalCount - v.{vd} = 0) \
                         AND (?3 = 0 OR NOT (v.totalCount > 0 AND v.{vd} = 0)) \
                   THEN 1 ELSE 0 END AS hit \
            FROM {view} v GROUP BY v._id), \
         lib AS (\
            SELECT e._id AS id, e.status AS status, e.source AS source, COUNT(i._id) AS total, \
                   COALESCE(SUM(i.{d} = 0), 0) AS unfinished, COALESCE(SUM(i.{d}), 0) AS finished, {watched} AS watched \
            FROM {e} e LEFT JOIN {i} i ON i.{fk} = e._id WHERE e.favorite = 1 GROUP BY e._id) \
         SELECT COUNT(*), \
                COALESCE(SUM(lib.status = {COMPLETED_STATUS} AND lib.unfinished = 0), 0), \
                COALESCE(SUM(lib.finished > 0), 0), \
                COALESCE(SUM(lib.source = {LOCAL_SOURCE_ID}), 0), \
                COALESCE(SUM(g.hit), 0), \
                COALESCE(SUM(lib.total), 0), \
                COALESCE(SUM(lib.finished), 0), \
                COALESCE(SUM(lib.watched), 0) \
         FROM lib LEFT JOIN g ON g.id = lib.id",
        view = n.view,
        vd = n.view_done,
        e = n.entries,
        i = n.items,
        fk = n.item_fk,
        d = n.done,
    )
}

fn aggregate(c: &Connection, kind: EntryKind, rules: &GlobalUpdateRules) -> rusqlite::Result<Aggregate> {
    c.query_row(
        &aggregate_sql(kind, rules),
        rusqlite::params![
            i64::from(rules.skip_completed),
            i64::from(rules.skip_unread),
            i64::from(rules.skip_not_started)
        ],
        |r| {
            Ok(Aggregate {
                library_count: r.get(0)?,
                completed_count: r.get(1)?,
                started_count: r.get(2)?,
                local_count: r.get(3)?,
                global_update_count: r.get(4)?,
                total: r.get(5)?,
                done: r.get(6)?,
                watched_ms: r.get(7)?,
            })
        },
    )
}

/// Manga: the read time of all history (EC-42-S-01). Its own statement, so a failing history table leaves the library
/// sections alone (EC-42-S-06). Anime takes its duration from the aggregate (EC-42-S-03).
fn manga_read_time(c: &Connection) -> rusqlite::Result<i64> {
    c.query_row("SELECT COALESCE(SUM(time_read), 0) FROM history", [], |r| r.get(0))
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

/// The tracker links of the library entries of one kind, one list per entry that has a logged-in link, each holding the
/// scores of those links (0 is unscored). A missing tracker table is a query error, which the caller marks as failed.
fn tracker_links(c: &Connection, kind: EntryKind, logged_in: &BTreeSet<i64>) -> rusqlite::Result<Vec<Vec<f64>>> {
    let (sync, fk, entries) = match kind {
        EntryKind::Manga => ("manga_sync", "manga_id", "mangas"),
        EntryKind::Anime => ("anime_sync", "anime_id", "animes"),
    };
    let mut stmt = c.prepare(&format!(
        "SELECT s.{fk}, s.sync_id, COALESCE(s.score, 0) FROM {sync} s JOIN {entries} e ON e._id = s.{fk} \
         WHERE e.favorite = 1 ORDER BY s.{fk}, s._id"
    ))?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, f64>(2)?)))?;
    let mut lists: Vec<Vec<f64>> = Vec::new();
    let mut current = None;
    for row in rows {
        let (entry, tracker, score) = row?;
        if !logged_in.contains(&tracker) {
            continue;
        }
        if current != Some(entry) {
            lists.push(Vec::new());
            current = Some(entry);
        }
        lists.last_mut().expect("a list was pushed for this entry").push(score);
    }
    Ok(lists)
}

/// Aggregates one kind. `downloaded` is the number of downloaded units (the caller owns the download cache), `logged_in`
/// the ids of the trackers the user is logged in to (the caller reads the secret store) and `rules` the library update
/// preferences the global-update count follows. One statement reads the library sections; if it fails all three show
/// N/A together.
pub fn load(db: &Arc<Db>, kind: EntryKind, downloaded: i64, logged_in: &BTreeSet<i64>, rules: &GlobalUpdateRules) -> Loaded {
    let mut failed = Failed::default();
    let totals = db.read(|c| Ok(aggregate(c, kind, rules)?));
    let library_failed = totals.is_err();
    let totals = section(totals, &mut failed.titles);
    failed.counts = library_failed;
    failed.overview = library_failed;
    let duration_ms = match kind {
        EntryKind::Manga => section(db.read(|c| Ok(manga_read_time(c)?)), &mut failed.overview),
        EntryKind::Anime => totals.watched_ms,
    };
    let overview = Overview { library_count: totals.library_count, completed_count: totals.completed_count, total_duration_ms: duration_ms };
    let titles = Titles {
        global_update_count: totals.global_update_count,
        started_count: totals.started_count,
        local_count: totals.local_count,
    };
    let counts = Counts { total: totals.total, read: totals.done, downloaded };
    let links = db.read(|c| Ok(tracker_links(c, kind, logged_in)?));
    let trackers = match links {
        Ok(links) => stats::tracker_stats(&links, logged_in.len() as i64),
        Err(e) => {
            tracing::warn!("the tracker statistics failed: {e}");
            failed.trackers = true;
            TrackerStats::default()
        }
    };
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

        let loaded = load(&db, EntryKind::Manga, 4, &BTreeSet::new(), &GlobalUpdateRules::default());
        let StatsState::Manga(stats) = loaded.state else { panic!("manga state") };
        assert_eq!(stats.overview.library_count, 3);
        assert_eq!(stats.overview.completed_count, 1, "status 2 and no unread chapter");
        assert_eq!(stats.titles.started_count, 2);
        assert_eq!(stats.titles.local_count, 1);
        assert_eq!(stats.titles.global_update_count, 3);
        assert_eq!(stats.chapters, Counts { total: 5, read: 3, downloaded: 4 }, "entries that left the library do not count");
        assert!(!loaded.failed.overview && !loaded.failed.titles && !loaded.failed.counts);
        assert!(!loaded.failed.trackers, "no logged-in tracker is an empty section, not a failure");
    }

    #[test]
    fn completed_empty_entry_counts() {
        // EC-42-S-04: a completed entry with no chapter has nothing unread, so it counts as completed
        let (_dir, db) = manga_db();
        manga(&db, "Empty and done", true, COMPLETED_STATUS, 7);
        let open = manga(&db, "Done but unread", true, COMPLETED_STATUS, 7);
        chapters(&db, open, &[true, false]);
        let StatsState::Manga(stats) = load(&db, EntryKind::Manga, 0, &BTreeSet::new(), &GlobalUpdateRules::default()).state else { panic!("manga state") };
        assert_eq!(stats.overview.completed_count, 1);
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
        let StatsState::Manga(stats) = load(&db, EntryKind::Manga, 0, &BTreeSet::new(), &GlobalUpdateRules::default()).state else { panic!("manga state") };
        assert_eq!(stats.overview.total_duration_ms, 90_000);
    }

    #[test]
    fn stats_an_empty_library_gives_zeros_not_a_failure() {
        let (_dir, db) = manga_db();
        let loaded = load(&db, EntryKind::Manga, 0, &BTreeSet::new(), &GlobalUpdateRules::default());
        assert_eq!(loaded.state, StatsState::Manga(MangaStats::default()));
        assert!(!loaded.failed.overview);
    }

    #[test]
    fn stats_a_failing_statement_marks_its_sections_and_spares_the_trackers() {
        let (_dir, db) = manga_db();
        manga(&db, "M", true, 1, 7);
        db.write(|tx| {
            tx.execute("DROP TABLE history", [])?;
            Ok(())
        })
        .expect("drop history");
        let loaded = load(&db, EntryKind::Manga, 0, &BTreeSet::new(), &GlobalUpdateRules::default());
        // the library view joins history, so the one aggregate statement fails and takes its three sections with it
        assert!(loaded.failed.overview && loaded.failed.titles && loaded.failed.counts, "the aggregate sections fail together");
        assert!(!loaded.failed.trackers, "the tracker links are their own query and keep their value");
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
        let StatsState::Anime(stats) = load(&db, EntryKind::Anime, 0, &BTreeSet::new(), &GlobalUpdateRules::default()).state else { panic!("anime state") };
        assert_eq!(stats.overview.total_duration_ms, 1_250);
        assert_eq!(stats.episodes, Counts { total: 3, read: 1, downloaded: 0 });
        assert_eq!(stats.titles.started_count, 1);
    }

    #[test]
    fn stats_tracker_mean_uses_logged_in_links_and_entry_means() {
        use kuro_data::track::{MangaTrack, MangaTrackRepo};
        let (_dir, db) = manga_db();
        let a = manga(&db, "A", true, 1, 7);
        let b = manga(&db, "B", true, 1, 7);
        let tracks = MangaTrackRepo::new(db.clone());
        let track = |manga_id: i64, tracker_id: i64, score: f64| MangaTrack { manga_id, score, ..MangaTrack::new(tracker_id) };
        tracks.insert(&track(a, 1, 8.0)).expect("track");
        tracks.insert(&track(a, 2, 6.0)).expect("track");
        tracks.insert(&track(a, 9, 10.0)).expect("track: tracker 9 is logged out");
        tracks.insert(&track(b, 1, 0.0)).expect("track");
        let logged: BTreeSet<i64> = [1, 2].into_iter().collect();
        let StatsState::Manga(stats) = load(&db, EntryKind::Manga, 0, &logged, &GlobalUpdateRules::default()).state else { panic!("manga state") };
        assert_eq!(stats.trackers.tracker_count, 2);
        assert_eq!(stats.trackers.tracked_count, 2, "both entries have a logged-in link");
        assert_eq!(stats.trackers.mean_score, Some(7.0), "A averages 8 and 6; B is unscored and does not count");
    }

    #[test]
    fn stats_no_logged_in_trackers_gives_no_mean() {
        let (_dir, db) = manga_db();
        manga(&db, "A", true, 1, 7);
        let loaded = load(&db, EntryKind::Manga, 0, &BTreeSet::new(), &GlobalUpdateRules::default());
        let StatsState::Manga(stats) = loaded.state else { panic!("manga state") };
        assert_eq!(stats.trackers.mean_score, None, "no logged-in tracker means N/A");
        assert!(!loaded.failed.trackers, "no logged-in tracker is not a failure");
    }

    /// One fixture entry: `items` are `(done, total_seconds, last_second)`, `categories` index into the two categories.
    struct Spec {
        favorite: bool,
        status: i64,
        source: i64,
        always: bool,
        next_update: i64,
        items: Vec<(bool, i64, i64)>,
        categories: Vec<usize>,
    }

    fn spec(status: i64, always: bool, items: &[bool], categories: &[usize]) -> Spec {
        Spec {
            favorite: true,
            status,
            source: 7,
            always,
            next_update: 0,
            items: items.iter().map(|done| (*done, 1_000, 250)).collect(),
            categories: categories.to_vec(),
        }
    }

    fn specs() -> Vec<Spec> {
        let mut local = spec(1, true, &[true], &[0]);
        local.source = LOCAL_SOURCE_ID;
        local.next_update = 9_000;
        let mut soon = spec(1, true, &[false, false], &[]);
        soon.next_update = 5_000;
        let mut gone = spec(1, true, &[true], &[0, 1]);
        gone.favorite = false;
        let mut due = spec(4, true, &[], &[]);
        due.next_update = 100;
        vec![
            spec(1, true, &[true, true, false], &[0]),
            spec(COMPLETED_STATUS, true, &[true, true], &[0, 1]),
            spec(1, false, &[false], &[1]),
            soon,
            due,
            local,
            gone,
            spec(COMPLETED_STATUS, true, &[true], &[1]),
        ]
    }

    /// The rule sets the parity test runs, each with a name for the failure message.
    fn rule_sets(a: i64, b: i64) -> Vec<(&'static str, GlobalUpdateRules)> {
        let set = |ids: &[i64]| ids.iter().copied().collect::<BTreeSet<i64>>();
        let base = GlobalUpdateRules::default();
        let with = |f: &dyn Fn(&mut GlobalUpdateRules)| {
            let mut rules = base.clone();
            f(&mut rules);
            rules
        };
        vec![
            ("no rule", base.clone()),
            ("include a", with(&|r| r.include = set(&[a]))),
            ("include the default category", with(&|r| r.include = set(&[0]))),
            ("include a and the default", with(&|r| r.include = set(&[a, 0]))),
            ("exclude b", with(&|r| r.exclude = set(&[b]))),
            ("include a, exclude b", with(&|r| {
                r.include = set(&[a]);
                r.exclude = set(&[b]);
            })),
            ("skip completed", with(&|r| r.skip_completed = true)),
            ("skip unread", with(&|r| r.skip_unread = true)),
            ("skip not started", with(&|r| r.skip_not_started = true)),
            ("every rule", with(&|r| {
                r.include = set(&[a, 0]);
                r.exclude = set(&[b]);
                r.skip_completed = true;
                r.skip_unread = true;
                r.skip_not_started = true;
            })),
        ]
    }

    /// What the per-entry reference computes: the page numbers, entry by entry.
    #[derive(Debug, Default, PartialEq, Eq)]
    struct Reference {
        library: i64,
        completed: i64,
        started: i64,
        local: i64,
        global: i64,
        total: i64,
        done: i64,
        watched: i64,
    }

    /// The N+1 reference: one query for the favourites, then three small queries per entry, with the global-update count
    /// taken from the formula of `getGlobalUpdateItemCount` over one `(entry, category)` row per link.
    fn reference(db: &Arc<Db>, kind: EntryKind, rules: &GlobalUpdateRules) -> Reference {
        let n = names(kind);
        let link = match kind {
            EntryKind::Manga => "mangas_categories",
            EntryKind::Anime => "animes_categories",
        };
        let ids: Vec<i64> = db
            .read(|c| {
                let mut stmt = c.prepare(&format!("SELECT _id FROM {} WHERE favorite = 1 ORDER BY _id", n.entries))?;
                Ok(stmt.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?)
            })
            .expect("favourites");
        let mut out = Reference::default();
        // (entry id, category, status, total, done), one per category link, 0 for none
        let mut rows: Vec<(i64, i64, i64, i64, i64)> = Vec::new();
        for id in ids {
            let (status, source): (i64, i64) = db
                .read(|c| {
                    Ok(c.query_row(&format!("SELECT status, source FROM {} WHERE _id = ?1", n.entries), [id], |r| Ok((r.get(0)?, r.get(1)?)))?)
                })
                .expect("entry");
            let items: Vec<(bool, i64, i64)> = db
                .read(|c| {
                    let sql = match kind {
                        EntryKind::Manga => format!("SELECT {d} = 1, 0, 0 FROM {i} WHERE {fk} = ?1", d = n.done, i = n.items, fk = n.item_fk),
                        EntryKind::Anime => format!(
                            "SELECT seen = 1, total_seconds, last_second_seen FROM {i} WHERE {fk} = ?1",
                            i = n.items,
                            fk = n.item_fk
                        ),
                    };
                    let mut stmt = c.prepare(&sql)?;
                    Ok(stmt.query_map([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<Result<_, _>>()?)
                })
                .expect("items");
            let mut categories: Vec<i64> = db
                .read(|c| {
                    let fk = if kind == EntryKind::Manga { "manga_id" } else { "anime_id" };
                    let mut stmt = c.prepare(&format!("SELECT category_id FROM {link} WHERE {fk} = ?1 ORDER BY category_id"))?;
                    Ok(stmt.query_map([id], |r| r.get(0))?.collect::<Result<_, _>>()?)
                })
                .expect("links");
            if categories.is_empty() {
                categories.push(0);
            }
            let done = items.iter().filter(|i| i.0).count() as i64;
            out.library += 1;
            out.completed += i64::from(status == COMPLETED_STATUS && done == items.len() as i64);
            out.started += i64::from(done > 0);
            out.local += i64::from(source == LOCAL_SOURCE_ID);
            out.total += items.len() as i64;
            out.done += done;
            out.watched += items.iter().map(|(seen, length, position)| if *seen { *length } else { *position }).sum::<i64>();
            rows.extend(categories.into_iter().map(|category| (id, category, status, items.len() as i64, done)));
        }
        let included: Vec<_> = rows.iter().filter(|r| rules.include.is_empty() || rules.include.contains(&r.1)).collect();
        let excluded: BTreeSet<i64> = if rules.exclude.is_empty() {
            BTreeSet::new()
        } else {
            rows.iter().filter(|r| rules.exclude.contains(&r.1)).map(|r| r.0).collect()
        };
        let mut seen = BTreeSet::new();
        out.global = included
            .into_iter()
            .filter(|r| !excluded.contains(&r.0))
            .filter(|r| seen.insert(r.0))
            .filter(|&&(_, _, status, total, done)| {
                !((rules.skip_completed && status == COMPLETED_STATUS)
                    || (rules.skip_unread && total - done != 0)
                    || (rules.skip_not_started && total > 0 && done == 0))
            })
            .count() as i64;
        out
    }

    fn page_numbers(loaded: &Loaded) -> Reference {
        let (overview, titles, counts, watched) = match &loaded.state {
            StatsState::Manga(s) => (s.overview, s.titles, s.chapters, 0),
            StatsState::Anime(s) => (s.overview, s.titles, s.episodes, s.overview.total_duration_ms),
            StatsState::Loading => panic!("a loaded state"),
        };
        Reference {
            library: overview.library_count,
            completed: overview.completed_count,
            started: titles.started_count,
            local: titles.local_count,
            global: titles.global_update_count,
            total: counts.total,
            done: counts.read,
            watched,
        }
    }

    #[test]
    fn stats_aggregate_matches_the_per_entry_reference_for_manga() {
        use kuro_data::category::create_manga_category;
        use kuro_db::codec::UpdateStrategy;
        let (_dir, db) = manga_db();
        let (a, b) = (create_manga_category(&db, "a", 0).expect("category a"), create_manga_category(&db, "b", 0).expect("category b"));
        let (repo, chapter_repo) = (MangaRepo::new(db.clone()), ChapterRepo::new(db.clone()));
        for (n, s) in specs().into_iter().enumerate() {
            let id = repo
                .insert(&Manga {
                    favorite: s.favorite,
                    status: s.status,
                    source: s.source,
                    next_update: s.next_update,
                    update_strategy: if s.always { UpdateStrategy::AlwaysUpdate } else { UpdateStrategy::OnlyFetchOnce },
                    url: format!("m{n}"),
                    title: format!("m{n}"),
                    ..Manga::create()
                })
                .expect("insert manga");
            let rows: Vec<Chapter> = s
                .items
                .iter()
                .enumerate()
                .map(|(i, (done, _, _))| Chapter { manga_id: id, url: format!("{id}-{i}"), name: format!("c{i}"), read: *done, ..Chapter::create() })
                .collect();
            chapter_repo.add_all(&rows).expect("chapters");
            repo.set_categories(id, &s.categories.iter().map(|c| [a, b][*c]).collect::<Vec<_>>()).expect("links");
        }
        for (name, rules) in rule_sets(a, b) {
            let loaded = load(&db, EntryKind::Manga, 0, &BTreeSet::new(), &rules);
            assert!(!loaded.failed.overview && !loaded.failed.titles && !loaded.failed.counts, "{name}: a section failed");
            assert_eq!(page_numbers(&loaded), reference(&db, EntryKind::Manga, &rules), "{name}");
        }
    }

    #[test]
    fn stats_aggregate_matches_the_per_entry_reference_for_anime() {
        use kuro_data::anime::{Anime, AnimeRepo};
        use kuro_data::category::create_anime_category;
        use kuro_data::episode::{Episode, EpisodeRepo};
        use kuro_db::codec::UpdateStrategy;
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("anime.db");
        let writer = kuro_db::schema_anime::open(&path).expect("open anime db");
        let db = Arc::new(Db::new(&path, writer).expect("anime db handle"));
        let (a, b) = (create_anime_category(&db, "a", 0).expect("category a"), create_anime_category(&db, "b", 0).expect("category b"));
        let (repo, episodes) = (AnimeRepo::new(db.clone()), EpisodeRepo::new(db.clone()));
        for (n, s) in specs().into_iter().enumerate() {
            let id = repo
                .insert(&Anime {
                    favorite: s.favorite,
                    status: s.status,
                    source: s.source,
                    next_update: s.next_update,
                    update_strategy: if s.always { UpdateStrategy::AlwaysUpdate } else { UpdateStrategy::OnlyFetchOnce },
                    url: format!("a{n}"),
                    title: format!("a{n}"),
                    ..Anime::create()
                })
                .expect("insert anime");
            let rows: Vec<Episode> = s
                .items
                .iter()
                .enumerate()
                .map(|(i, (done, length, position))| Episode {
                    anime_id: id,
                    url: format!("{id}-{i}"),
                    name: format!("e{i}"),
                    seen: *done,
                    total_seconds: *length,
                    last_second_seen: *position,
                    ..Episode::create()
                })
                .collect();
            episodes.add_all(&rows).expect("episodes");
            repo.set_categories(id, &s.categories.iter().map(|c| [a, b][*c]).collect::<Vec<_>>()).expect("links");
        }
        for (name, rules) in rule_sets(a, b) {
            let loaded = load(&db, EntryKind::Anime, 0, &BTreeSet::new(), &rules);
            assert!(!loaded.failed.overview && !loaded.failed.titles && !loaded.failed.counts, "{name}: a section failed");
            assert_eq!(page_numbers(&loaded), reference(&db, EntryKind::Anime, &rules), "{name}");
        }
    }

    #[test]
    fn global_update_include_exclude_and_restrictions() {
        use kuro_data::category::create_manga_category;
        let (_dir, db) = manga_db();
        let included = create_manga_category(&db, "in", 0).expect("category");
        let excluded = create_manga_category(&db, "out", 0).expect("category");
        let repo = MangaRepo::new(db.clone());
        let keep = manga(&db, "keep", true, 1, 7);
        let both = manga(&db, "both", true, 1, 7);
        let finished = manga(&db, "finished", true, COMPLETED_STATUS, 7);
        chapters(&db, finished, &[true]);
        repo.set_categories(keep, &[included]).expect("links");
        repo.set_categories(both, &[included, excluded]).expect("links");
        repo.set_categories(finished, &[included]).expect("links");
        let count = |rules: GlobalUpdateRules| {
            let StatsState::Manga(s) = load(&db, EntryKind::Manga, 0, &BTreeSet::new(), &rules).state else { panic!("manga state") };
            s.titles.global_update_count
        };
        let tokens: BTreeSet<String> = ["manga_ongoing".to_owned()].into();
        let ids = |set: &[i64]| set.iter().map(i64::to_string).collect::<BTreeSet<String>>();
        assert_eq!(count(GlobalUpdateRules::default()), 3, "no rule: every favourite");
        assert_eq!(count(GlobalUpdateRules::from_prefs(&ids(&[included]), &ids(&[excluded]), &BTreeSet::new())), 2, "an excluded category removes the entry even when it is also included");
        assert_eq!(count(GlobalUpdateRules::from_prefs(&ids(&[]), &ids(&[]), &tokens)), 2, "the ongoing-only token skips the completed entry");
        assert_eq!(count(GlobalUpdateRules::from_prefs(&ids(&[excluded]), &ids(&[]), &BTreeSet::new())), 1, "only the one entry in the included category");
    }

    /// The 2,000-entry budget of T-42a-044. Ignored on CI: run with `cargo test -- --ignored stats_two_thousand`.
    #[test]
    #[ignore = "performance check, run locally"]
    fn stats_two_thousand_entries_under_a_second() {
        let (_dir, db) = manga_db();
        for i in 0..2_000 {
            manga(&db, &format!("entry {i}"), true, 1, 7);
        }
        let started = std::time::Instant::now();
        let loaded = load(&db, EntryKind::Manga, 0, &BTreeSet::new(), &GlobalUpdateRules::default());
        assert!(started.elapsed() < std::time::Duration::from_secs(1), "took {:?}", started.elapsed());
        let StatsState::Manga(stats) = loaded.state else { panic!("manga state") };
        assert_eq!(stats.overview.library_count, 2_000);
    }
}
