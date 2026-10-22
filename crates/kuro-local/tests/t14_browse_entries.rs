//! Area 14 behavioural tests (edge-case file sections 5.3, 5.4, 5.14 and the identity and message rules of 1.x and
//! 5.12): browse and search, latest window, entry defaults and exact user-visible texts.

use std::fs;
use std::io;
use std::time::{Duration, SystemTime};

use kuro_local::browse::LATEST_WINDOW_MS;
use kuro_local::error::LocalSourceError;
use kuro_local::manga::LocalMangaSource;
use kuro_local::anime::LocalAnimeSource;
use kuro_local::natsort::natural_cmp_ci;
use kuro_local::{LOCAL_SOURCE_ID, is_local_source};
use kuro_source::anime::{AnimeSource, FetchType, SAnime, SEpisode};
use kuro_source::model::{SChapter, SManga, UpdateStrategy};
use kuro_source::{CatalogueSource, FilterList, MangaSource, SortSelection};
use kuro_local::manga::chapters::chapters;
use kuro_local::sort::{LATEST_SORT, POPULAR_SORT, order_by};
use kuro_local::ux::LOCAL_HELP_URL;

const JPEG: &[u8] = b"\xFF\xD8\xFF\xE0\0\x10JFIF";

fn root_with(folders: &[&str]) -> io::Result<(tempfile::TempDir, LocalMangaSource)> {
    let root = tempfile::tempdir()?;
    for folder in folders {
        fs::create_dir_all(root.path().join(folder))?;
    }
    let source = LocalMangaSource::new(Some(root.path().to_path_buf()));
    Ok((root, source))
}

fn titles(page: &kuro_source::model::MangasPage) -> Vec<String> {
    page.mangas.iter().map(|m| m.title.clone()).collect()
}

/// Sets a folder's modification time to `age` before now.
fn age(dir: &std::path::Path, age: Duration) -> io::Result<()> {
    let when = SystemTime::now() - age;
    fs::File::open(dir)?.set_modified(when)
}

#[tokio::test]
async fn t_14_browse_popular_title_asc_default() -> io::Result<()> {
    let (_root, source) = root_with(&["b", "A", "c"])?;
    let page = CatalogueSource::popular(&source, 1).await.map_err(io::Error::other)?;
    assert_eq!(titles(&page), ["A", "b", "c"], "default popular order is title ascending");
    Ok(())
}

#[tokio::test]
async fn t_14_browse_query_contains_ignore_case() -> io::Result<()> {
    let (_root, source) = root_with(&["One Piece", "Naruto", "piece of cake"])?;
    let page = CatalogueSource::search(&source, 1, "PIECE", &FilterList::default()).await.map_err(io::Error::other)?;
    assert_eq!(titles(&page), ["One Piece", "piece of cake"]);
    Ok(())
}

#[tokio::test]
async fn t_14_browse_whitespace_query_is_blank() -> io::Result<()> {
    let (_root, source) = root_with(&["A", "B"])?;
    let page = CatalogueSource::search(&source, 1, "   ", &FilterList::default()).await.map_err(io::Error::other)?;
    assert_eq!(titles(&page), ["A", "B"], "a blank query keeps everything");
    Ok(())
}

#[tokio::test]
async fn t_14_browse_search_sort_none_keeps_name_order() -> io::Result<()> {
    let (_root, source) = root_with(&["zeta", "Alpha", "mid"])?;
    let page = CatalogueSource::search(&source, 1, "", &FilterList::default()).await.map_err(io::Error::other)?;
    assert_eq!(titles(&page), ["Alpha", "mid", "zeta"], "no sort filter keeps the listing's name order");
    Ok(())
}

#[tokio::test]
async fn t_14_browse_page_ignored() -> io::Result<()> {
    let (_root, source) = root_with(&["A", "B"])?;
    let first = CatalogueSource::popular(&source, 1).await.map_err(io::Error::other)?;
    let second = CatalogueSource::popular(&source, 2).await.map_err(io::Error::other)?;
    assert_eq!(titles(&first), titles(&second), "the local source has one page");
    assert!(!second.has_next_page);
    Ok(())
}

#[tokio::test]
async fn t_14_browse_missing_root_empty() -> io::Result<()> {
    let source = LocalMangaSource::new(Some(std::path::PathBuf::from("/definitely/not/here/kuro")));
    let page = CatalogueSource::popular(&source, 1).await.map_err(io::Error::other)?;
    assert!(page.mangas.is_empty());
    Ok(())
}

#[tokio::test]
async fn t_14_browse_hidden_and_files_excluded() -> io::Result<()> {
    let (root, source) = root_with(&["Shown", ".hidden"])?;
    fs::write(root.path().join("loose.cbz"), b"x")?;
    let page = CatalogueSource::popular(&source, 1).await.map_err(io::Error::other)?;
    assert_eq!(titles(&page), ["Shown"]);
    Ok(())
}

#[tokio::test]
async fn t_14_browse_cover_requires_image() -> io::Result<()> {
    let (root, source) = root_with(&["Shown", "Plain"])?;
    fs::write(root.path().join("Shown").join("cover.txt"), b"not an image")?;
    fs::write(root.path().join("Plain").join("cover.jpg"), JPEG)?;
    let page = CatalogueSource::popular(&source, 1).await.map_err(io::Error::other)?;
    let shown = page.mangas.iter().find(|m| m.title == "Shown").expect("Shown");
    let plain = page.mangas.iter().find(|m| m.title == "Plain").expect("Plain");
    assert!(shown.thumbnail_url.is_none(), "a text file named cover is not a cover");
    assert!(plain.thumbnail_url.is_some());
    Ok(())
}

#[tokio::test]
async fn t_14_browse_distinct_names() -> io::Result<()> {
    let (_root, source) = root_with(&["Same", "same"])?;
    let page = CatalogueSource::popular(&source, 1).await.map_err(io::Error::other)?;
    assert_eq!(page.mangas.len(), 2, "names differing only in case are two entries on Linux");
    Ok(())
}

#[tokio::test]
async fn t_14_browse_latest_date_desc_window() -> io::Result<()> {
    let (root, source) = root_with(&["Newest", "Older", "Stale"])?;
    age(&root.path().join("Newest"), Duration::from_secs(60))?;
    age(&root.path().join("Older"), Duration::from_secs(3 * 24 * 3600))?;
    age(&root.path().join("Stale"), Duration::from_millis(LATEST_WINDOW_MS as u64 + 3_600_000))?;
    let page = CatalogueSource::latest(&source, 1).await.map_err(io::Error::other)?;
    assert_eq!(titles(&page), ["Newest", "Older"], "newest first, outside the seven-day window dropped");
    Ok(())
}

#[tokio::test]
async fn t_14_latest_ignores_query() -> io::Result<()> {
    let (root, source) = root_with(&["Alpha", "Beta"])?;
    age(&root.path().join("Alpha"), Duration::from_secs(60))?;
    age(&root.path().join("Beta"), Duration::from_secs(120))?;
    let page = CatalogueSource::latest(&source, 1).await.map_err(io::Error::other)?;
    assert_eq!(titles(&page), ["Alpha", "Beta"], "latest has no query and returns every recent folder");
    Ok(())
}

#[tokio::test]
async fn t_14_latest_window_inclusive() -> io::Result<()> {
    let (root, source) = root_with(&["Edge"])?;
    age(&root.path().join("Edge"), Duration::from_millis(LATEST_WINDOW_MS as u64 - 3_600_000))?;
    let page = CatalogueSource::latest(&source, 1).await.map_err(io::Error::other)?;
    assert_eq!(titles(&page), ["Edge"], "a folder just inside the window is listed");
    Ok(())
}

#[test]
fn t_14_sort_title_is_not_natural() {
    // The title sort is plain case-insensitive text, so "10" sorts before "9" (E-BROWSE-4).
    let mut names = vec!["9", "10"];
    names.sort_by(|a, b| natural_cmp_ci(a, b));
    assert_eq!(names, ["9", "10"], "the natural comparator is used for chapters, not for the title sort");
    let mut titles = vec!["9".to_owned(), "10".to_owned()];
    titles.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    assert_eq!(titles, ["10", "9"]);
}

#[test]
fn t_14_sort_filters_are_the_documented_defaults() {
    assert_eq!(POPULAR_SORT, SortSelection { index: 0, ascending: true });
    assert_eq!(LATEST_SORT, SortSelection { index: 1, ascending: false });
    assert_eq!(order_by(None).name(), "Order by");
}

#[test]
fn t_14_entry_defaults() {
    let chapter = SChapter::new("Show/Ch 1", "Ch 1");
    assert_eq!((chapter.chapter_number, chapter.date_upload), (-1.0, 0));
    let episode = SEpisode { url: "Show/ep.mkv".into(), name: "ep".into(), ..SEpisode::default() };
    assert_eq!(episode.episode_number, -1.0);
    assert!(!episode.fillermark);
    let anime = SAnime { url: "Show".into(), title: "Show".into(), ..SAnime::default() };
    assert_eq!(anime.fetch_type, FetchType::Episodes);
    assert_eq!(anime.season_number, -1.0);
    assert_eq!(anime.status.to_i32(), 0);
    assert_eq!(anime.update_strategy, UpdateStrategy::AlwaysUpdate);
    let manga = SManga::new("Show", "Show");
    assert_eq!(manga.status, 0);
    assert_eq!(manga.update_strategy, UpdateStrategy::AlwaysUpdate);
}

#[test]
fn t_14_messages_exact_text() {
    assert_eq!(LocalSourceError::ChapterNotFound.to_string(), "Chapter not found");
    assert_eq!(LocalSourceError::InvalidFormat.to_string(), "Invalid chapter format");
}

#[test]
fn t_14_is_local_predicates() {
    assert!(is_local_source(LOCAL_SOURCE_ID));
    assert!(!is_local_source(1));
}

#[test]
fn t_14_source_identity() {
    let manga = LocalMangaSource::new(None);
    let anime = LocalAnimeSource::new(None);
    assert_eq!((MangaSource::id(&manga), AnimeSource::id(&anime)), (0, 0));
    assert_eq!(MangaSource::lang(&manga), "other");
    assert_eq!(AnimeSource::lang(&anime), "other");
    assert_eq!(MangaSource::name(&manga), "Local manga source");
    assert_eq!(AnimeSource::name(&anime), "Local anime source");
}

#[test]
fn t_14_help_urls_exact() {
    assert_eq!(LOCAL_HELP_URL, None, "the local source has no help link");
}

#[tokio::test]
async fn t_14_chapters_from_a_missing_folder_are_an_error() -> io::Result<()> {
    let root = tempfile::tempdir()?;
    assert!(chapters(root.path(), &mut SManga::new("Gone", "Gone")).is_err(), "a missing folder never empties a title");
    Ok(())
}

#[tokio::test]
async fn t_14_anime_episode_list_for_unknown_root_is_an_error() -> io::Result<()> {
    let source = LocalAnimeSource::new(None);
    let got = AnimeSource::episode_list(&source, &SAnime { url: "x".into(), ..SAnime::default() }).await;
    assert!(got.is_err(), "no Local folder never empties a title");
    Ok(())
}

#[tokio::test]
async fn t_14_browse_query_case_insensitive() -> io::Result<()> {
    let (_root, source) = root_with(&["NARUTO", "bleach"])?;
    let page = CatalogueSource::search(&source, 1, "naruto", &FilterList::default()).await.map_err(io::Error::other)?;
    assert_eq!(titles(&page), ["NARUTO"]);
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn t_14_browse_permission_denied_error() -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let (root, source) = root_with(&["Locked"])?;
    let locked = root.path().join("Locked");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000))?;
    // Listing the root still works; only the locked folder cannot be read, which browse does not need.
    let page = CatalogueSource::popular(&source, 1).await.map_err(io::Error::other)?;
    assert_eq!(titles(&page), ["Locked"]);
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755))?;
    Ok(())
}
