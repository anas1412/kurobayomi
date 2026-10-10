//! The local manga path end to end without a display: a folder with a directory chapter and a CBZ chapter is scanned,
//! stored, rescanned, and its pages are read through the reader's loader choice.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use kuro_archive::CbzWriter;
use kuro_data::chapter::{ChapterRepo, ChapterUpdate};
use kuro_data::manga::MangaRepo;
use kuro_db::Db;
use kuro_local::LOCAL_SOURCE_ID;

use super::reader::{build_loader, find_local_ids, local_format};
use super::scan::scan;
use super::sync::sync;

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

pub(super) fn series(root: &Path) -> PathBuf {
    let series = root.join("Series");
    folder_chapter(&series.join("Ch 1"), 2);
    cbz_chapter(&series.join("Ch 2.cbz"), 2);
    fs::write(series.join("notes.txt"), b"not a chapter").expect("notes");
    fs::create_dir_all(series.join(".hidden")).expect("hidden");
    series
}

#[test]
fn a_folder_is_stored_once_with_its_chapters_and_cover() {
    let root = tempfile::tempdir().expect("temp dir");
    let db = manga_db(root.path());
    let series = series(root.path());

    let scanned = scan(&series).expect("scan");
    let names: Vec<&str> = scanned.chapters.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["Ch 2", "Ch 1"], "newest first, hidden and text files skipped");
    let cover = scanned.cover.clone().unwrap_or_default();
    assert!(cover.starts_with("file://") && cover.ends_with("/Ch%201/001.png"), "{cover}");

    let done = sync(&db, &scanned, 1_000).expect("sync");
    assert_eq!((done.new_chapters, done.renamed_chapters), (2, 0));
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
    let first = scan(&series).expect("scan");
    let done = sync(&db, &first, 1_000).expect("sync");

    let chapters = ChapterRepo::new(Arc::clone(&db));
    let ch1 = chapters.by_manga_id(done.manga_id, false).expect("list").into_iter().find(|c| c.name == "Ch 1").expect("Ch 1");
    let update = ChapterUpdate { read: Some(true), last_page_read: Some(1), ..ChapterUpdate::new(ch1.id) };
    chapters.update(&update).expect("mark read");

    folder_chapter(&series.join("Ch 3"), 1);
    let second = scan(&series).expect("rescan");
    let again = sync(&db, &second, 2_000).expect("sync again");
    assert_eq!((again.new_chapters, again.renamed_chapters), (1, 0), "only Ch 3 is new");

    let after = chapters.by_id(ch1.id).expect("read").expect("Ch 1 still there");
    assert!(after.read, "read state survives the rescan");
    assert_eq!(after.last_page_read, 1, "the saved page survives the rescan");
    assert_eq!(chapters.by_manga_id(done.manga_id, false).expect("list").len(), 3);
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
    let done = sync(&db, &scan(&series).expect("scan"), 1_000).expect("sync");

    let chapter_id = ChapterRepo::new(Arc::clone(&db))
        .by_manga_id(done.manga_id, false)
        .expect("list")
        .iter()
        .find(|c| c.name == "Ch 2")
        .map(|c| c.id);
    assert_eq!(find_local_ids(&db, "Series", "Ch 2", None), Some((done.manga_id, chapter_id.unwrap_or(0))));
    assert_eq!(find_local_ids(&db, "Series", "Ch 9", None), Some((done.manga_id, 0)), "an unknown chapter keeps the entry");
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
    let a = sync(&db, &scan(&first).expect("scan"), 1_000).expect("sync a");
    let b = sync(&db, &scan(&second).expect("scan"), 1_000).expect("sync b");
    assert_ne!(a.manga_id, b.manga_id, "two folders with one name are two entries");

    let chapters = ChapterRepo::new(Arc::clone(&db));
    let in_b = chapters.by_manga_id(b.manga_id, false).expect("list")[0].id;
    assert_eq!(find_local_ids(&db, "Series", "Ch 1", Some(b.manga_id)), Some((b.manga_id, in_b)), "the open entry wins");
    assert_eq!(find_local_ids(&db, "Series", &in_b.to_string(), Some(b.manga_id)), Some((b.manga_id, in_b)), "a chapter id works as the name");
    assert_eq!(find_local_ids(&db, "Series", "Ch 1", Some(9_999)).map(|ids| ids.0), Some(a.manga_id), "an unknown preferred id falls back to the title");
}

#[test]
fn a_folder_and_an_archive_with_one_name_become_two_chapter_names() {
    let root = tempfile::tempdir().expect("temp dir");
    let series = series(root.path());
    cbz_chapter(&series.join("Ch 1.cbz"), 1);
    let scanned = scan(&series).expect("scan");
    let mut names: Vec<&str> = scanned.chapters.iter().map(|c| c.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, ["Ch 1", "Ch 1.cbz", "Ch 2"]);
}
