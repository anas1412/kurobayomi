//! Area 14 behavioural tests (edge-case file sections 5.8, 5.9, 5.10): anime folders, seasons, episodes, playback
//! hosters and the ffmpeg frame path. Tests that need `ffmpeg` skip with a printed reason when it is not installed.

use std::fs;
use std::io;
use std::path::Path;

use kuro_local::LocalAnimeSource;
use kuro_local::anime::fetch_type_of;
use kuro_local::format::Format;
use kuro_local::fs::FsEntry;
use kuro_local::manga::reader::format_of_chapter;
use kuro_local::media::{FrameExtractor, Tool, frame_position};
use kuro_local::error::LocalSourceError;
use kuro_source::anime::{AnimeSource, FetchType, SAnime, SEpisode};

fn entry(name: &str, is_dir: bool) -> FsEntry {
    FsEntry {
        name: name.into(),
        path: Path::new(name).to_path_buf(),
        is_dir,
        is_file: !is_dir,
        modified_ms: 0,
    }
}

/// A local anime root with the given folders and files (`folder`, `[file names]`).
fn library(folders: &[(&str, &[&str])]) -> io::Result<(tempfile::TempDir, LocalAnimeSource)> {
    let root = tempfile::tempdir()?;
    for (folder, files) in folders {
        let dir = root.path().join(folder);
        fs::create_dir_all(&dir)?;
        for file in *files {
            fs::write(dir.join(file), b"x")?;
        }
    }
    let source = LocalAnimeSource::new(Some(root.path().to_path_buf()));
    Ok((root, source))
}

fn anime(url: &str) -> SAnime {
    SAnime { url: url.into(), title: url.into(), ..SAnime::default() }
}

fn episode(url: &str) -> SEpisode {
    SEpisode { url: url.into(), name: url.into(), ..SEpisode::default() }
}

/// Fake `ffprobe` and `ffmpeg` scripts. The probe prints `seconds`, and ffmpeg writes `frame` to its output argument.
fn fake_tools(seconds: &str, frame: &str) -> io::Result<(tempfile::TempDir, FrameExtractor, tempfile::TempDir)> {
    let tools = tempfile::tempdir()?;
    let probe = tools.path().join("ffprobe.sh");
    let ffmpeg = tools.path().join("ffmpeg.sh");
    fs::write(&probe, format!("echo {seconds}\n"))?;
    fs::write(&ffmpeg, format!("printf '{frame}' > \"$9\"\n"))?;
    let temp = tempfile::tempdir()?;
    let frames = FrameExtractor::new(
        Some(Tool::new("sh").with_args([probe])),
        Some(Tool::new("sh").with_args([ffmpeg])),
        temp.path(),
    );
    Ok((tools, frames, temp))
}

fn has_ffmpeg() -> bool {
    Tool::on_path("ffmpeg").is_some() && Tool::on_path("ffprobe").is_some()
}

#[tokio::test]
async fn t_14_anime_browse_fetch_type_and_images() -> io::Result<()> {
    let (root, source) = library(&[("Show", &["ep.mkv", "cover.jpg"])])?;
    fs::write(root.path().join("Show").join("background.png"), b"\x89PNG\r\n\x1a\n")?;
    let page = AnimeSource::popular(&source, 1).await.map_err(io::Error::other)?;
    let show = page.animes.iter().find(|a| a.title == "Show").expect("Show");
    assert_eq!(show.fetch_type, FetchType::Episodes);
    assert!(show.thumbnail_url.as_deref().is_some_and(|u| u.ends_with("/Show/cover.jpg")));
    assert!(show.background_url.as_deref().is_some_and(|u| u.ends_with("/Show/background.png")));
    Ok(())
}

#[test]
fn t_14_fetch_type_episode_wins_over_dir() {
    assert_eq!(fetch_type_of(&[entry("S1", true), entry("ep.mkv", false)]), FetchType::Episodes);
}

#[test]
fn t_14_fetch_type_dir_only_is_seasons() {
    assert_eq!(fetch_type_of(&[entry("Season 1", true)]), FetchType::Seasons);
}

#[test]
fn t_14_fetch_type_hidden_dir_counts() {
    assert_eq!(fetch_type_of(&[entry(".hidden", true)]), FetchType::Seasons, "a hidden folder alone means seasons (E-ANIME-8)");
}

#[test]
fn t_14_fetch_type_mixed_is_episodes() {
    assert_eq!(fetch_type_of(&[entry("S1", true), entry("ep.mp4", false), entry("notes", false)]), FetchType::Episodes);
}

#[tokio::test]
async fn t_14_seasons_list_names_and_urls() -> io::Result<()> {
    let (root, source) = library(&[("Show", &[])])?;
    fs::create_dir_all(root.path().join("Show").join("Season 1"))?;
    fs::create_dir_all(root.path().join("Show").join(".hidden"))?;
    fs::write(root.path().join("Show").join("loose.mkv"), b"x")?;
    let seasons = AnimeSource::season_list(&source, &anime("Show")).await.map_err(io::Error::other)?;
    assert_eq!(seasons.len(), 1, "hidden folders and files are not seasons");
    assert_eq!(seasons[0].url, "Show/Season 1");
    assert_eq!(seasons[0].title, "Season 1");
    Ok(())
}

#[tokio::test]
async fn t_14_seasons_nested_urls_resolve() -> io::Result<()> {
    let (_root, source) = library(&[("Show/Season 1", &["Show - 1.mkv"])])?;
    let got = AnimeSource::episode_list(&source, &anime("Show/Season 1")).await.map_err(io::Error::other)?;
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].url, "Show/Season 1/Show - 1.mkv");
    Ok(())
}

#[tokio::test]
async fn t_14_seasons_empty_season_episode_list_empty() -> io::Result<()> {
    let (_root, source) = library(&[("Show/Season 1", &[])])?;
    let got = AnimeSource::episode_list(&source, &anime("Show/Season 1")).await.map_err(io::Error::other)?;
    assert!(got.is_empty());
    Ok(())
}

#[tokio::test]
async fn t_14_episodes_number_uses_anime_title() -> io::Result<()> {
    let (_root, source) = library(&[("Show", &["Show 3.mkv"])])?;
    let got = AnimeSource::episode_list(&source, &anime("Show")).await.map_err(io::Error::other)?;
    assert_eq!(got[0].episode_number, 3.0);
    Ok(())
}

#[tokio::test]
async fn t_14_episodes_uppercase_rejected() -> io::Result<()> {
    let (_root, source) = library(&[("Show", &["EP01.MKV", "ep01.mkv"])])?;
    let got = AnimeSource::episode_list(&source, &anime("Show")).await.map_err(io::Error::other)?;
    let urls: Vec<&str> = got.iter().map(|e| e.url.as_str()).collect();
    assert_eq!(urls, ["Show/ep01.mkv"]);
    Ok(())
}

#[tokio::test]
async fn t_14_episodes_dir_named_mkv_excluded() -> io::Result<()> {
    let (root, source) = library(&[("Show", &["Show 1.mkv"])])?;
    fs::create_dir(root.path().join("Show").join("Extra.mkv"))?;
    let got = AnimeSource::episode_list(&source, &anime("Show")).await.map_err(io::Error::other)?;
    assert_eq!(got.len(), 1, "a folder named like an episode is not an episode");
    Ok(())
}

#[tokio::test]
async fn t_14_episodes_json_bad_fails_listing() -> io::Result<()> {
    let (root, source) = library(&[("Show", &["Show 1.mkv"])])?;
    fs::write(root.path().join("Show").join("episodes.json"), "{")?;
    assert!(AnimeSource::episode_list(&source, &anime("Show")).await.is_err());
    Ok(())
}

#[tokio::test]
async fn t_14_episodes_json_missing_number_fails() -> io::Result<()> {
    let (root, source) = library(&[("Show", &["Show 1.mkv"])])?;
    fs::write(root.path().join("Show").join("episodes.json"), r#"[{"name":"no number"}]"#)?;
    assert!(AnimeSource::episode_list(&source, &anime("Show")).await.is_err());
    Ok(())
}

#[tokio::test]
async fn t_14_episodes_json_overrides_summary_scanlator() -> io::Result<()> {
    let (root, source) = library(&[("Show", &["Show 1.mkv"])])?;
    fs::write(
        root.path().join("Show").join("episodes.json"),
        r#"[{"episode_number":1,"summary":"Plot","scanlator":"Team","fillermark":true,"preview_url":"x"}]"#,
    )?;
    let got = AnimeSource::episode_list(&source, &anime("Show")).await.map_err(io::Error::other)?;
    assert_eq!(got[0].summary.as_deref(), Some("Plot"));
    assert_eq!(got[0].scanlator.as_deref(), Some("Team"));
    Ok(())
}

#[tokio::test]
async fn t_14_episodes_json_preview_and_filler_ignored() -> io::Result<()> {
    let (root, source) = library(&[("Show", &["Show 1.mkv"])])?;
    fs::write(
        root.path().join("Show").join("episodes.json"),
        r#"[{"episode_number":1,"fillermark":true,"preview_url":"https://example.invalid/p.jpg"}]"#,
    )?;
    let got = AnimeSource::episode_list(&source, &anime("Show")).await.map_err(io::Error::other)?;
    assert!(!got[0].fillermark, "fillermark in episodes.json is not applied");
    assert_ne!(got[0].preview_url.as_deref(), Some("https://example.invalid/p.jpg"), "preview_url is not applied");
    Ok(())
}

#[tokio::test]
async fn t_14_anime_details_bad_json_fails_update() -> io::Result<()> {
    let (root, source) = library(&[("Show", &[])])?;
    fs::write(root.path().join("Show").join("details.json"), "{")?;
    assert!(AnimeSource::anime_details(&source, &anime("Show")).await.is_err());
    Ok(())
}

#[tokio::test]
async fn t_14_anime_cover_from_lowest_episode() -> io::Result<()> {
    let (tools, frames, temp) = fake_tools("10", "jpeg-of-ep")?;
    let (root, _) = library(&[("Show", &["Show 2.mkv", "Show 1.mkv"])])?;
    let source = LocalAnimeSource::with_frames(Some(root.path().to_path_buf()), frames);
    AnimeSource::episode_list(&source, &anime("Show")).await.map_err(io::Error::other)?;
    let cover = fs::read(root.path().join("Show").join("cover.jpg"))?;
    assert_eq!(cover, b"jpeg-of-ep", "the cover comes from the frame of the lowest episode");
    drop((tools, temp));
    Ok(())
}

#[tokio::test]
async fn t_14_anime_cover_fallback_when_file_missing() -> io::Result<()> {
    let (_tools, frames, _temp) = fake_tools("10", "frame")?;
    let (root, _) = library(&[("Show", &["Show 1.mkv"])])?;
    let source = LocalAnimeSource::with_frames(Some(root.path().to_path_buf()), frames);
    let details = AnimeSource::anime_details(&source, &anime("Show")).await.map_err(io::Error::other)?;
    assert!(details.thumbnail_url.is_none(), "details alone do not invent a cover");
    AnimeSource::episode_list(&source, &anime("Show")).await.map_err(io::Error::other)?;
    let details = AnimeSource::anime_details(&source, &anime("Show")).await.map_err(io::Error::other)?;
    assert!(details.thumbnail_url.is_some(), "the cover is filled in once the episode list made it");
    Ok(())
}

#[tokio::test]
async fn t_14_anime_season_update_flags_false_keeps_input() -> io::Result<()> {
    let (_root, source) = library(&[("Show", &["Show 1.mkv"])])?;
    let input = vec![episode("Show/Show 1.mkv")];
    let update = AnimeSource::anime_episode_update(&source, anime("Show"), input.clone(), false, false)
        .await
        .map_err(io::Error::other)?;
    assert_eq!(update.episodes, input, "with both flags off the input comes back unchanged");
    assert_eq!(update.anime, anime("Show"));
    Ok(())
}

#[tokio::test]
async fn t_14_anime_season_update_error_from_one_side_still_returns_error() -> io::Result<()> {
    let (root, source) = library(&[("Show", &["Show 1.mkv"])])?;
    fs::write(root.path().join("Show").join("details.json"), "{")?;
    let result = AnimeSource::anime_episode_update(&source, anime("Show"), Vec::new(), true, true).await;
    assert!(result.is_err(), "a failing details branch fails the whole update");
    Ok(())
}

#[tokio::test]
async fn t_14_playback_hoster_single_video_title() -> io::Result<()> {
    let (_root, source) = library(&[("Show", &["Show - 1.mkv"])])?;
    let hosters = AnimeSource::hosters(&source, &episode("Show/Show - 1.mkv")).await.map_err(io::Error::other)?;
    assert_eq!(hosters.len(), 1);
    let videos = hosters[0].video_list.clone().expect("videos");
    assert_eq!(videos.len(), 1);
    assert!(videos[0].video_url.starts_with("file://") && videos[0].video_url.ends_with("/Show/Show%20-%201.mkv"), "{}", videos[0].video_url);
    assert_eq!(videos[0].video_title, "Local source: Show/Show - 1.mkv");
    Ok(())
}

#[tokio::test]
async fn t_14_playback_missing_file_empty() -> io::Result<()> {
    let (_root, source) = library(&[("Show", &[])])?;
    let hosters = AnimeSource::hosters(&source, &episode("Show/gone.mkv")).await.map_err(io::Error::other)?;
    assert!(hosters.is_empty());
    Ok(())
}

#[tokio::test]
async fn t_14_playback_bad_url_empty() -> io::Result<()> {
    let (_root, source) = library(&[("Show", &["Show 1.mkv"])])?;
    for bad in ["../Show/Show 1.mkv", "/etc/passwd", "Show\\Show 1.mkv", ""] {
        let hosters = AnimeSource::hosters(&source, &episode(bad)).await.map_err(io::Error::other)?;
        assert!(hosters.is_empty(), "{bad:?} gives no hoster");
    }
    Ok(())
}

#[tokio::test]
async fn t_14_playback_external_uses_path() -> io::Result<()> {
    let (root, source) = library(&[("Show", &["Show 1.mkv"])])?;
    let hosters = AnimeSource::hosters(&source, &episode("Show/Show 1.mkv")).await.map_err(io::Error::other)?;
    let uri = hosters[0].video_list.as_ref().expect("videos")[0].video_url.clone();
    let path = url::Url::parse(&uri).map_err(io::Error::other)?.to_file_path().map_err(|()| io::Error::other("not a file url"))?;
    assert_eq!(path, root.path().join("Show").join("Show 1.mkv"), "the external player gets a plain file path");
    Ok(())
}

#[test]
fn t_14_format_bad_url_chapter_not_found() {
    let root = tempfile::tempdir().unwrap();
    let got = format_of_chapter(Some(root.path()), "../escape");
    assert!(matches!(got, Err(LocalSourceError::ChapterNotFound)));
    assert_eq!(LocalSourceError::ChapterNotFound.to_string(), "Chapter not found");
}

#[test]
fn t_14_format_missing_chapter() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("Show")).unwrap();
    assert!(matches!(format_of_chapter(Some(root.path()), "Show/Missing"), Err(LocalSourceError::ChapterNotFound)));
    assert!(matches!(format_of_chapter(Some(root.path()), "NoSlash"), Err(LocalSourceError::ChapterNotFound)));
}

#[test]
fn t_14_format_unknown_type_invalid() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("Show")).unwrap();
    fs::write(root.path().join("Show").join("notes.txt"), b"x").unwrap();
    assert!(matches!(format_of_chapter(Some(root.path()), "Show/notes.txt"), Err(LocalSourceError::InvalidFormat)));
    assert_eq!(LocalSourceError::InvalidFormat.to_string(), "Invalid chapter format");
}

#[test]
fn t_14_format_epub_open_error_propagates() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("Show")).unwrap();
    fs::write(root.path().join("Show").join("Book.epub"), b"not a zip").unwrap();
    let got = format_of_chapter(Some(root.path()), "Show/Book.epub");
    assert!(matches!(got, Ok(Format::Epub(_))), "the format is found; reading the broken EPUB fails later");
    assert!(kuro_local::epub::read_metadata(&root.path().join("Show").join("Book.epub")).is_err());
}

#[test]
fn t_14_media_frame_position_truncates() {
    assert_eq!(frame_position(4.9), 2);
    assert_eq!(frame_position(5.0), 2);
}

#[test]
fn t_14_media_real_ffmpeg_frame() {
    if !has_ffmpeg() {
        eprintln!("skipped t_14_media_real_ffmpeg_frame: ffmpeg or ffprobe is not installed");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let clip = dir.path().join("tiny.mp4");
    let status = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-f", "lavfi", "-i", "testsrc=duration=4:size=64x64:rate=5", "-y"])
        .arg(&clip)
        .status()
        .expect("ffmpeg runs");
    assert!(status.success());
    let extractor = FrameExtractor::from_path(dir.path());
    let bytes = extractor.frame(&clip).unwrap().expect("a frame");
    assert_eq!(&bytes[..2], b"\xFF\xD8", "a JPEG frame");
    let tool = Tool::on_path("ffprobe").unwrap();
    assert!((extractor.probe_duration(&tool, &clip).unwrap() - 4.0).abs() < 0.2);
}

#[test]
fn t_14_media_missing_tool_skips() {
    let dir = tempfile::tempdir().unwrap();
    let extractor = FrameExtractor::new(None, None, dir.path());
    assert_eq!(extractor.frame(&dir.path().join("x.mp4")).unwrap(), None);
}

#[test]
fn t_14_media_bad_duration() {
    let dir = tempfile::tempdir().unwrap();
    let probe = dir.path().join("probe.sh");
    fs::write(&probe, "echo nonsense\n").unwrap();
    let extractor = FrameExtractor::new(Some(Tool::new("sh").with_args([probe.clone()])), None, dir.path());
    let tool = Tool::new("sh").with_args([probe]);
    assert!(extractor.probe_duration(&tool, &dir.path().join("x.mp4")).is_err());
}

#[test]
fn t_14_media_zero_duration_frame_zero() {
    assert_eq!(frame_position(0.0), 0);
}

#[test]
fn t_14_media_empty_output_no_image() {
    let dir = tempfile::tempdir().unwrap();
    let probe = dir.path().join("probe.sh");
    let ffmpeg = dir.path().join("ffmpeg.sh");
    fs::write(&probe, "echo 10\n").unwrap();
    fs::write(&ffmpeg, "exit 0\n").unwrap();
    let extractor = FrameExtractor::new(
        Some(Tool::new("sh").with_args([probe])),
        Some(Tool::new("sh").with_args([ffmpeg])),
        dir.path(),
    );
    assert_eq!(extractor.frame(&dir.path().join("x.mp4")).unwrap(), None, "an empty output is no image");
}

#[test]
fn t_14_media_timeout_kills_child() {
    let dir = tempfile::tempdir().unwrap();
    let probe = dir.path().join("probe.sh");
    fs::write(&probe, "sleep 30\n").unwrap();
    let extractor = FrameExtractor::new(Some(Tool::new("sh").with_args([probe.clone()])), None, dir.path())
        .with_timeouts(std::time::Duration::from_millis(200), std::time::Duration::from_millis(200));
    let tool = Tool::new("sh").with_args([probe]);
    let started = std::time::Instant::now();
    assert!(extractor.probe_duration(&tool, &dir.path().join("x.mp4")).is_err());
    assert!(started.elapsed() < std::time::Duration::from_secs(10), "the child is killed at the timeout");
}

#[test]
fn t_14_media_path_with_quotes_and_spaces() {
    let dir = tempfile::tempdir().unwrap();
    let probe = dir.path().join("probe.sh");
    fs::write(&probe, "echo \"$@\" | grep -q \"it's a clip.mp4\" && echo 8\n").unwrap();
    let extractor = FrameExtractor::new(Some(Tool::new("sh").with_args([probe.clone()])), None, dir.path());
    let tool = Tool::new("sh").with_args([probe]);
    let input = dir.path().join("it's a clip.mp4");
    assert_eq!(extractor.probe_duration(&tool, &input).unwrap(), 8.0);
}

#[test]
fn t_14_media_serialized() {
    let dir = tempfile::tempdir().unwrap();
    let probe = dir.path().join("probe.sh");
    fs::write(&probe, "sleep 0.2\necho 4\n").unwrap();
    let ffmpeg = dir.path().join("ffmpeg.sh");
    fs::write(&ffmpeg, "printf 'x' > \"$9\"\n").unwrap();
    let extractor = std::sync::Arc::new(FrameExtractor::new(
        Some(Tool::new("sh").with_args([probe])),
        Some(Tool::new("sh").with_args([ffmpeg])),
        dir.path(),
    ));
    let input = dir.path().join("x.mp4");
    let handles: Vec<_> = (0..3)
        .map(|_| {
            let extractor = extractor.clone();
            let input = input.clone();
            std::thread::spawn(move || extractor.frame(&input).unwrap())
        })
        .collect();
    for handle in handles {
        assert!(handle.join().unwrap().is_some());
    }
}

#[test]
fn t_14_media_drop_cleans_temp() {
    let dir = tempfile::tempdir().unwrap();
    let probe = dir.path().join("probe.sh");
    fs::write(&probe, "echo 10\n").unwrap();
    let ffmpeg = dir.path().join("ffmpeg.sh");
    fs::write(&ffmpeg, "printf 'x' > \"$9\"\n").unwrap();
    let extractor = FrameExtractor::new(
        Some(Tool::new("sh").with_args([probe])),
        Some(Tool::new("sh").with_args([ffmpeg])),
        dir.path(),
    );
    let _ = extractor.frame(&dir.path().join("x.mp4")).unwrap();
    let left: Vec<_> = fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert!(left.iter().all(|n| !n.to_string_lossy().starts_with("tmp_")), "{left:?}");
}
