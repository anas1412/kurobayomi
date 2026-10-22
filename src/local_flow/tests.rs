//! Local titles without a display: linked folders are stored, their chapters and episodes are found by the screens, and
//! manga pages are read through the reader's loader choice. Reading the files into chapters and episodes is kuro-local's
//! and tested there; here the rows are written the way the library update stores what it reads.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use kuro_archive::CbzWriter;
use kuro_data::entry::EntryRepo;
use kuro_data::item::{Item, ItemRepo};
use kuro_db::Db;
use kuro_platform::kind::Kind;
use kuro_prefs::store::PrefStore;
use kuro_local::LOCAL_SOURCE_ID;

use super::reader::{build_loader, find_local_ids, local_format};
use super::titles;

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

/// A database in a temporary folder, opened the way the app opens it.
pub(super) fn test_db() -> (tempfile::TempDir, Arc<Db>) {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("test.db");
    let writer = kuro_db::schema_unified::open_file(&path).expect("open db");
    (dir, Arc::new(Db::new(&path, writer).expect("db handle")))
}

/// A manga folder "Series" under `root` with a folder chapter and a CBZ chapter, two pages each.
pub(super) fn series(root: &Path) -> PathBuf {
    let series = root.join("Series");
    folder_chapter(&series.join("Ch 1"), 2);
    cbz_chapter(&series.join("Ch 2.cbz"), 2);
    series
}

/// Links `folder` and stores `files` as its chapters or episodes, as the library update stores what the local source
/// reads: url `{folder}/{file}`, the name without the extension. Returns the entry id.
pub(super) fn linked(db: &Arc<Db>, kind: Kind, folder: &Path, files: &[&str]) -> i64 {
    for file in files {
        let path = folder.join(file);
        if !path.exists() {
            fs::create_dir_all(folder).expect("title folder");
            fs::write(&path, b"x").expect("file");
        }
    }
    titles::link(db, &PrefStore::in_memory(), kind, folder, 1_000).expect("link");
    let entry_id = EntryRepo::new(Arc::clone(db))
        .by_url_and_source(kind, &folder.to_string_lossy(), LOCAL_SOURCE_ID)
        .expect("lookup")
        .expect("the linked folder is stored")
        .id;
    let items: Vec<Item> = files
        .iter()
        .zip(0..)
        .map(|(file, order)| Item {
            entry_id,
            url: format!("{}/{file}", folder.display()),
            name: Path::new(file).file_stem().expect("name").to_string_lossy().into_owned(),
            source_order: order,
            ..Item::create(kind)
        })
        .collect();
    ItemRepo::new(Arc::clone(db)).add_all(&items).expect("items");
    entry_id
}

#[test]
fn the_reader_loads_the_pages_of_a_folder_and_an_archive() {
    let root = tempfile::tempdir().expect("temp dir");
    let series = series(root.path());
    for chapter in ["Ch 1", "Ch 2.cbz"] {
        let url = series.join(chapter).to_string_lossy().into_owned();
        assert!(local_format(&url).is_some(), "{url} is a local chapter");
        let mut row = Item::create(Kind::Manga);
        row.url = url;
        let rc = crate::reader::model::ReaderChapter::new(row);
        let loader = build_loader(&rc).unwrap_or_else(|e| panic!("{chapter}: {}", e.message));
        assert_eq!(loader.get_pages().map(|p| p.len()).ok(), Some(2), "{chapter} has two pages");
    }
    assert!(local_format("../Series/Ch 1").is_none(), "an url leaving its folder is not a local chapter");
}

#[test]
fn a_title_and_chapter_name_find_the_local_entry_and_its_chapter() {
    let (_d, db) = test_db();
    let root = tempfile::tempdir().expect("temp dir");
    let id = linked(&db, Kind::Manga, &series(root.path()), &["Ch 1", "Ch 2.cbz"]);

    let chapter_id = ItemRepo::new(Arc::clone(&db)).by_entry_id(id, false).expect("list").iter().find(|c| c.name == "Ch 2").map(|c| c.id);
    assert_eq!(find_local_ids(&db, "Series", "Ch 2", None), Some((id, chapter_id.unwrap_or(0))));
    assert_eq!(find_local_ids(&db, "Series", "Ch 9", None), Some((id, 0)), "an unknown chapter keeps the entry");
    assert_eq!(find_local_ids(&db, "Other", "Ch 1", None), None);
}

#[test]
fn a_chapter_id_from_the_library_and_a_preferred_entry_pick_the_right_rows() {
    let (_d, db) = test_db();
    let root = tempfile::tempdir().expect("temp dir");
    let a = linked(&db, Kind::Manga, &series(root.path()), &["Ch 1"]);
    let second = root.path().join("other").join("Series");
    folder_chapter(&second.join("Ch 1"), 1);
    let b = linked(&db, Kind::Manga, &second, &["Ch 1"]);
    assert_ne!(a, b, "two folders with one name are two entries");

    let in_b = ItemRepo::new(Arc::clone(&db)).by_entry_id(b, false).expect("list")[0].id;
    assert_eq!(find_local_ids(&db, "Series", "Ch 1", Some(b)), Some((b, in_b)), "the open entry wins");
    assert_eq!(find_local_ids(&db, "Series", &in_b.to_string(), Some(b)), Some((b, in_b)), "a chapter id works as the name");
    assert_eq!(find_local_ids(&db, "Series", "Ch 1", Some(9_999)).map(|ids| ids.0), Some(a), "an unknown preferred id falls back to the title");
}

#[test]
fn a_local_folder_joins_the_library_with_the_library_defaults_of_its_kind() {
    let (_d, db) = test_db();
    let root = tempfile::tempdir().expect("temp dir");
    let series = series(root.path());
    let defaults = kuro_db::flags::item::SORTING_NUMBER | kuro_db::flags::item::SORT_ASC;
    let prefs = PrefStore::in_memory();
    kuro_domain::entries::defaults::set_item_settings_default(&prefs, Kind::Manga, defaults);
    titles::link(&db, &prefs, Kind::Manga, &series, 1_000).expect("link");
    let row = EntryRepo::new(Arc::clone(&db)).by_url_and_source(Kind::Manga, &series.to_string_lossy(), LOCAL_SOURCE_ID).expect("lookup").expect("row");
    assert!(row.favorite);
    assert_eq!(row.item_flags, defaults, "a link writes the defaults");
}
