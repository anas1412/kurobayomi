//! `LocalAnimeSource`: the public API for local anime (area:14 section 1).
//!
//! Registration: id 0, name `Local anime source`, lang `other`, unmetered, latest supported, no related anime.

pub mod episodes;

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use kuro_source::anime::{AnimeRelation, AnimeSource, AnimesPage, FetchType, Hoster, SAnime, SEpisode, local_hosters};
use kuro_source::{FilterList, SortSelection, SourceError};

use crate::LOCAL_SOURCE_ID;
use crate::browse::browse;
use crate::error::LocalSourceError;
use crate::format::is_anime_episode;
use crate::fs::{FsEntry, file_url, find_child, list_dir, safe_join};
use crate::images::find_image;
use crate::json_side::{AnimeDetails, apply_anime_details, details_file, read_details_json};
use crate::media::FrameExtractor;
use crate::sort::{LATEST_SORT, POPULAR_SORT, order_by, sort_from};

/// `LocalAnimeSource.name`: `local_anime_source` in the i18n table.
pub const NAME: &str = "Local anime source";

/// The local anime source over `<storage base>/localanime`. `root` is `None` while the storage base is missing.
#[derive(Debug, Clone)]
pub struct LocalAnimeSource {
    root: Option<PathBuf>,
    /// Makes the video frames for thumbnails, covers and backgrounds (T-14a-026).
    frames: FrameExtractor,
}

impl LocalAnimeSource {
    /// Uses `ffmpeg` and `ffprobe` from `PATH` and the system temp folder. Without the tools no frame is made.
    pub fn new(root: Option<PathBuf>) -> Self {
        Self::with_frames(root, FrameExtractor::from_path(std::env::temp_dir()))
    }

    pub fn with_frames(root: Option<PathBuf>, frames: FrameExtractor) -> Self {
        Self { root, frames }
    }

    /// Kotlin `getOldAnimeDetails`: the cover and background steps (behavior 11.4), then `details.json` (T-14a-038).
    /// A decode failure propagates to the caller (E-ANIME-2).
    async fn details(&self, anime: &SAnime) -> Result<SAnime, LocalSourceError> {
        let root = self.root.clone();
        let anime = anime.clone();
        crate::blocking(move || {
            let mut updated = anime;
            let Some(dir) = root.and_then(|root| find_child(&root, &updated.url)) else {
                return Ok(updated);
            };
            if let Some(cover) = find_image(&dir, "cover").and_then(|p| file_url(&p)) {
                updated.thumbnail_url = Some(cover);
            }
            if let Some(background) = find_image(&dir, "background").and_then(|p| file_url(&p)) {
                updated.background_url = Some(background);
            }
            let entries = list_dir(&dir)?;
            if let Some(json) = details_file(&entries) {
                let details = read_details_json::<AnimeDetails>(&json.path)?;
                apply_anime_details(&mut updated, &details);
            }
            Ok(updated)
        })
        .await
    }

    async fn browse_page(
        &self,
        query: String,
        latest: bool,
        sort: Option<SortSelection>,
    ) -> Result<AnimesPage, LocalSourceError> {
        let animes = browse(self.root.clone(), query, latest, sort, anime_entry).await?;
        Ok(AnimesPage {
            animes,
            has_next_page: false,
        })
    }
}

/// Kotlin `fetchTypeManager.find`: an episode file anywhere in the folder means episodes. Otherwise any folder,
/// hidden ones included, means seasons. Otherwise episodes (behavior 11.1).
pub fn fetch_type_of(children: &[FsEntry]) -> FetchType {
    if children.iter().any(|e| is_anime_episode(&e.name)) {
        FetchType::Episodes
    } else if children.iter().any(|e| e.is_dir) {
        FetchType::Seasons
    } else {
        FetchType::Episodes
    }
}

/// `fetch_type_of` for a folder on disk. A folder that cannot be listed counts as empty, as in Kotlin.
fn fetch_type_at(dir: &Path) -> FetchType {
    list_dir(dir).map_or(FetchType::Episodes, |children| fetch_type_of(&children))
}

/// One browse row: the folder name is title and URL, with its fetch type, cover and background.
fn anime_entry(entry: &FsEntry) -> SAnime {
    SAnime {
        url: entry.name.clone(),
        title: entry.name.clone(),
        fetch_type: fetch_type_at(&entry.path),
        thumbnail_url: find_image(&entry.path, "cover").and_then(|p| file_url(&p)),
        background_url: find_image(&entry.path, "background").and_then(|p| file_url(&p)),
        ..SAnime::default()
    }
}

#[async_trait]
impl AnimeSource for LocalAnimeSource {
    fn id(&self) -> i64 {
        LOCAL_SOURCE_ID
    }

    fn name(&self) -> &str {
        NAME
    }

    fn lang(&self) -> &str {
        "other"
    }

    fn supports_latest(&self) -> bool {
        true
    }

    fn supports_related_anime(&self) -> bool {
        false
    }

    fn is_unmetered(&self) -> bool {
        true
    }

    fn filter_list(&self) -> FilterList {
        FilterList(vec![order_by(Some(POPULAR_SORT))])
    }

    async fn popular(&self, _page: i32) -> Result<AnimesPage, SourceError> {
        Ok(self
            .browse_page(String::new(), false, Some(POPULAR_SORT))
            .await?)
    }

    async fn latest(&self, _page: i32) -> Result<AnimesPage, SourceError> {
        Ok(self
            .browse_page(String::new(), true, Some(LATEST_SORT))
            .await?)
    }

    async fn search(
        &self,
        _page: i32,
        query: &str,
        filters: &FilterList,
    ) -> Result<AnimesPage, SourceError> {
        Ok(self
            .browse_page(query.to_owned(), false, sort_from(filters))
            .await?)
    }

    async fn anime_details(&self, anime: &SAnime) -> Result<SAnime, SourceError> {
        Ok(self.details(anime).await?)
    }

    async fn episode_list(&self, anime: &SAnime) -> Result<Vec<SEpisode>, SourceError> {
        let root = self.root.clone();
        let anime = anime.clone();
        let frames = self.frames.clone();
        let list = crate::blocking(move || match root {
            Some(root) => episodes::episode_list_with(&root, &anime, |video| frames.frame_io(video)),
            None => Ok(Vec::new()),
        })
        .await?;
        Ok(list)
    }

    /// The seasons of an anime folder: its non-hidden sub-folders in name order (behavior 11.3, T-14a-024).
    async fn season_list(&self, anime: &SAnime) -> Result<Vec<SAnime>, SourceError> {
        let root = self.root.clone();
        let url = anime.url.clone();
        let seasons = crate::blocking(move || {
            let Some(dir) = root.and_then(|root| find_child(&root, &url)) else {
                return Ok(Vec::new());
            };
            let mut seasons = Vec::new();
            for entry in list_dir(&dir)?.into_iter().filter(|e| e.is_dir && !e.name.starts_with('.')) {
                seasons.push(SAnime {
                    url: format!("{url}/{}", entry.name),
                    title: entry.name.clone(),
                    fetch_type: fetch_type_at(&entry.path),
                    ..SAnime::default()
                });
            }
            Ok(seasons)
        })
        .await?;
        Ok(seasons)
    }

    /// One hoster holding the episode's file as a `file://` video (behavior 11.7, T-14a-025). An unsafe or missing
    /// episode URL gives an empty list, with no error.
    async fn hosters(&self, episode: &SEpisode) -> Result<Vec<Hoster>, SourceError> {
        let root = self.root.clone();
        let url = episode.url.clone();
        let hosters = crate::blocking(move || {
            let file = root
                .and_then(|root| safe_join(&root, &url))
                .filter(|path| path.is_file())
                .and_then(|path| file_url(&path));
            Ok(local_hosters(&url, file))
        })
        .await?;
        Ok(hosters)
    }

    /// The local source lists hosters, one per episode file, so the hoster path is used instead of the legacy videos.
    fn provides_hosters(&self) -> bool {
        true
    }

    /// Kotlin `getRelatedAnimeList` returns an empty list.
    async fn related_anime(&self, _anime: &SAnime) -> Result<Vec<AnimeRelation>, SourceError> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io;

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

    #[test]
    fn the_registration_is_the_kotlin_identity() {
        let source = LocalAnimeSource::new(None);
        assert_eq!(AnimeSource::id(&source), 0);
        assert_eq!(AnimeSource::name(&source), "Local anime source");
        assert_eq!(AnimeSource::lang(&source), "other");
        assert!(AnimeSource::supports_latest(&source));
        assert!(!AnimeSource::supports_related_anime(&source));
        assert!(AnimeSource::is_unmetered(&source));
    }

    #[test]
    fn fetch_type_prefers_episodes_then_folders_then_episodes() {
        let file = |name: &str, is_dir: bool| FsEntry {
            name: name.into(),
            path: PathBuf::from(name),
            is_dir,
            is_file: !is_dir,
            modified_ms: 0,
        };
        assert_eq!(
            fetch_type_of(&[file("s1", true), file("ep.mkv", false)]),
            FetchType::Episodes
        );
        assert_eq!(
            fetch_type_of(&[file("s1", true), file(".hidden", true)]),
            FetchType::Seasons
        );
        assert_eq!(fetch_type_of(&[file("readme", false)]), FetchType::Episodes);
        assert_eq!(
            fetch_type_of(&[file("Extra.mkv", true)]),
            FetchType::Episodes
        );
    }

    #[tokio::test]
    async fn browse_lists_folders_with_their_fetch_type_and_images() -> io::Result<()> {
        let (root, source) = library(&[
            ("Show", &["ep.mkv", "cover.jpg"]),
            ("Seasoned", &[]),
            ("Empty", &[]),
        ])?;
        fs::create_dir(root.path().join("Seasoned").join("S1"))?;
        fs::write(
            root.path().join("Show").join("background.png"),
            b"\x89PNG\r\n\x1a\n",
        )?;
        let page = AnimeSource::popular(&source, 1)
            .await
            .map_err(io::Error::other)?;
        let by_title = |t: &str| page.animes.iter().find(|a| a.title == t).cloned().expect(t);
        let show = by_title("Show");
        assert_eq!(show.url, "Show");
        assert_eq!(show.fetch_type, FetchType::Episodes);
        assert!(
            show.thumbnail_url
                .as_deref()
                .is_some_and(|u| u.ends_with("/Show/cover.jpg"))
        );
        assert!(
            show.background_url
                .as_deref()
                .is_some_and(|u| u.ends_with("/Show/background.png"))
        );
        assert_eq!(by_title("Seasoned").fetch_type, FetchType::Seasons);
        assert!(!page.has_next_page);
        Ok(())
    }

    #[tokio::test]
    async fn episodes_come_from_the_anime_folder() -> io::Result<()> {
        let (_root, source) = library(&[("Show", &["Show - 1.mkv", "Show - 2.mp4"])])?;
        let anime = SAnime {
            url: "Show".into(),
            title: "Show".into(),
            ..SAnime::default()
        };
        let got = AnimeSource::episode_list(&source, &anime)
            .await
            .map_err(io::Error::other)?;
        let numbers: Vec<f32> = got.iter().map(|e| e.episode_number).collect();
        assert_eq!(numbers, [2.0, 1.0]);
        Ok(())
    }

    #[tokio::test]
    async fn t_14_thumbnail_temp_removed() -> io::Result<()> {
        use crate::media::Tool;
        // fake tools: ffprobe says 10 s, ffmpeg writes a fake JPEG to its output argument ($9)
        let tools = tempfile::tempdir()?;
        let probe = tools.path().join("ffprobe.sh");
        let ffmpeg = tools.path().join("ffmpeg.sh");
        fs::write(&probe, "echo 10\n")?;
        fs::write(&ffmpeg, "printf 'jpeg' > \"$9\"\n")?;
        let temp = tempfile::tempdir()?;
        let frames = FrameExtractor::new(
            Some(Tool::new("sh").with_args([probe])),
            Some(Tool::new("sh").with_args([ffmpeg])),
            temp.path(),
        );
        let (root, _source) = library(&[("Show", &["Show - 1.mkv"])])?;
        let source = LocalAnimeSource::with_frames(Some(root.path().to_path_buf()), frames);
        let anime = SAnime { url: "Show".into(), title: "Show".into(), ..SAnime::default() };
        AnimeSource::episode_list(&source, &anime).await.map_err(io::Error::other)?;
        let thumb = root.path().join("Show").join("Show - 1-thumbnail.jpg");
        assert_eq!(fs::read(thumb)?, b"jpeg", "the frame was written next to the episode");
        assert!(root.path().join("Show").join("cover.jpg").is_file(), "the lowest episode gives the cover too");
        assert_eq!(fs::read_dir(temp.path())?.count(), 0, "no tmp_ file is left in the temp folder");
        Ok(())
    }

    #[tokio::test]
    async fn related_anime_is_empty() -> io::Result<()> {
        let source = LocalAnimeSource::new(None);
        let got = AnimeSource::related_anime(&source, &SAnime::default())
            .await
            .map_err(io::Error::other)?;
        assert!(got.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn details_fills_the_cover_and_background_only() -> io::Result<()> {
        let (root, source) = library(&[("Show", &["cover.jpg", "background.jpg"])])?;
        let anime = SAnime {
            url: "Show".into(),
            title: "Show".into(),
            ..SAnime::default()
        };
        let updated = AnimeSource::anime_details(&source, &anime)
            .await
            .map_err(io::Error::other)?;
        assert!(updated.thumbnail_url.is_some() && updated.background_url.is_some());
        assert_eq!(updated.title, "Show");
        assert!(root.path().join("Show").is_dir());
        Ok(())
    }

    #[tokio::test]
    async fn anime_details_json_fills_the_six_fields_without_comicinfo() -> io::Result<()> {
        let (root, source) = library(&[("Show", &[])])?;
        fs::write(
            root.path().join("Show").join("details.json"),
            r#"{"title":"Real","artist":"Art","genre":["A","B"],"status":2}"#,
        )?;
        let anime = SAnime {
            url: "Show".into(),
            title: "Show".into(),
            ..SAnime::default()
        };
        let updated = AnimeSource::anime_details(&source, &anime)
            .await
            .map_err(io::Error::other)?;
        assert_eq!(updated.title, "Real");
        assert_eq!(updated.artist.as_deref(), Some("Art"));
        assert_eq!(updated.genre.as_deref(), Some("A, B"));
        assert!(!root.path().join("Show").join("ComicInfo.xml").exists());
        Ok(())
    }

    #[tokio::test]
    async fn a_bad_anime_details_json_fails_the_details() -> io::Result<()> {
        let (root, source) = library(&[("Show", &[])])?;
        fs::write(root.path().join("Show").join("details.json"), "{")?;
        let anime = SAnime {
            url: "Show".into(),
            title: "Show".into(),
            ..SAnime::default()
        };
        assert!(AnimeSource::anime_details(&source, &anime).await.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn a_missing_root_lists_nothing() -> io::Result<()> {
        let source = LocalAnimeSource::new(None);
        let page = AnimeSource::latest(&source, 1)
            .await
            .map_err(io::Error::other)?;
        assert!(page.animes.is_empty());
        Ok(())
    }
}
