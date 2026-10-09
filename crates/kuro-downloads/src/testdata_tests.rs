//! Checks of the fixtures committed under `testdata/` (T-23a-036, section 12 of the edge-case plan), and the
//! traceability of the test list: every id of section 13 is named by a test in this crate.
//! `testdata/make_fixtures.py` regenerates the binary fixtures; nothing writes them at test time.

use std::fs;
use std::path::{Path, PathBuf};

use crate::images::{resolve_extension, split_tall_image};
use crate::index::{DownloadIndex, SNAPSHOT_FILE, scan};
use crate::store::{QueueRow, QueueStore};

fn data(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata").join(relative)
}

#[test]
fn the_page_images_are_the_sizes_and_formats_the_plan_lists() {
    for name in ["tiny.jpg", "tiny.png", "tiny.webp", "tiny.gif"] {
        let bytes = fs::read(data(&format!("img/{name}"))).unwrap();
        assert_eq!(image::load_from_memory(&bytes).unwrap().width(), 8, "{name} is 8x8");
        let extension = resolve_extension(None, &bytes);
        assert_eq!(extension, name.rsplit('.').next().unwrap(), "{name} is sniffed as its own type");
    }
    let tall = image::open(data("img/tall_800x4000.png")).unwrap();
    assert_eq!((tall.width(), tall.height()), (800, 4000));
    let gif = fs::File::open(data("img/animated.gif")).unwrap();
    let decoder = image::codecs::gif::GifDecoder::new(std::io::BufReader::new(gif)).unwrap();
    assert_eq!(image::AnimationDecoder::into_frames(decoder).count(), 2, "two frames");
}

#[test]
fn broken_jpeg_has_a_jpeg_header_and_does_not_decode() {
    let bytes = fs::read(data("img/broken.jpg")).unwrap();
    assert_eq!(resolve_extension(None, &bytes), "jpg", "the header says JPEG (EC-56)");
    assert!(image::load_from_memory(&bytes).is_err());
}

#[test]
fn the_tall_fixture_splits_and_the_animated_gif_does_not() {
    let tmp = tempfile::tempdir().unwrap();
    let tall = tmp.path().join("001.png");
    fs::copy(data("img/tall_800x4000.png"), &tall).unwrap();
    assert!(split_tall_image(tmp.path(), &tall, "001", 2000).unwrap());
    assert!(tmp.path().join("001__001.jpg").is_file() && tmp.path().join("001__002.jpg").is_file());
    let gif = tmp.path().join("002.gif");
    fs::copy(data("img/animated.gif"), &gif).unwrap();
    assert!(!split_tall_image(tmp.path(), &gif, "002", 4).unwrap());
}

#[test]
fn the_index_tree_scans_into_the_source_manga_and_chapter_names() {
    let installed = vec![(1, "MangaDex (EN)".to_owned()), (2, "Unknown (XX)".to_owned())];
    let tree = scan(&data("index/tree"), &installed);
    let mangadex = &tree.sources[&1];
    assert_eq!(mangadex.dir_name, "MangaDex (EN)");
    assert_eq!(mangadex.manga["Berserk"].chapters.iter().cloned().collect::<Vec<_>>(), vec!["Chapter 1", "Chapter 2", "Chapter 3"]);
    assert!(mangadex.manga.contains_key("Vagabond"), "the lower-case folder is the same source");
    assert_eq!(tree.sources[&2].manga["Other"].chapters.len(), 1);
}

#[test]
fn the_corrupt_snapshot_fixture_is_deleted_when_the_index_opens() {
    let root = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();
    let snapshot = cache.path().join(SNAPSHOT_FILE);
    fs::copy(data("index/corrupt.bin"), &snapshot).unwrap();
    assert_eq!(fs::metadata(&snapshot).unwrap().len(), 16);
    let index = DownloadIndex::open(root.path(), cache.path());
    assert!(!snapshot.exists(), "the corrupt file is removed");
    assert_eq!(index.last_renew_ms(), None, "and nothing is trusted from it");
}

#[test]
fn the_queue_sql_fixture_gives_the_store_its_parent_tables() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(&fs::read_to_string(data("db/queue_v1.sql")).unwrap()).unwrap();
    let store = QueueStore::open(conn).unwrap();
    let row = |chapter_id, position| QueueRow { chapter_id, manga_id: 1, source_id: 1, position, status: 1, enqueued_at: 0 };
    store.replace_all(&[row(10, 0), row(12, 1)]).unwrap();
    assert_eq!(store.load().unwrap().iter().map(|r| r.chapter_id).collect::<Vec<_>>(), vec![10, 12]);
}

#[test]
fn the_comic_info_golden_file_is_the_one_the_comic_info_test_compares() {
    let golden = fs::read_to_string(data("comicinfo/golden.xml")).unwrap();
    assert!(golden.starts_with("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<ComicInfo "));
    assert!(golden.contains("<Series>Test: Manga/1</Series>") && golden.contains("<Number>12.5</Number>"));
}

/// Section 13 lists TST-01 to TST-52. Each id must appear in a test of the module that owns the function.
#[test]
fn every_test_id_of_the_plan_is_named_by_a_test() {
    let sources = [
        include_str!("queue.rs"),
        include_str!("scheduler.rs"),
        include_str!("manager.rs"),
        include_str!("downloader.rs"),
        include_str!("pipeline.rs"),
        include_str!("images.rs"),
        include_str!("throttle.rs"),
        include_str!("comicinfo.rs"),
        include_str!("sanitize.rs"),
        include_str!("layout.rs"),
        include_str!("index.rs"),
        include_str!("store.rs"),
        include_str!("pending.rs"),
        include_str!("notify.rs"),
    ];
    let missing: Vec<String> = (1..=52)
        .map(|n| format!("TST-{n:02}"))
        .filter(|id| !sources.iter().any(|s| s.contains(id.as_str())))
        .collect();
    assert!(missing.is_empty(), "no test names {missing:?}");
}
