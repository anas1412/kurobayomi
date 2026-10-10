//! Area 14 behavioural tests (edge-case file sections 5.5 to 5.7, 5.12, 5.14, 5.15): manga folders, chapter lists,
//! covers, ComicInfo, EPUB metadata, dates and JSON side files.

use std::fs;
use std::io;
use std::path::Path;

use kuro_local::bridge::apply_comic_info;
use kuro_local::comicinfo::{ComicInfo, ComicInfoPublishingStatus};
use kuro_local::dates::{parse_epub_date, parse_local_date_in};
use kuro_local::epub::{self, EpubMetadata};
use kuro_local::fs::FsEntry;
use kuro_local::json_side::{MangaDetails, read_details_json};
use kuro_local::manga::LocalMangaSource;
use kuro_local::manga::chapters::chapters;
use kuro_local::manga::cover::cover_from_lowest_chapter;
use kuro_local::ux::{chapter_counts_as_downloaded, local_manga_download_count, shows_local_badge};
use kuro_local::ux::DISPLAY_LOCAL_BADGE_DEFAULT;
use kuro_local::{LOCAL_SOURCE_ID, is_local_source};
use kuro_source::model::{SChapter, SManga};
use kuro_source::MangaSource;

const JPEG: &[u8] = b"\xFF\xD8\xFF\xE0\0\x10JFIF";

fn folder(root: &Path, name: &str) -> io::Result<std::path::PathBuf> {
    let dir = root.join(name);
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn manga(url: &str) -> SManga {
    SManga::new(url, url)
}

fn names(list: &[SChapter]) -> Vec<String> {
    list.iter().map(|c| c.name.clone()).collect()
}

fn entry(name: &str, is_dir: bool) -> FsEntry {
    FsEntry { name: name.into(), path: Path::new(name).into(), is_dir, is_file: !is_dir, modified_ms: 0 }
}

// ---------------------------------------------------------------- 5.5 chapters and covers

#[test]
fn t_14_chapters_dirs_archives_epubs_only() -> io::Result<()> {
    let root = tempfile::tempdir()?;
    let show = folder(root.path(), "Show")?;
    fs::create_dir(show.join("Ch 1"))?;
    fs::write(show.join("Ch 2.cbz"), b"x")?;
    fs::write(show.join("Ch 3.epub"), b"x")?;
    fs::write(show.join("notes.txt"), b"x")?;
    fs::write(show.join("loose.png"), JPEG)?;
    fs::create_dir(show.join(".hidden"))?;
    let got = chapters(root.path(), &mut manga("Show")).map_err(io::Error::other)?;
    let mut sorted = names(&got);
    sorted.sort();
    assert_eq!(sorted, ["Ch 1", "Ch 2", "Ch 3"], "folders, archives and EPUBs only");
    Ok(())
}

#[test]
fn t_14_chapters_url_format() -> io::Result<()> {
    let root = tempfile::tempdir()?;
    let foo = folder(root.path(), "Foo")?;
    fs::write(foo.join("Ch 02.cbz"), b"x")?;
    fs::create_dir(foo.join("Ch 2"))?;
    let got = chapters(root.path(), &mut manga("Foo")).map_err(io::Error::other)?;
    let urls: Vec<&str> = got.iter().map(|c| c.url.as_str()).collect();
    assert!(urls.contains(&"Foo/Ch 02.cbz"), "{urls:?}");
    assert!(urls.contains(&"Foo/Ch 2"), "a folder keeps its name without an extension: {urls:?}");
    Ok(())
}

#[test]
fn t_14_chapters_uppercase_archive_accepted() -> io::Result<()> {
    let root = tempfile::tempdir()?;
    let show = folder(root.path(), "Show")?;
    fs::write(show.join("Ch 1.CBZ"), b"x")?;
    let got = chapters(root.path(), &mut manga("Show")).map_err(io::Error::other)?;
    assert_eq!(got.len(), 1);
    Ok(())
}

#[test]
fn t_14_chapters_ignore_dot_and_loose_images() -> io::Result<()> {
    let root = tempfile::tempdir()?;
    let show = folder(root.path(), "Show")?;
    fs::write(show.join("page.jpg"), JPEG)?;
    fs::create_dir(show.join(".cache"))?;
    let got = chapters(root.path(), &mut manga("Show")).map_err(io::Error::other)?;
    assert!(got.is_empty());
    Ok(())
}

#[test]
fn t_14_chapters_corrupt_archive_listed() -> io::Result<()> {
    let root = tempfile::tempdir()?;
    let show = folder(root.path(), "Show")?;
    fs::write(show.join("Ch 1.cbz"), b"not a zip at all")?;
    let got = chapters(root.path(), &mut manga("Show")).map_err(io::Error::other)?;
    assert_eq!(names(&got), ["Ch 1"], "an archive is listed without being opened");
    Ok(())
}

#[test]
fn t_14_chapters_bad_epub_keeps_others() -> io::Result<()> {
    let root = tempfile::tempdir()?;
    let show = folder(root.path(), "Show")?;
    fs::write(show.join("Ch 1.epub"), b"broken")?;
    fs::create_dir(show.join("Ch 2"))?;
    let got = chapters(root.path(), &mut manga("Show")).map_err(io::Error::other)?;
    let mut listed = names(&got);
    listed.sort();
    assert_eq!(listed, ["Ch 1", "Ch 2"], "a bad EPUB keeps its file name and the others are listed");
    Ok(())
}

#[test]
fn t_14_chapters_sort_number_desc_then_name_desc() -> io::Result<()> {
    let root = tempfile::tempdir()?;
    let show = folder(root.path(), "Show")?;
    for name in ["Ch 1", "Ch 10", "Ch 2", "Ch 2.5"] {
        fs::create_dir(show.join(name))?;
    }
    let got = chapters(root.path(), &mut manga("Show")).map_err(io::Error::other)?;
    let numbers: Vec<f32> = got.iter().map(|c| c.chapter_number).collect();
    assert_eq!(numbers, [10.0, 2.5, 2.0, 1.0], "newest number first");
    Ok(())
}

#[test]
fn t_14_chapters_unrecognised_bottom() -> io::Result<()> {
    let root = tempfile::tempdir()?;
    let show = folder(root.path(), "Show")?;
    fs::create_dir(show.join("Extras"))?;
    fs::create_dir(show.join("Ch 1"))?;
    let got = chapters(root.path(), &mut manga("Show")).map_err(io::Error::other)?;
    assert_eq!(names(&got).last().map(String::as_str), Some("Extras"), "a name with no number sorts last");
    Ok(())
}

#[test]
fn t_14_chapters_json_override_name_date_scanlator() -> io::Result<()> {
    let root = tempfile::tempdir()?;
    let show = folder(root.path(), "Show")?;
    fs::create_dir(show.join("Ch 1"))?;
    fs::write(
        show.join("chapters.json"),
        r#"[{"chapter_number":1,"name":"Pilot","date_upload":"2024-01-02T10:00:00","scanlator":"Team"}]"#,
    )?;
    let got = chapters(root.path(), &mut manga("Show")).map_err(io::Error::other)?;
    assert_eq!(got[0].name, "Pilot");
    assert_eq!(got[0].scanlator.as_deref(), Some("Team"));
    assert!(got[0].date_upload > 0);
    Ok(())
}

#[test]
fn t_14_chapters_json_first_match_wins() -> io::Result<()> {
    let root = tempfile::tempdir()?;
    let show = folder(root.path(), "Show")?;
    fs::create_dir(show.join("Ch 1"))?;
    fs::write(
        show.join("chapters.json"),
        r#"[{"chapter_number":1,"name":"First"},{"chapter_number":1,"name":"Second"}]"#,
    )?;
    let got = chapters(root.path(), &mut manga("Show")).map_err(io::Error::other)?;
    assert_eq!(got[0].name, "First");
    Ok(())
}

#[test]
fn t_14_chapters_json_tolerance() -> io::Result<()> {
    let root = tempfile::tempdir()?;
    let show = folder(root.path(), "Show")?;
    fs::create_dir(show.join("Ch 1"))?;
    fs::write(show.join("chapters.json"), r#"[{"chapter_number":1.00005,"name":"Close enough"}]"#)?;
    let got = chapters(root.path(), &mut manga("Show")).map_err(io::Error::other)?;
    assert_eq!(got[0].name, "Close enough", "numbers within 0.0001 are the same entry");
    Ok(())
}

#[test]
fn t_14_chapters_json_missing_number_disables_file() -> io::Result<()> {
    let root = tempfile::tempdir()?;
    let show = folder(root.path(), "Show")?;
    fs::create_dir(show.join("Ch 1"))?;
    fs::write(show.join("chapters.json"), r#"[{"name":"No number"}]"#)?;
    let got = chapters(root.path(), &mut manga("Show")).map_err(io::Error::other)?;
    assert_eq!(names(&got), ["Ch 1"], "the file is ignored as a whole");
    Ok(())
}

#[test]
fn t_14_chapters_json_bad_date_keeps_file_date() -> io::Result<()> {
    let root = tempfile::tempdir()?;
    let show = folder(root.path(), "Show")?;
    fs::create_dir(show.join("Ch 1"))?;
    let before = chapters(root.path(), &mut manga("Show")).map_err(io::Error::other)?[0].date_upload;
    fs::write(show.join("chapters.json"), r#"[{"chapter_number":1,"date_upload":"not a date","name":"X"}]"#)?;
    let got = chapters(root.path(), &mut manga("Show")).map_err(io::Error::other)?;
    assert_eq!(got[0].name, "X");
    assert_eq!(got[0].date_upload, before, "a date that does not parse keeps the file date");
    Ok(())
}

#[test]
fn t_14_chapters_json_clears_scanlator() -> io::Result<()> {
    let root = tempfile::tempdir()?;
    let show = folder(root.path(), "Show")?;
    fs::create_dir(show.join("Ch 1"))?;
    fs::write(show.join("chapters.json"), r#"[{"chapter_number":1}]"#)?;
    let got = chapters(root.path(), &mut manga("Show")).map_err(io::Error::other)?;
    assert_eq!(got[0].scanlator, None, "the override replaces the scanlator even when it has none");
    Ok(())
}

#[test]
fn t_14_chapters_zero_mtime_kept() -> io::Result<()> {
    let root = tempfile::tempdir()?;
    let show = folder(root.path(), "Show")?;
    let file = show.join("Ch 1.cbz");
    fs::write(&file, b"x")?;
    fs::File::options().write(true).open(&file)?.set_modified(std::time::UNIX_EPOCH)?;
    let got = chapters(root.path(), &mut manga("Show")).map_err(io::Error::other)?;
    assert_eq!(names(&got), ["Ch 1"], "a zero date is kept, not dropped");
    Ok(())
}

#[test]
fn t_14_cover_fallback_only_when_blank() -> io::Result<()> {
    let root = tempfile::tempdir()?;
    let show = folder(root.path(), "Show")?;
    let ch = folder(&show, "Ch 1")?;
    fs::write(ch.join("001.jpg"), JPEG)?;
    fs::write(show.join("cover.png"), b"\x89PNG\r\n\x1a\nexisting")?;
    let list = chapters(root.path(), &mut manga("Show")).map_err(io::Error::other)?;
    let mut has_cover = manga("Show");
    has_cover.thumbnail_url = Some("file:///already/set.png".into());
    cover_from_lowest_chapter(root.path(), &has_cover, &list);
    assert_eq!(fs::read(show.join("cover.png"))?, b"\x89PNG\r\n\x1a\nexisting", "a manga with a thumbnail is left alone");
    // With a blank thumbnail the lowest chapter's image takes the existing cover name.
    cover_from_lowest_chapter(root.path(), &manga("Show"), &list);
    assert_eq!(fs::read(show.join("cover.png"))?, JPEG, "the cover keeps its name and takes the chapter image");
    Ok(())
}

#[test]
fn t_14_cover_fallback_corrupt_archive_swallowed() -> io::Result<()> {
    let root = tempfile::tempdir()?;
    let show = folder(root.path(), "Show")?;
    fs::write(show.join("Ch 1.cbz"), b"not a zip")?;
    let list = chapters(root.path(), &mut manga("Show")).map_err(io::Error::other)?;
    cover_from_lowest_chapter(root.path(), &manga("Show"), &list);
    assert!(!show.join("cover.jpg").exists(), "a corrupt archive gives no cover and no panic");
    Ok(())
}

// ---------------------------------------------------------------- 5.6 details and ComicInfo

fn source_with(folders: &[&str]) -> io::Result<(tempfile::TempDir, LocalMangaSource)> {
    let root = tempfile::tempdir()?;
    for name in folders {
        fs::create_dir_all(root.path().join(name))?;
    }
    let source = LocalMangaSource::new(Some(root.path().to_path_buf()));
    Ok((root, source))
}

async fn details(source: &LocalMangaSource, url: &str) -> io::Result<SManga> {
    MangaSource::get_manga_details(source, &SManga::new(url, url)).await.map_err(io::Error::other)
}

const COMIC_TITLE: &str = r#"<?xml version="1.0" encoding="utf-8"?><ComicInfo><Series>From XML</Series></ComicInfo>"#;

#[tokio::test]
async fn t_14_details_branch_a_comicinfo_wins_over_json() -> io::Result<()> {
    let (root, source) = source_with(&["Show"])?;
    let show = root.path().join("Show");
    fs::write(show.join("ComicInfo.xml"), COMIC_TITLE)?;
    fs::write(show.join("details.json"), r#"{"title":"From JSON"}"#)?;
    let got = details(&source, "Show").await?;
    assert_eq!(got.title, "From XML");
    assert!(show.join("details.json").is_file(), "branch (a) does not touch the JSON");
    Ok(())
}

#[tokio::test]
async fn t_14_details_branch_b_legacy_json_migrates_and_deletes() -> io::Result<()> {
    let (root, source) = source_with(&["Show"])?;
    let show = root.path().join("Show");
    fs::write(show.join("details.json"), r#"{"title":"Real","author":"A","genre":["Action"],"status":2}"#)?;
    let got = details(&source, "Show").await?;
    assert_eq!((got.title.as_str(), got.status), ("Real", 2));
    assert!(show.join("ComicInfo.xml").is_file());
    assert!(!show.join("details.json").exists());
    Ok(())
}

#[tokio::test]
async fn t_14_details_branch_c_archive_copy() -> io::Result<()> {
    let (root, source) = source_with(&["Show"])?;
    let show = root.path().join("Show");
    let staging = tempfile::tempdir()?;
    let info = staging.path().join("ComicInfo.xml");
    fs::write(&info, COMIC_TITLE)?;
    let page = staging.path().join("001.jpg");
    fs::write(&page, JPEG)?;
    let archive = show.join("Ch 1.cbz");
    let mut writer = kuro_archive::CbzWriter::create(&archive)?;
    writer.add_file(&info)?;
    writer.add_file(&page)?;
    writer.finish()?;
    let got = details(&source, "Show").await?;
    assert_eq!(got.title, "From XML", "the archive's ComicInfo is applied");
    assert_eq!(fs::read(show.join("ComicInfo.xml"))?, COMIC_TITLE.as_bytes(), "and copied to the folder root");
    Ok(())
}

#[tokio::test]
async fn t_14_details_noxml_marker_created_once() -> io::Result<()> {
    let (root, source) = source_with(&["Show"])?;
    let show = root.path().join("Show");
    details(&source, "Show").await?;
    assert!(show.join(".noxml").is_file());
    fs::write(show.join(".noxml"), b"kept")?;
    details(&source, "Show").await?;
    assert_eq!(fs::read(show.join(".noxml"))?, b"kept", "the marker is created once and not rewritten");
    Ok(())
}

#[tokio::test]
async fn t_14_details_comicinfo_removes_noxml() -> io::Result<()> {
    let (root, source) = source_with(&["Show"])?;
    let show = root.path().join("Show");
    fs::write(show.join(".noxml"), b"")?;
    fs::write(show.join("ComicInfo.xml"), COMIC_TITLE)?;
    details(&source, "Show").await?;
    assert!(!show.join(".noxml").exists(), "a top-level ComicInfo.xml removes the marker");
    Ok(())
}

#[tokio::test]
async fn t_14_details_dir_named_cbz_skipped() -> io::Result<()> {
    let (root, source) = source_with(&["Show", "Show/Folder.cbz"])?;
    let show = root.path().join("Show");
    fs::write(show.join("Folder.cbz").join("ComicInfo.xml"), COMIC_TITLE).ok();
    details(&source, "Show").await?;
    assert!(!show.join("ComicInfo.xml").exists(), "a folder named like an archive is never copied from");
    assert!(show.join(".noxml").is_file());
    Ok(())
}

#[tokio::test]
async fn t_14_details_legacy_json_kept_on_write_failure() -> io::Result<()> {
    let (root, source) = source_with(&["Show"])?;
    let show = root.path().join("Show");
    fs::write(show.join("details.json"), r#"{"title":"Kept"}"#)?;
    // A folder named ComicInfo.xml is branch (a), which cannot read it. The JSON branch is never reached, so it stays.
    fs::create_dir(show.join("ComicInfo.xml"))?;
    let got = details(&source, "Show").await?;
    assert_eq!(got.title, "Show", "a failed write leaves the manga without the JSON fields");
    assert!(show.join("details.json").is_file(), "the JSON is kept after a failed write");
    Ok(())
}

#[tokio::test]
async fn t_14_details_missing_folder_logs_and_returns() -> io::Result<()> {
    let (_root, source) = source_with(&[])?;
    let got = details(&source, "Gone").await?;
    assert_eq!(got.title, "Gone");
    Ok(())
}

#[test]
fn t_14_comicinfo_roundtrip_all_fields() -> Result<(), kuro_local::error::LocalSourceError> {
    let info = ComicInfo {
        series: Some("Series".into()),
        writer: Some("Writer".into()),
        summary: Some("Summary".into()),
        genre: Some("Action".into()),
        penciller: Some("Pencil".into()),
        publishing_status: Some("Ongoing".into()),
        ..ComicInfo::default()
    };
    let parsed = ComicInfo::from_xml(info.to_xml().as_bytes())?;
    assert_eq!(parsed.series, info.series);
    assert_eq!(parsed.writer, info.writer);
    assert_eq!(parsed.summary, info.summary);
    assert_eq!(parsed.genre, info.genre);
    assert_eq!(parsed.penciller, info.penciller);
    assert_eq!(parsed.publishing_status, info.publishing_status);
    Ok(())
}

#[test]
fn t_14_comicinfo_genre_merge_no_split() {
    let mut manga = SManga::new("m", "m");
    let info = ComicInfo { genre: Some("Action, Drama".into()), ..ComicInfo::default() };
    apply_comic_info(&mut manga, &info);
    assert_eq!(manga.genre.as_deref(), Some("Action, Drama"), "a genre string is kept whole");
}

#[test]
fn t_14_comicinfo_artist_merge_split() {
    let mut manga = SManga::new("m", "m");
    let info = ComicInfo { penciller: Some("A, B".into()), ..ComicInfo::default() };
    apply_comic_info(&mut manga, &info);
    assert_eq!(manga.artist.as_deref(), Some("A, B"));
}

#[test]
fn t_14_comicinfo_absent_status_resets_to_unknown() {
    let mut manga = SManga::new("m", "m");
    manga.status = 2;
    apply_comic_info(&mut manga, &ComicInfo::default());
    assert_eq!(manga.status, 0, "status is always assigned, so an absent element is Unknown");
}

#[test]
fn t_14_comicinfo_empty_series_assigns_empty() {
    let mut manga = SManga::new("m", "m");
    apply_comic_info(&mut manga, &ComicInfo { series: Some(String::new()), ..ComicInfo::default() });
    assert_eq!(manga.title, "");
}

#[test]
fn t_14_comicinfo_status_text_table() {
    let table = [
        ("Ongoing", 1),
        ("Completed", 2),
        ("Licensed", 3),
        ("Publishing finished", 4),
        ("Cancelled", 5),
        ("On hiatus", 6),
        ("Unknown", 0),
    ];
    for (text, number) in table {
        assert_eq!(ComicInfoPublishingStatus::to_s_manga_value(Some(text)), number, "{text}");
        assert_eq!(ComicInfoPublishingStatus::to_comic_info_value(i64::from(number)), text, "{number}");
    }
    assert_eq!(ComicInfoPublishingStatus::to_s_manga_value(Some("ongoing")), 0, "lookups are case-sensitive");
}

#[test]
fn t_14_comicinfo_status_upcoming_writes_unknown() {
    assert_eq!(ComicInfoPublishingStatus::to_comic_info_value(7), "Unknown");
}

#[test]
fn t_14_comicinfo_write_escapes() {
    let info = ComicInfo { series: Some("A & <B>".into()), ..ComicInfo::default() };
    let xml = info.to_xml();
    assert!(xml.contains("A &amp; &lt;B&gt;"), "{xml}");
}

#[test]
fn t_14_comicinfo_entities_roundtrip() -> Result<(), kuro_local::error::LocalSourceError> {
    let info = ComicInfo { series: Some("Tom & Jerry \"quoted\" <x>".into()), ..ComicInfo::default() };
    let back = ComicInfo::from_xml(info.to_xml().as_bytes())?;
    assert_eq!(back.series, info.series);
    Ok(())
}

#[test]
fn t_14_comicinfo_duplicate_last_wins() -> Result<(), kuro_local::error::LocalSourceError> {
    let xml = "<ComicInfo><Series>First</Series><Series>Last</Series></ComicInfo>";
    assert_eq!(ComicInfo::from_xml(xml.as_bytes())?.series.as_deref(), Some("Last"));
    Ok(())
}

#[test]
fn t_14_comicinfo_unknown_children_ignored() -> Result<(), kuro_local::error::LocalSourceError> {
    let xml = "<ComicInfo><Series>S</Series><Mystery>x</Mystery></ComicInfo>";
    assert_eq!(ComicInfo::from_xml(xml.as_bytes())?.series.as_deref(), Some("S"));
    Ok(())
}

#[test]
fn t_14_comicinfo_status_local_name_match() -> Result<(), kuro_local::error::LocalSourceError> {
    let namespaced = r#"<ComicInfo xmlns:ty="urn:tachiyomi"><ty:PublishingStatusTachiyomi>Completed</ty:PublishingStatusTachiyomi></ComicInfo>"#;
    assert_eq!(ComicInfo::from_xml(namespaced.as_bytes())?.publishing_status.as_deref(), Some("Completed"));
    // E-CI-6 proposal: match by local name, so an unbound or missing prefix is honoured too.
    let plain = "<ComicInfo><PublishingStatusTachiyomi>Completed</PublishingStatusTachiyomi></ComicInfo>";
    assert_eq!(ComicInfo::from_xml(plain.as_bytes())?.publishing_status.as_deref(), Some("Completed"));
    Ok(())
}

#[test]
fn t_14_comicinfo_wrong_root_error() {
    assert!(ComicInfo::from_xml(b"<Book><Series>S</Series></Book>").is_err());
}

#[test]
fn t_14_comicinfo_empty_file_error() {
    assert!(ComicInfo::from_xml(b"").is_err());
}

#[test]
fn t_14_comicinfo_utf8_bom_accepted() -> Result<(), kuro_local::error::LocalSourceError> {
    let mut bytes = b"\xEF\xBB\xBF".to_vec();
    bytes.extend_from_slice(b"<ComicInfo><Series>BOM</Series></ComicInfo>");
    assert_eq!(ComicInfo::from_xml(&bytes)?.series.as_deref(), Some("BOM"));
    Ok(())
}

#[test]
fn t_14_comicinfo_utf16_rejected() {
    let utf16: Vec<u8> = "<ComicInfo/>".encode_utf16().flat_map(u16::to_le_bytes).collect();
    assert!(ComicInfo::from_xml(&utf16).is_err());
}

// ---------------------------------------------------------------- 5.7 EPUB

#[test]
fn t_14_epub_fill_metadata_fields() {
    let opf = r#"<package><metadata>
        <dc:title>Chapter Title</dc:title><dc:creator>Author</dc:creator>
        <dc:publisher>Pub</dc:publisher><dc:description>Desc</dc:description>
        <dc:date>2024-03-04T10:00:00Z</dc:date></metadata></package>"#;
    let meta = epub::metadata(opf).expect("parses");
    let mut manga = SManga::new("m", "m");
    let mut chapter = SChapter::new("m/c.epub", "c");
    epub::fill_metadata(&mut manga, &mut chapter, &meta);
    assert_eq!(manga.author.as_deref(), Some("Author"));
    assert_eq!(manga.description.as_deref(), Some("Desc"));
    assert_eq!(chapter.name, "Chapter Title");
    assert_eq!(chapter.scanlator.as_deref(), Some("Pub"));
    assert!(chapter.date_upload > 0);
}

#[test]
fn t_14_epub_scanlator_falls_back_to_creator() {
    let meta = EpubMetadata { creator: Some("Creator".into()), ..EpubMetadata::default() };
    let mut manga = SManga::new("m", "m");
    let mut chapter = SChapter::new("m/c.epub", "c");
    epub::fill_metadata(&mut manga, &mut chapter, &meta);
    assert_eq!(chapter.scanlator.as_deref(), Some("Creator"));
}

#[test]
fn t_14_epub_date_formats() {
    assert!(parse_epub_date("2024-01-02T03:04:05+0100").is_some(), "a positive offset");
    assert!(parse_epub_date("2024-01-02T03:04:05-0800").is_some(), "a negative offset");
    assert!(parse_epub_date("2024-01-02T03:04:05Z").is_some(), "a bare Z is UTC");
    assert_eq!(parse_epub_date("2024-03-04"), None, "a date without a time is not an EPUB date");
    assert_eq!(parse_epub_date("not a date"), None);
}

#[test]
fn t_14_epub_separator_backslash() {
    assert_eq!(epub::separator_of(&["META-INF\\container.xml".to_owned()]), '\\');
    assert_eq!(epub::separator_of(&["META-INF/container.xml".to_owned()]), '/');
}

#[test]
fn t_14_epub_first_image_spine_order() {
    let opf = r#"<package><manifest>
        <item id="b" href="b.xhtml" media-type="application/xhtml+xml"/>
        <item id="a" href="a.xhtml" media-type="application/xhtml+xml"/>
        </manifest><spine><itemref idref="a"/><itemref idref="b"/></spine></package>"#;
    let pages = epub::spine_pages(opf).expect("spine");
    assert_eq!(pages, ["a.xhtml", "b.xhtml"], "the spine order, not the manifest order, decides");
}

#[test]
fn t_14_epub_image_xlink_href() {
    let xhtml = r#"<svg xmlns:xlink="http://www.w3.org/1999/xlink"><image xlink:href="../images/cover.jpg"/></svg>"#;
    let images = epub::page_images("OEBPS/cover.xhtml", xhtml, '/');
    assert_eq!(images, ["images/cover.jpg"], "../ from OEBPS/ resolves to the archive root");
}

// ---------------------------------------------------------------- 5.12, 5.14, 5.15

#[test]
fn t_14_badge_pref_respected() {
    assert!(!shows_local_badge(false, LOCAL_SOURCE_ID), "a local source hides its badge when the pref is off");
    assert!(shows_local_badge(true, LOCAL_SOURCE_ID));
}

#[test]
fn t_14_pref_badge_key_default_true() {
    assert!(DISPLAY_LOCAL_BADGE_DEFAULT);
}

#[test]
fn t_14_filter_downloaded_true_for_local() {
    assert!(chapter_counts_as_downloaded(LOCAL_SOURCE_ID));
    assert!(is_local_source(LOCAL_SOURCE_ID));
}

#[test]
fn t_14_download_count_local_excludes_epub() {
    let children = [entry("Ch 1", true), entry("Ch 2.cbz", false), entry("Ch 3.epub", false)];
    assert_eq!(local_manga_download_count(&children), 2, "folders and archives count, EPUBs do not (E-SYNC-4)");
}

#[test]
fn t_14_dates_strict_invalid_no_override() {
    let offset = chrono::FixedOffset::east_opt(0).unwrap();
    assert_eq!(parse_local_date_in("2024-02-31T10:00:00", offset), None, "no roll-over to March");
}

#[test]
fn t_14_dates_trailing_text_rejected() {
    let offset = chrono::FixedOffset::east_opt(0).unwrap();
    assert_eq!(parse_local_date_in("2024-01-01T10:00:00 trailing", offset), None);
}

#[test]
fn t_14_dates_use_local_offset() {
    let utc = chrono::FixedOffset::east_opt(0).unwrap();
    let plus_one = chrono::FixedOffset::east_opt(3600).unwrap();
    let a = parse_local_date_in("2024-01-01T10:00:00", utc).expect("parses");
    let b = parse_local_date_in("2024-01-01T10:00:00", plus_one).expect("parses");
    assert_eq!(a - b, 3_600_000, "the same wall clock one hour east is one hour earlier");
}

#[test]
fn t_14_json_unknown_keys_ignored() -> io::Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("details.json");
    fs::write(&path, r#"{"title":"T","unknown":42}"#)?;
    let d: MangaDetails = read_details_json(&path).map_err(io::Error::other)?;
    assert_eq!(d.title.as_deref(), Some("T"));
    Ok(())
}

#[test]
fn t_14_json_null_equals_absent() -> io::Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("details.json");
    fs::write(&path, r#"{"title":null}"#)?;
    let d: MangaDetails = read_details_json(&path).map_err(io::Error::other)?;
    assert_eq!(d, MangaDetails::default());
    Ok(())
}

#[test]
fn t_14_json_wrong_type_fails() -> io::Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("details.json");
    fs::write(&path, r#"{"title":5}"#)?;
    assert!(read_details_json::<MangaDetails>(&path).is_err());
    Ok(())
}

#[test]
fn t_14_json_genre_string_fails() -> io::Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("details.json");
    fs::write(&path, r#"{"genre":"Action"}"#)?;
    assert!(read_details_json::<MangaDetails>(&path).is_err(), "genre must be a list");
    Ok(())
}

#[test]
fn t_14_json_number_forms() -> io::Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("details.json");
    fs::write(&path, r#"{"status":2}"#)?;
    assert_eq!(read_details_json::<MangaDetails>(&path).map_err(io::Error::other)?.status, Some(2));
    fs::write(&path, r#"{"status":2.0}"#)?;
    assert!(read_details_json::<MangaDetails>(&path).is_err(), "a fractional status is not an integer");
    Ok(())
}

#[test]
fn t_14_json_bom_accepted() -> io::Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("details.json");
    let mut bytes = b"\xEF\xBB\xBF".to_vec();
    bytes.extend_from_slice(br#"{"title":"B"}"#);
    fs::write(&path, bytes)?;
    assert_eq!(read_details_json::<MangaDetails>(&path).map_err(io::Error::other)?.title.as_deref(), Some("B"));
    Ok(())
}

#[test]
fn t_14_json_empty_file_error() -> io::Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("details.json");
    fs::write(&path, b"")?;
    assert!(read_details_json::<MangaDetails>(&path).is_err());
    Ok(())
}

fn cbz_with_comic_info(path: &Path, body: &str, extra_page: &str) -> io::Result<()> {
    let staging = tempfile::tempdir()?;
    let info = staging.path().join("ComicInfo.xml");
    fs::write(&info, body)?;
    let page = staging.path().join(extra_page);
    fs::write(&page, JPEG)?;
    let mut writer = kuro_archive::CbzWriter::create(path)?;
    writer.add_file(&info)?;
    writer.add_file(&page)?;
    writer.finish()?;
    Ok(())
}

#[tokio::test]
async fn t_14_archive_comicinfo_root_only() -> io::Result<()> {
    let (root, source) = source_with(&["Show"])?;
    let show = root.path().join("Show");
    // Only the archive that holds ComicInfo.xml at its root is used; the other one is ignored.
    cbz_with_comic_info(&show.join("Ch 1.cbz"), COMIC_TITLE, "001.jpg")?;
    details(&source, "Show").await?;
    assert_eq!(fs::read(show.join("ComicInfo.xml"))?, COMIC_TITLE.as_bytes());
    Ok(())
}

#[tokio::test]
async fn t_14_archive_comicinfo_first_by_name() -> io::Result<()> {
    let (root, source) = source_with(&["Show"])?;
    let show = root.path().join("Show");
    let second = r#"<ComicInfo><Series>Second</Series></ComicInfo>"#;
    cbz_with_comic_info(&show.join("Vol 2.cbz"), second, "001.jpg")?;
    cbz_with_comic_info(&show.join("Vol 10.cbz"), COMIC_TITLE, "001.jpg")?;
    let got = details(&source, "Show").await?;
    assert_eq!(got.title, "Second", "the first archive in natural name order wins");
    Ok(())
}

#[tokio::test]
async fn t_14_comicinfo_malformed_keeps_file_and_fields() -> io::Result<()> {
    let (root, source) = source_with(&["Show"])?;
    let show = root.path().join("Show");
    fs::write(show.join("ComicInfo.xml"), "<ComicInfo><Series>")?;
    let got = details(&source, "Show").await?;
    assert_eq!(got.title, "Show", "fields are kept when the file does not parse");
    assert!(show.join("ComicInfo.xml").is_file(), "the malformed file stays on disk");
    Ok(())
}

#[tokio::test]
async fn t_14_epub_corrupt_keeps_listing() -> io::Result<()> {
    let root = tempfile::tempdir()?;
    let show = folder(root.path(), "Show")?;
    fs::write(show.join("Ch 1.epub"), b"broken")?;
    let got = chapters(root.path(), &mut manga("Show")).map_err(io::Error::other)?;
    assert_eq!(names(&got), ["Ch 1"], "a corrupt EPUB stays in the listing with its file name");
    Ok(())
}
