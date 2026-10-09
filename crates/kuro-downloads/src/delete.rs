//! Delete interactors for downloaded chapters and episodes (port of DeleteChapterDownload.kt and
//! DeleteEpisodeDownload.kt; T-25a-010, T-25a-011, T-25a-006 parsing). The category sets come from the preference store
//! as strings, and a value that is not a number aborts the call.

use std::collections::BTreeSet;
use std::sync::Arc;

use tokio::sync::oneshot;
use tracing::warn;

use crate::anime::manager::{AnimeQueue, Command as AnimeCommand};
use crate::anime::model::{AnimeRef, AnimeSource, EpisodeRef};
use crate::error::DownloadError;
use crate::manager::{DeleteRules, DownloadManager};
use crate::model::{ChapterRef, MangaRef};
use crate::source::ChapterSource;

/// Errors of the delete path that the caller logs.
#[derive(Debug, thiserror::Error)]
pub enum DeleteError {
    /// A stored category id is not a whole number (E-05). The delete call is aborted.
    #[error("preference {key} holds a non-numeric category id: {value}")]
    BadCategoryId { key: &'static str, value: String },
    #[error(transparent)]
    Download(#[from] DownloadError),
}

/// The six category-set keys (`DownloadPreferences.categoryPreferenceKeys`, same order). Backup restore merges these with
/// the library category keys (PreferenceRestorer.kt:136).
pub const CATEGORY_KEYS: [&str; 6] = [
    "remove_exclude_categories",
    "remove_exclude_anime_categories",
    "download_new_categories",
    "download_new_anime_categories",
    "download_new_categories_exclude",
    "download_new_anime_categories_exclude",
];

/// Parses stored category ids. Any value that is not an integer fails the whole set, so no id is silently dropped.
pub fn parse_category_ids(key: &'static str, values: &BTreeSet<String>) -> Result<BTreeSet<i64>, DeleteError> {
    values
        .iter()
        .map(|value| {
            value.parse::<i64>().map_err(|_| DeleteError::BadCategoryId { key, value: value.clone() })
        })
        .collect()
}

/// The delete settings read from the store for one kind.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeleteSettings {
    /// Categories whose watched or read items are kept.
    pub exclude_categories: BTreeSet<i64>,
    /// `pref_remove_bookmarked`: allow deleting bookmarked items.
    pub remove_bookmarked: bool,
}

impl DeleteSettings {
    /// Builds the settings from the stored category set. A value that is not a number aborts the whole call with
    /// `BadCategoryId`, which is logged, and nothing is deleted (E-05).
    pub fn from_stored(key: &'static str, exclude: &BTreeSet<String>, remove_bookmarked: bool) -> Result<Self, DeleteError> {
        match parse_category_ids(key, exclude) {
            Ok(ids) => Ok(Self { exclude_categories: ids, remove_bookmarked }),
            Err(err) => {
                warn!(target: "kuro_downloads", error = %err, "delete aborted: stored category id is not a number");
                Err(err)
            }
        }
    }
}

/// Looks up an installed manga or novel source by id (`sourceManager.get`). `None` means the source is not installed.
pub trait ChapterSourceResolver: Send + Sync {
    fn chapter_source(&self, source_id: i64) -> Option<Arc<dyn ChapterSource>>;
}

/// Looks up an installed anime source by id. `None` means the source is not installed.
pub trait AnimeSourceResolver: Send + Sync {
    fn anime_source(&self, source_id: i64) -> Option<AnimeSource>;
}

/// `DeleteChapterDownload.awaitAll`.
pub struct DeleteChapterDownload {
    sources: Arc<dyn ChapterSourceResolver>,
    manager: Arc<DownloadManager>,
}

impl DeleteChapterDownload {
    pub fn new(sources: Arc<dyn ChapterSourceResolver>, manager: Arc<DownloadManager>) -> Self {
        Self { sources, manager }
    }

    /// Deletes the chapters the rules allow. Returns how many were selected, and `0` with no file touched when the
    /// source is unknown (E-02). The work runs on a spawned task, so a dropped caller cannot stop it half way (T-25a-016).
    pub async fn await_all(
        &self,
        manga: &MangaRef,
        chapters: Vec<ChapterRef>,
        manga_categories: Vec<i64>,
        settings: &DeleteSettings,
    ) -> Result<usize, DownloadError> {
        let Some(source) = self.sources.chapter_source(manga.source_id) else {
            warn!(target: "kuro_downloads", manga_id = manga.id, source_id = manga.source_id, "delete skipped: source is not installed");
            return Ok(0);
        };
        let rules = DeleteRules {
            exclude_categories: settings.exclude_categories.iter().copied().collect(),
            manga_categories,
            remove_bookmarked: settings.remove_bookmarked,
        };
        let manager = self.manager.clone();
        let manga = manga.clone();
        let task = tokio::spawn(async move { manager.delete_chapters(chapters, &manga, source.as_ref(), &rules).await });
        task.await.map_err(|e| DownloadError::Internal(e.to_string()))?
    }
}

/// `DeleteEpisodeDownload.awaitAll`. The anime actor applies its own delete rules, as it always did.
pub struct DeleteEpisodeDownload {
    sources: Arc<dyn AnimeSourceResolver>,
    queue: AnimeQueue,
}

impl DeleteEpisodeDownload {
    pub fn new(sources: Arc<dyn AnimeSourceResolver>, queue: AnimeQueue) -> Self {
        Self { sources, queue }
    }

    /// The Rust form of `await_items(entry, candidates, names)` (T-25a-010): `anime` is the entry, `episodes` the
    /// candidates, and `categories` the entry's category ids (the rules read the settings in the actor, so no names are
    /// passed). The source is resolved first, and an unknown one gives `Ok(0)` (E-02). The anime actor runs the shared
    /// delete steps of [`EntryKind::Anime`](crate::queue_model::EntryKind) on the blocking pool. Returns how many
    /// episodes the rules selected; the reply arrives after the folders are removed, so the caller can refresh the rows.
    pub async fn await_all(
        &self,
        anime: &AnimeRef,
        episodes: Vec<EpisodeRef>,
        categories: Vec<i64>,
    ) -> Result<usize, DownloadError> {
        let Some(source) = self.sources.anime_source(anime.source_id) else {
            warn!(target: "kuro_downloads", anime_id = anime.id, source_id = anime.source_id, "delete skipped: source is not installed");
            return Ok(0);
        };
        let (reply, answer) = oneshot::channel();
        self.queue.send(AnimeCommand::DeleteEpisodes {
            source,
            anime: anime.clone(),
            episodes,
            categories,
            reply,
        })?;
        answer.await.map_err(|_| DownloadError::Internal("anime queue manager stopped".into()))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anime::layout::AnimeLayout;
    use crate::layout::DownloadLayout;

    #[test]
    fn category_keys_match_the_kotlin_order() {
        assert_eq!(CATEGORY_KEYS[0], "remove_exclude_categories");
        assert_eq!(CATEGORY_KEYS[5], "download_new_anime_categories_exclude");
        assert_eq!(CATEGORY_KEYS.len(), 6);
    }

    #[test]
    fn numeric_category_ids_parse_and_a_bad_one_fails_the_whole_set() {
        let good: BTreeSet<String> = ["3", "-1", "12"].iter().map(|s| s.to_string()).collect();
        assert_eq!(parse_category_ids("remove_exclude_categories", &good).unwrap(), BTreeSet::from([-1, 3, 12]));
        let bad: BTreeSet<String> = ["3", "abc"].iter().map(|s| s.to_string()).collect();
        match parse_category_ids("download_new_categories", &bad) {
            Err(DeleteError::BadCategoryId { key, value }) => {
                assert_eq!(key, "download_new_categories");
                assert_eq!(value, "abc");
            }
            other => panic!("E-05: expected BadCategoryId, got {other:?}"),
        }
    }

    #[test]
    fn an_empty_set_parses_to_an_empty_set() {
        assert!(parse_category_ids("remove_exclude_categories", &BTreeSet::new()).unwrap().is_empty());
    }

    #[test]
    fn delete_settings_abort_on_a_bad_stored_id() {
        let good: BTreeSet<String> = ["4".to_owned()].into();
        let settings = DeleteSettings::from_stored("remove_exclude_categories", &good, true).unwrap();
        assert_eq!(settings, DeleteSettings { exclude_categories: BTreeSet::from([4]), remove_bookmarked: true });
        let bad: BTreeSet<String> = ["x".to_owned()].into();
        assert!(matches!(
            DeleteSettings::from_stored("remove_exclude_categories", &bad, false),
            Err(DeleteError::BadCategoryId { .. })
        ));
    }

    struct NoAnimeSource;

    impl AnimeSourceResolver for NoAnimeSource {
        fn anime_source(&self, _source_id: i64) -> Option<AnimeSource> {
            None
        }
    }

    struct NoChapterSource;

    impl ChapterSourceResolver for NoChapterSource {
        fn chapter_source(&self, _source_id: i64) -> Option<Arc<dyn ChapterSource>> {
            None
        }
    }

    fn episode(id: i64) -> EpisodeRef {
        EpisodeRef {
            id,
            anime_id: 5,
            url: format!("/e/{id}"),
            name: format!("Ep {id}"),
            scanlator: None,
            seen: false,
            bookmark: false,
            fillermark: false,
            source_order: id,
            upload_date: 0,
            episode_number: id as f64,
        }
    }

    #[tokio::test]
    async fn an_unknown_anime_source_deletes_nothing_and_returns_zero() {
        let root = tempfile::tempdir().unwrap();
        let show = AnimeRef { id: 5, source_id: 9, url: "/a/5".into(), title: "Show".into() };
        let owner = AnimeSource { id: 9, name: "Gone".into(), lang: "en".into(), is_http: true, unmetered: false };
        let folder = AnimeLayout::new(Some(root.path().to_path_buf())).anime_dir(&owner, &show.title).unwrap();
        std::fs::create_dir(folder.join("Ep 1")).unwrap();
        let delete = DeleteEpisodeDownload::new(Arc::new(NoAnimeSource), crate::testing::anime_queue(root.path()));
        let deleted = delete.await_all(&show, vec![episode(1)], vec![]).await.unwrap();
        assert_eq!(deleted, 0, "E-02: an unknown source is a logged no-op");
        assert!(folder.join("Ep 1").is_dir(), "no file is touched");
    }

    #[tokio::test]
    async fn an_unknown_manga_source_deletes_nothing_and_returns_zero() {
        let base = tempfile::tempdir().unwrap();
        let manager = crate::testing::manga_manager(base.path());
        let layout = DownloadLayout::new(base.path().join("downloads"));
        let dir = layout.manga_dir("Gone (EN)", "Manga 1").unwrap();
        std::fs::create_dir(dir.join("Chapter 1")).unwrap();
        let manga = crate::testing::manga(1, 9, "Manga 1");
        let delete = DeleteChapterDownload::new(Arc::new(NoChapterSource), manager);
        let chapters = vec![crate::testing::chapter(1, 1, "Chapter 1")];
        let deleted = delete.await_all(&manga, chapters, vec![], &DeleteSettings::default()).await.unwrap();
        assert_eq!(deleted, 0, "E-02: an unknown source is a logged no-op");
        assert!(dir.join("Chapter 1").is_dir(), "no file is touched");
    }
}
