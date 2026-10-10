//! The local library end to end without a display (gui-wire-finish): one manga folder with a folder chapter and a CBZ
//! chapter, and one anime folder with two episode files, are scanned into temporary databases. The assertions read the
//! library rows, the chapter and episode lists, and check that read and watch progress survive a rescan.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use kuro_data::chapter::{ChapterRepo, ChapterUpdate};
use kuro_data::episode::{EpisodeRepo, EpisodeUpdate};
use kuro_domain::kind::EntryKind;
use kuro_domain::library::model::LibraryRow;
use kuro_domain::library::repo::LibraryRepo;
use kuro_local::LOCAL_SOURCE_ID;

use super::scan::scan;
use super::sync::sync;
use super::tests::{manga_db, series};
use crate::local_flow_anime::sync::rescan_library;
use crate::local_flow_anime::sync::scan_root;
use crate::local_flow_anime::sync::tests::anime_db;

/// A short real clip made by ffmpeg when it is installed. The scan reads only the file name, so a plain file of the
/// same name is the fallback on a machine without ffmpeg.
fn tiny_video(path: &Path) {
    let made = Command::new("ffmpeg")
        .args(["-v", "error", "-y", "-f", "lavfi", "-i", "color=c=black:s=16x16:d=0.2", "-c:v", "mpeg4"])
        .arg(path)
        .status();
    if !made.is_ok_and(|s| s.success()) {
        fs::write(path, b"video").expect("video file");
    }
}

fn library(db: &Arc<kuro_db::Db>, kind: EntryKind) -> Vec<LibraryRow> {
    LibraryRepo::new(Arc::clone(db)).all(kind).expect("library rows")
}

#[test]
fn local_manga_and_anime_scan_into_the_library_and_keep_progress_across_a_rescan() {
    let root = tempfile::tempdir().expect("temp dir");

    // manga: a folder chapter "Ch 1" and a CBZ chapter "Ch 2.cbz", stored in a temporary database
    let manga_db_dir = tempfile::tempdir().expect("temp dir");
    let mdb = manga_db(manga_db_dir.path());
    let manga_folder = series(root.path());
    let done = sync(&mdb, &scan(&manga_folder).expect("scan"), 1_000).expect("sync");

    let rows = library(&mdb, EntryKind::Manga);
    assert_eq!(rows.len(), 1, "one manga in the library");
    assert_eq!((rows[0].title.as_str(), rows[0].source, rows[0].total), ("Series", LOCAL_SOURCE_ID, 2));

    let chapters = ChapterRepo::new(Arc::clone(&mdb));
    let mut names: Vec<String> = chapters.by_manga_id(done.manga_id, false).expect("chapters").into_iter().map(|c| c.name).collect();
    names.sort();
    assert_eq!(names, ["Ch 1", "Ch 2"]);
    let ch2 = chapters.by_manga_id(done.manga_id, false).expect("chapters").into_iter().find(|c| c.name == "Ch 2").expect("Ch 2");
    chapters.update(&ChapterUpdate { read: Some(true), last_page_read: Some(1), ..ChapterUpdate::new(ch2.id) }).expect("progress");

    // a new chapter lands in the folder, then the library rescans it the way the Rescan action does
    fs::create_dir_all(manga_folder.join("Ch 3")).expect("Ch 3");
    fs::write(manga_folder.join("Ch 3").join("001.png"), b"page").expect("page");
    let again = sync(&mdb, &scan(&manga_folder).expect("rescan"), 2_000).expect("sync again");
    assert_eq!((again.new_chapters, again.renamed_chapters), (1, 0));
    let rows = library(&mdb, EntryKind::Manga);
    assert_eq!(rows[0].total, 3, "the new chapter is counted");
    let kept = chapters.by_id(ch2.id).expect("read").expect("Ch 2 still there");
    assert!(kept.read && kept.last_page_read == 1, "read state and saved page survive the rescan");

    // anime: a series folder with two episode files, stored in a temporary database
    let (_anime_db_dir, adb) = anime_db();
    let anime_root = root.path().join("anime");
    let show = anime_root.join("Show");
    fs::create_dir_all(&show).expect("series folder");
    tiny_video(&show.join("Show - 1.mkv"));
    tiny_video(&show.join("Show - 2.mkv"));
    let report = scan_root(&adb, &anime_root).expect("scan anime");
    assert_eq!((report.series, report.episodes_added), (1, 2));

    let rows = library(&adb, EntryKind::Anime);
    assert_eq!(rows.len(), 1, "one anime in the library");
    assert_eq!((rows[0].title.as_str(), rows[0].source, rows[0].total), ("Show", LOCAL_SOURCE_ID, 2));
    let episodes = EpisodeRepo::new(Arc::clone(&adb));
    let anime_id = rows[0].id;
    let mut names: Vec<String> = episodes.by_anime_id(anime_id).expect("episodes").into_iter().map(|e| e.name).collect();
    names.sort();
    assert_eq!(names, ["Show - 1", "Show - 2"]);
    let first = episodes.by_anime_id(anime_id).expect("episodes").into_iter().find(|e| e.name == "Show - 1").expect("episode 1");
    episodes.update(&EpisodeUpdate { seen: Some(true), last_second_seen: Some(612), ..EpisodeUpdate::new(first.id) }).expect("progress");

    // episode 2 is deleted and episode 3 is added, then the library rescans
    fs::remove_file(show.join("Show - 2.mkv")).expect("remove episode 2");
    tiny_video(&show.join("Show - 3.mkv"));
    let report = rescan_library(&adb, 2_000).expect("rescan anime");
    assert_eq!((report.episodes_added, report.episodes_removed), (1, 1));
    let kept = episodes.by_anime_id(anime_id).expect("episodes");
    let mut names: Vec<String> = kept.iter().map(|e| e.name.clone()).collect();
    names.sort();
    assert_eq!(names, ["Show - 1", "Show - 3"]);
    let watched = kept.iter().find(|e| e.name == "Show - 1").expect("episode 1 still there");
    assert_eq!((watched.id, watched.seen, watched.last_second_seen), (first.id, true, 612), "the row and its position survive");
    assert_eq!(library(&adb, EntryKind::Anime)[0].total, 2);
}
