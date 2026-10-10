//! One scan of a chosen root written to the anime database (wire-local-anime). Anime and episode rows go through the
//! kuro-data repositories under the local source id. Which episode rows change is `plan::plan_episodes`, so a rescan
//! keeps the seen state and the saved position of every file that is still there.

use std::path::Path;
use std::sync::Arc;

use kuro_data::anime::{Anime, AnimeRepo, AnimeUpdate};
use kuro_data::episode::EpisodeRepo;
use kuro_data::{DataError, LookupOrInsert};
use kuro_db::handle::Db;
use kuro_local::LOCAL_SOURCE_ID;

use super::plan::plan_episodes;
use super::scan::{self, Series};

/// What one scan of a root changed.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ScanReport {
    /// Series folders with episodes after the scan.
    pub series: usize,
    pub episodes_added: usize,
    pub episodes_removed: usize,
}

/// Scans every series folder under `root` and writes its rows. A series that fails is logged and left out of the report.
pub fn scan_root(db: &Arc<Db>, root: &Path) -> Result<ScanReport, String> {
    let now = chrono::Utc::now().timestamp_millis();
    let mut report = ScanReport::default();
    for series in scan::series_in(root).map_err(|e| e.to_string())? {
        match sync_series(db, root, &series, now) {
            Ok(Some(done)) => {
                report.series += 1;
                report.episodes_added += done.added;
                report.episodes_removed += done.removed;
            }
            Ok(None) => {}
            Err(e) => tracing::error!(error = %e, series = %series.name, "a local anime folder could not be synced"),
        }
    }
    Ok(report)
}

/// Rescans the series already in the library, each under the folder it came from (the parent of its series folder).
/// A series the user took out of the library is not added back, unlike a folder chosen again with `scan_root`.
pub fn rescan_library(db: &Arc<Db>, now: i64) -> Result<ScanReport, String> {
    let mut report = ScanReport::default();
    for row in AnimeRepo::new(Arc::clone(db)).favorites_by_source(LOCAL_SOURCE_ID).map_err(text)? {
        let dir = std::path::PathBuf::from(&row.url);
        let (Some(root), Some(name)) = (dir.parent(), dir.file_name()) else { continue };
        let series = Series { name: name.to_string_lossy().into_owned(), dir: dir.clone() };
        match sync_series(db, root, &series, now) {
            Ok(Some(done)) => {
                report.series += 1;
                report.episodes_added += done.added;
                report.episodes_removed += done.removed;
            }
            Ok(None) => {}
            Err(e) => tracing::error!(error = %e, series = %series.name, "a local anime folder could not be rescanned"),
        }
    }
    Ok(report)
}

/// The episode changes of one series.
#[derive(Debug, PartialEq, Eq)]
struct Synced {
    added: usize,
    removed: usize,
}

/// Makes the rows of one series match its folder. A series with no episode files gets no row, and an existing row keeps
/// its episodes removed when its files are gone. Returns `None` when no row was made.
fn sync_series(db: &Arc<Db>, root: &Path, series: &Series, now: i64) -> Result<Option<Synced>, String> {
    let anime = AnimeRepo::new(Arc::clone(db));
    let episodes = EpisodeRepo::new(Arc::clone(db));
    let url = series.dir.to_string_lossy().into_owned();
    let existing = anime.by_url_and_source(&url, LOCAL_SOURCE_ID).map_err(text)?;
    let mut wanted = scan::episodes_of(root, series, now)?;
    if existing.is_none() && wanted.is_empty() {
        return Ok(None);
    }
    let anime_id = match existing {
        Some(found) => {
            // the user chose this folder again, so the series belongs in the library
            if !found.favorite {
                anime.update(&AnimeUpdate { favorite: Some(true), ..AnimeUpdate::new(found.id) }).map_err(text)?;
            }
            found.id
        }
        None => match anime.lookup_or_insert(&series_row(series, url, now)).map_err(text)? {
            LookupOrInsert::Found(found) => found.id,
            LookupOrInsert::Inserted(id) => id,
        },
    };
    for row in &mut wanted {
        row.anime_id = anime_id;
    }
    let plan = plan_episodes(&episodes.by_anime_id(anime_id).map_err(text)?, wanted);
    episodes.add_all(&plan.add).map_err(text)?;
    episodes.update_all(&plan.update).map_err(text)?;
    episodes.remove_with_ids(&plan.remove).map_err(text)?;
    Ok(Some(Synced { added: plan.add.len(), removed: plan.remove.len() }))
}

/// The anime row of a new series: a library entry of the local source, titled by its folder.
fn series_row(series: &Series, url: String, now: i64) -> Anime {
    Anime { source: LOCAL_SOURCE_ID, url, title: series.name.clone(), favorite: true, initialized: true, date_added: now, ..Anime::create() }
}

fn text(e: DataError) -> String {
    e.to_string()
}

#[cfg(test)]
pub(crate) mod tests {
    use std::fs;

    use kuro_data::episode::{EpisodeRepo, EpisodeUpdate};

    use super::*;

    /// A database in a temporary folder, opened the way the app opens its anime database.
    pub(crate) fn anime_db() -> (tempfile::TempDir, Arc<Db>) {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("anime.db");
        let writer = kuro_db::schema_anime::open(&path).expect("open anime db");
        (dir, Arc::new(Db::new(&path, writer).expect("anime db handle")))
    }

    /// The rows the anime Library screen lists, read the way the screen reads them.
    fn library_rows(db: &Arc<Db>) -> Vec<kuro_domain::library::model::LibraryRow> {
        kuro_domain::library::repo::LibraryRepo::new(Arc::clone(db)).all(kuro_domain::kind::EntryKind::Anime).expect("library rows")
    }

    fn video(root: &Path, series: &str, file: &str) {
        let dir = root.join(series);
        fs::create_dir_all(&dir).expect("series folder");
        fs::write(dir.join(file), b"video").expect("video file");
    }

    #[test]
    fn a_scan_makes_library_entries_with_their_episodes() {
        let (_db_dir, db) = anime_db();
        let root = tempfile::tempdir().expect("root");
        video(root.path(), "Show", "Show - 1.mkv");
        video(root.path(), "Show", "Show - 2.mkv");
        video(root.path(), "Other", "Other 01.mp4");
        fs::create_dir(root.path().join("Empty")).expect("empty series");

        let report = scan_root(&db, root.path()).expect("scan");
        assert_eq!(report, ScanReport { series: 2, episodes_added: 3, episodes_removed: 0 });

        let shown = library_rows(&db);
        let show = shown.iter().find(|a| a.title == "Show").expect("Show in the library");
        let row = AnimeRepo::new(Arc::clone(&db)).by_id(show.id).expect("row");
        assert_eq!(row.source, LOCAL_SOURCE_ID);
        assert!(row.favorite);
        assert_eq!(row.url, root.path().join("Show").to_string_lossy());
        let rows = EpisodeRepo::new(db).by_anime_id(row.id).expect("episodes");
        let mut urls: Vec<String> = rows.iter().map(|e| e.url.clone()).collect();
        urls.sort();
        assert_eq!(urls.len(), 2);
        assert!(urls[0].ends_with("Show - 1.mkv") && urls[1].ends_with("Show - 2.mkv"));
    }

    #[test]
    fn a_rescan_keeps_seen_state_and_position_and_follows_the_files() {
        let (_db_dir, db) = anime_db();
        let root = tempfile::tempdir().expect("root");
        video(root.path(), "Show", "Show - 1.mkv");
        video(root.path(), "Show", "Show - 2.mkv");
        scan_root(&db, root.path()).expect("first scan");
        let episodes = EpisodeRepo::new(Arc::clone(&db));
        let anime_id = AnimeRepo::new(Arc::clone(&db)).by_url_and_source(&root.path().join("Show").to_string_lossy(), LOCAL_SOURCE_ID).expect("query").expect("row").id;
        let first = episodes.by_anime_id(anime_id).expect("rows").into_iter().find(|e| e.name == "Show - 1").expect("episode 1");
        episodes.update(&EpisodeUpdate { seen: Some(true), last_second_seen: Some(612), ..EpisodeUpdate::new(first.id) }).expect("mark seen");

        fs::remove_file(root.path().join("Show").join("Show - 2.mkv")).expect("remove episode 2");
        video(root.path(), "Show", "Show - 3.mkv");
        let report = scan_root(&db, root.path()).expect("rescan");
        assert_eq!(report, ScanReport { series: 1, episodes_added: 1, episodes_removed: 1 });

        let rows = episodes.by_anime_id(anime_id).expect("rows");
        let kept = rows.iter().find(|e| e.name == "Show - 1").expect("episode 1 still there");
        assert_eq!(kept.id, first.id, "the row is updated, not recreated");
        assert!(kept.seen);
        assert_eq!(kept.last_second_seen, 612);
        assert!(rows.iter().any(|e| e.name == "Show - 3"));
        assert!(!rows.iter().any(|e| e.name == "Show - 2"));
    }

    #[test]
    fn a_folder_without_episodes_is_not_added_and_an_existing_row_loses_its_episodes() {
        let (_db_dir, db) = anime_db();
        let root = tempfile::tempdir().expect("root");
        fs::create_dir(root.path().join("Bare")).expect("folder without episodes");
        assert_eq!(scan_root(&db, root.path()).expect("scan"), ScanReport::default());
        assert!(library_rows(&db).is_empty());

        video(root.path(), "Show", "Show - 1.mkv");
        scan_root(&db, root.path()).expect("first scan");
        fs::remove_file(root.path().join("Show").join("Show - 1.mkv")).expect("remove");
        let report = scan_root(&db, root.path()).expect("rescan");
        assert_eq!(report.episodes_removed, 1);
        let anime_id = AnimeRepo::new(Arc::clone(&db)).by_url_and_source(&root.path().join("Show").to_string_lossy(), LOCAL_SOURCE_ID).expect("query").expect("row").id;
        assert!(EpisodeRepo::new(db).by_anime_id(anime_id).expect("rows").is_empty());
    }

    #[test]
    fn a_library_rescan_follows_its_series_and_leaves_removed_ones_out() {
        let (_db_dir, db) = anime_db();
        let root = tempfile::tempdir().expect("root");
        video(root.path(), "Show", "Show - 1.mkv");
        video(root.path(), "Gone", "Gone - 1.mkv");
        scan_root(&db, root.path()).expect("first scan");
        let repo = AnimeRepo::new(Arc::clone(&db));
        let gone = repo.by_url_and_source(&root.path().join("Gone").to_string_lossy(), LOCAL_SOURCE_ID).expect("query").expect("row").id;
        repo.update(&AnimeUpdate { favorite: Some(false), ..AnimeUpdate::new(gone) }).expect("unfavourite");
        video(root.path(), "Show", "Show - 2.mkv");
        video(root.path(), "New", "New - 1.mkv");

        let report = rescan_library(&db, 5).expect("rescan");
        assert_eq!(report, ScanReport { series: 1, episodes_added: 1, episodes_removed: 0 });
        assert!(!repo.by_id(gone).expect("row").favorite, "a removed series stays out of the library");
        assert!(library_rows(&db).iter().all(|a| a.title != "New"), "a folder that was never added is not picked up");
        let show = repo.by_url_and_source(&root.path().join("Show").to_string_lossy(), LOCAL_SOURCE_ID).expect("query").expect("row").id;
        assert_eq!(EpisodeRepo::new(db).by_anime_id(show).expect("rows").len(), 2);
    }

    #[test]
    fn choosing_a_folder_again_puts_its_series_back_in_the_library() {
        let (_db_dir, db) = anime_db();
        let root = tempfile::tempdir().expect("root");
        video(root.path(), "Show", "Show - 1.mkv");
        scan_root(&db, root.path()).expect("first scan");
        let repo = AnimeRepo::new(Arc::clone(&db));
        let url = root.path().join("Show").to_string_lossy().into_owned();
        let id = repo.by_url_and_source(&url, LOCAL_SOURCE_ID).expect("query").expect("row").id;
        repo.update(&AnimeUpdate { favorite: Some(false), ..AnimeUpdate::new(id) }).expect("unfavourite");

        scan_root(&db, root.path()).expect("second scan");
        assert!(repo.by_id(id).expect("row").favorite);
        assert_eq!(repo.by_url_and_source(&url, LOCAL_SOURCE_ID).expect("query").expect("row").id, id, "no duplicate row");
    }
}
