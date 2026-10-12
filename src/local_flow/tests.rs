//! Local folders end to end without a display: manga and anime folders are scanned, stored, rescanned, and manga pages
//! are read through the reader's loader choice.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use kuro_archive::CbzWriter;
use kuro_data::anime::{AnimeRepo, AnimeUpdate};
use kuro_data::chapter::{ChapterRepo, ChapterUpdate};
use kuro_data::episode::{EpisodeRepo, EpisodeUpdate};
use kuro_data::manga::MangaRepo;
use kuro_db::Db;
use kuro_domain::kind::EntryKind;
use kuro_domain::library::model::LibraryRow;
use kuro_domain::library::repo::LibraryRepo;
use kuro_local::LOCAL_SOURCE_ID;

use super::reader::{build_loader, find_local_ids, local_format};
use super::scan::scan;
use super::sync::{Outcome, Synced, add, rescan, sync};

fn png(path: &Path) {
    image::RgbImage::from_pixel(8, 8, image::Rgb([200, 40, 40])).save(path).expect("write png");
}

/// A folder chapter with `pages` generated pages.
fn folder_chapter(dir: &Path, pages: usize) {
    fs::create_dir_all(dir).expect("chapter dir");
    for n in 1..=pages {
        png(&dir.join(format!("{n:03}.png")));
    }
}

/// A CBZ chapter built from generated pages, through the archive crate's own writer.
fn cbz_chapter(target: &Path, pages: usize) {
    let scratch = target.with_extension("pages");
    fs::create_dir_all(&scratch).expect("scratch dir");
    let mut writer = CbzWriter::create(target).expect("cbz writer");
    for n in 1..=pages {
        let page = scratch.join(format!("{n:03}.png"));
        png(&page);
        writer.add_file(&page).expect("add page");
    }
    writer.finish().expect("finish cbz");
    fs::remove_dir_all(&scratch).expect("remove scratch");
}

pub(super) fn manga_db(dir: &Path) -> Arc<Db> {
    let path = dir.join("manga.db");
    let writer = kuro_db::schema_manga::open(&path).expect("open manga db");
    Arc::new(Db::new(&path, writer).expect("manga db handle"))
}

/// An anime database in a temporary folder, opened the way the app opens it.
pub(super) fn anime_db() -> (tempfile::TempDir, Arc<Db>) {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("anime.db");
    let writer = kuro_db::schema_anime::open(&path).expect("open anime db");
    (dir, Arc::new(Db::new(&path, writer).expect("anime db handle")))
}

/// A manga folder "Series" with a folder chapter, a CBZ chapter, a text file and a hidden folder.
pub(super) fn series(root: &Path) -> PathBuf {
    let series = root.join("Series");
    folder_chapter(&series.join("Ch 1"), 2);
    cbz_chapter(&series.join("Ch 2.cbz"), 2);
    fs::write(series.join("notes.txt"), b"not a chapter").expect("notes");
    fs::create_dir_all(series.join(".hidden")).expect("hidden");
    series
}

/// An episode file in a series folder under `root`. The scan reads only the file name.
pub(super) fn video(root: &Path, series: &str, file: &str) {
    let dir = root.join(series);
    fs::create_dir_all(&dir).expect("series folder");
    fs::write(dir.join(file), b"video").expect("video file");
}

/// The rows the Library screen of `kind` lists, read the way the screen reads them.
fn library_rows(db: &Arc<Db>, kind: EntryKind) -> Vec<LibraryRow> {
    LibraryRepo::new(Arc::clone(db)).all(kind).expect("library rows")
}

/// `(series, added, removed)` over the folders that were stored, as the anime toast counts them.
fn counts(outcomes: &[Outcome]) -> (usize, usize, usize) {
    let done: Vec<&Synced> = outcomes.iter().filter_map(|o| o.result.as_ref().ok()).collect();
    (done.len(), done.iter().map(|d| d.added).sum(), done.iter().map(|d| d.removed).sum())
}

fn anime_id(db: &Arc<Db>, root: &Path, series: &str) -> i64 {
    let url = root.join(series).to_string_lossy().into_owned();
    AnimeRepo::new(Arc::clone(db)).by_url_and_source(&url, LOCAL_SOURCE_ID).expect("query").expect("row").id
}

#[test]
fn a_folder_is_stored_once_with_its_chapters_and_cover() {
    let root = tempfile::tempdir().expect("temp dir");
    let db = manga_db(root.path());
    let series = series(root.path());

    let scanned = scan(EntryKind::Manga, &series).expect("scan");
    let names: Vec<&str> = scanned.items.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["Ch 2", "Ch 1"], "newest first, hidden and text files skipped");
    let cover = scanned.cover.clone().unwrap_or_default();
    assert!(cover.starts_with("file://") && cover.ends_with("/Ch%201/001.png"), "{cover}");

    let done = sync(&db, EntryKind::Manga, &scanned, 1_000).expect("sync");
    assert_eq!((done.added, done.updated), (2, 0));
    let manga = MangaRepo::new(Arc::clone(&db))
        .by_url_and_source(&series.to_string_lossy(), LOCAL_SOURCE_ID)
        .expect("lookup")
        .expect("the entry is stored under the local source");
    assert!(manga.favorite, "a local folder joins the library");
    assert_eq!(manga.title, "Series");
    assert_eq!(manga.thumbnail_url.as_deref(), Some(cover.as_str()));
}

#[test]
fn a_rescan_adds_new_chapters_and_keeps_read_state() {
    let root = tempfile::tempdir().expect("temp dir");
    let db = manga_db(root.path());
    let series = series(root.path());
    let done = sync(&db, EntryKind::Manga, &scan(EntryKind::Manga, &series).expect("scan"), 1_000).expect("sync");

    let chapters = ChapterRepo::new(Arc::clone(&db));
    let ch1 = chapters.by_manga_id(done.entry_id, false).expect("list").into_iter().find(|c| c.name == "Ch 1").expect("Ch 1");
    let update = ChapterUpdate { read: Some(true), last_page_read: Some(1), ..ChapterUpdate::new(ch1.id) };
    chapters.update(&update).expect("mark read");

    folder_chapter(&series.join("Ch 3"), 1);
    let again = rescan(&db, EntryKind::Manga, 2_000).expect("rescan");
    assert_eq!(again.len(), 1, "one local manga in the library");
    let again = again[0].result.clone().expect("sync again");
    assert_eq!((again.added, again.updated), (1, 0), "only Ch 3 is new");

    let after = chapters.by_id(ch1.id).expect("read").expect("Ch 1 still there");
    assert!(after.read, "read state survives the rescan");
    assert_eq!(after.last_page_read, 1, "the saved page survives the rescan");
    assert_eq!(chapters.by_manga_id(done.entry_id, false).expect("list").len(), 3);
    assert_eq!(library_rows(&db, EntryKind::Manga)[0].total, 3, "the new chapter is counted");
}

#[test]
fn the_reader_loads_the_pages_of_a_folder_and_an_archive() {
    let root = tempfile::tempdir().expect("temp dir");
    let series = series(root.path());
    for chapter in ["Ch 1", "Ch 2.cbz"] {
        let url = series.join(chapter).to_string_lossy().into_owned();
        assert!(local_format(&url).is_some(), "{url} is a local chapter");
        let mut row = kuro_data::chapter::Chapter::create();
        row.url = url;
        let rc = crate::reader::model::ReaderChapter::new(row);
        let loader = build_loader(&rc).unwrap_or_else(|e| panic!("{chapter}: {}", e.message));
        assert_eq!(loader.get_pages().map(|p| p.len()).ok(), Some(2), "{chapter} has two pages");
    }
    assert!(local_format("Series/Ch 1").is_none(), "a relative url is not a local chapter");
}

#[test]
fn a_title_and_chapter_name_find_the_local_entry_and_its_chapter() {
    let root = tempfile::tempdir().expect("temp dir");
    let db = manga_db(root.path());
    let series = series(root.path());
    let done = sync(&db, EntryKind::Manga, &scan(EntryKind::Manga, &series).expect("scan"), 1_000).expect("sync");

    let chapter_id = ChapterRepo::new(Arc::clone(&db))
        .by_manga_id(done.entry_id, false)
        .expect("list")
        .iter()
        .find(|c| c.name == "Ch 2")
        .map(|c| c.id);
    assert_eq!(find_local_ids(&db, "Series", "Ch 2", None), Some((done.entry_id, chapter_id.unwrap_or(0))));
    assert_eq!(find_local_ids(&db, "Series", "Ch 9", None), Some((done.entry_id, 0)), "an unknown chapter keeps the entry");
    assert_eq!(find_local_ids(&db, "Other", "Ch 1", None), None);
}

#[test]
fn a_chapter_id_from_the_library_and_a_preferred_entry_pick_the_right_rows() {
    let root = tempfile::tempdir().expect("temp dir");
    let db = manga_db(root.path());
    let first = series(root.path());
    let other_parent = root.path().join("other");
    fs::create_dir_all(&other_parent).expect("other parent");
    let second = other_parent.join("Series");
    folder_chapter(&second.join("Ch 1"), 1);
    let a = sync(&db, EntryKind::Manga, &scan(EntryKind::Manga, &first).expect("scan"), 1_000).expect("sync a");
    let b = sync(&db, EntryKind::Manga, &scan(EntryKind::Manga, &second).expect("scan"), 1_000).expect("sync b");
    assert_ne!(a.entry_id, b.entry_id, "two folders with one name are two entries");

    let chapters = ChapterRepo::new(Arc::clone(&db));
    let in_b = chapters.by_manga_id(b.entry_id, false).expect("list")[0].id;
    assert_eq!(find_local_ids(&db, "Series", "Ch 1", Some(b.entry_id)), Some((b.entry_id, in_b)), "the open entry wins");
    assert_eq!(find_local_ids(&db, "Series", &in_b.to_string(), Some(b.entry_id)), Some((b.entry_id, in_b)), "a chapter id works as the name");
    assert_eq!(find_local_ids(&db, "Series", "Ch 1", Some(9_999)).map(|ids| ids.0), Some(a.entry_id), "an unknown preferred id falls back to the title");
}

#[test]
fn a_folder_and_an_archive_with_one_name_become_two_chapter_names() {
    let root = tempfile::tempdir().expect("temp dir");
    let series = series(root.path());
    cbz_chapter(&series.join("Ch 1.cbz"), 1);
    let scanned = scan(EntryKind::Manga, &series).expect("scan");
    let mut names: Vec<&str> = scanned.items.iter().map(|c| c.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, ["Ch 1", "Ch 1.cbz", "Ch 2"]);
}

#[test]
fn an_anime_folder_adds_each_series_with_its_episodes() {
    let (_db_dir, db) = anime_db();
    let root = tempfile::tempdir().expect("root");
    video(root.path(), "Show", "Show - 1.mkv");
    video(root.path(), "Show", "Show - 2.mkv");
    video(root.path(), "Other", "Other 01.mp4");
    fs::create_dir(root.path().join("Empty")).expect("empty series");

    let outcomes = add(&db, EntryKind::Anime, root.path(), 1_000).expect("scan");
    assert_eq!(counts(&outcomes), (2, 3, 0), "the folder without episodes is skipped");

    let shown = library_rows(&db, EntryKind::Anime);
    let show = shown.iter().find(|a| a.title == "Show").expect("Show in the library");
    assert_eq!((show.source, show.total), (LOCAL_SOURCE_ID, 2));
    let row = AnimeRepo::new(Arc::clone(&db)).by_id(show.id).expect("row");
    assert!(row.favorite);
    assert_eq!(row.url, root.path().join("Show").to_string_lossy());
    let rows = EpisodeRepo::new(db).by_anime_id(row.id).expect("episodes");
    let mut urls: Vec<String> = rows.iter().map(|e| e.url.clone()).collect();
    urls.sort();
    assert_eq!(urls.len(), 2);
    assert!(urls[0].ends_with("Show - 1.mkv") && urls[1].ends_with("Show - 2.mkv"));
    let mut orders: Vec<i64> = rows.iter().map(|e| e.source_order).collect();
    orders.sort_unstable();
    assert_eq!(orders, [0, 1]);
}

#[test]
fn an_anime_rescan_keeps_seen_state_and_position_and_follows_the_files() {
    let (_db_dir, db) = anime_db();
    let root = tempfile::tempdir().expect("root");
    video(root.path(), "Show", "Show - 1.mkv");
    video(root.path(), "Show", "Show - 2.mkv");
    add(&db, EntryKind::Anime, root.path(), 1_000).expect("first scan");
    let episodes = EpisodeRepo::new(Arc::clone(&db));
    let anime_id = anime_id(&db, root.path(), "Show");
    let first = episodes.by_anime_id(anime_id).expect("rows").into_iter().find(|e| e.name == "Show - 1").expect("episode 1");
    episodes.update(&EpisodeUpdate { seen: Some(true), last_second_seen: Some(612), ..EpisodeUpdate::new(first.id) }).expect("mark seen");

    fs::remove_file(root.path().join("Show").join("Show - 2.mkv")).expect("remove episode 2");
    video(root.path(), "Show", "Show - 3.mkv");
    assert_eq!(counts(&add(&db, EntryKind::Anime, root.path(), 2_000).expect("rescan")), (1, 1, 1));

    let rows = episodes.by_anime_id(anime_id).expect("rows");
    let kept = rows.iter().find(|e| e.name == "Show - 1").expect("episode 1 still there");
    assert_eq!((kept.id, kept.seen, kept.last_second_seen), (first.id, true, 612), "the row and its position survive");
    assert!(rows.iter().any(|e| e.name == "Show - 3"));
    assert!(!rows.iter().any(|e| e.name == "Show - 2"));
}

#[test]
fn an_anime_folder_without_episodes_is_not_added_and_an_existing_row_loses_its_episodes() {
    let (_db_dir, db) = anime_db();
    let root = tempfile::tempdir().expect("root");
    fs::create_dir(root.path().join("Bare")).expect("folder without episodes");
    assert!(add(&db, EntryKind::Anime, root.path(), 1_000).expect("scan").is_empty());
    assert!(library_rows(&db, EntryKind::Anime).is_empty());

    video(root.path(), "Show", "Show - 1.mkv");
    add(&db, EntryKind::Anime, root.path(), 1_000).expect("first scan");
    fs::remove_file(root.path().join("Show").join("Show - 1.mkv")).expect("remove");
    assert_eq!(counts(&add(&db, EntryKind::Anime, root.path(), 2_000).expect("rescan")).2, 1);
    assert!(EpisodeRepo::new(Arc::clone(&db)).by_anime_id(anime_id(&db, root.path(), "Show")).expect("rows").is_empty());
}

#[test]
fn an_anime_library_rescan_follows_its_series_and_leaves_removed_ones_out() {
    let (_db_dir, db) = anime_db();
    let root = tempfile::tempdir().expect("root");
    video(root.path(), "Show", "Show - 1.mkv");
    video(root.path(), "Gone", "Gone - 1.mkv");
    add(&db, EntryKind::Anime, root.path(), 1_000).expect("first scan");
    let repo = AnimeRepo::new(Arc::clone(&db));
    let gone = anime_id(&db, root.path(), "Gone");
    repo.update(&AnimeUpdate { favorite: Some(false), ..AnimeUpdate::new(gone) }).expect("unfavourite");
    video(root.path(), "Show", "Show - 2.mkv");
    video(root.path(), "New", "New - 1.mkv");

    assert_eq!(counts(&rescan(&db, EntryKind::Anime, 5).expect("rescan")), (1, 1, 0));
    assert!(!repo.by_id(gone).expect("row").favorite, "a removed series stays out of the library");
    assert!(library_rows(&db, EntryKind::Anime).iter().all(|a| a.title != "New"), "a folder that was never added is not picked up");
    assert_eq!(EpisodeRepo::new(Arc::clone(&db)).by_anime_id(anime_id(&db, root.path(), "Show")).expect("rows").len(), 2);
}

#[test]
fn choosing_an_anime_folder_again_puts_its_series_back_in_the_library() {
    let (_db_dir, db) = anime_db();
    let root = tempfile::tempdir().expect("root");
    video(root.path(), "Show", "Show - 1.mkv");
    add(&db, EntryKind::Anime, root.path(), 1_000).expect("first scan");
    let repo = AnimeRepo::new(Arc::clone(&db));
    let id = anime_id(&db, root.path(), "Show");
    repo.update(&AnimeUpdate { favorite: Some(false), ..AnimeUpdate::new(id) }).expect("unfavourite");

    add(&db, EntryKind::Anime, root.path(), 2_000).expect("second scan");
    assert!(repo.by_id(id).expect("row").favorite);
    assert_eq!(anime_id(&db, root.path(), "Show"), id, "no duplicate row");
}
